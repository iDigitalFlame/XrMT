// Copyright (C) 2023 iDigitalFlame
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//

#![no_implicit_prelude]

use core::cmp::Ordering;

use crate::data::{Readable, Reader, Writable, Writer};
use crate::io;
use crate::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::*;
#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::filter::*;

pub trait ChildExtra {
    fn wait_with_output_in_combo(self, out: &mut Vec<u8>) -> io::Result<ExitStatus>;
    fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus>;
}

pub type ThreadEntry = list::ThreadEntry;
pub type ProcessEntry = list::ProcessEntry;

impl Eq for ThreadEntry {}
impl Ord for ThreadEntry {
    #[inline]
    fn cmp(&self, other: &ThreadEntry) -> Ordering {
        match self.pid {
            _ if self.pid == other.pid && self.tid == other.tid => Ordering::Equal,
            _ if self.pid == other.pid && self.tid < other.tid => Ordering::Less,
            _ if self.pid == other.pid => Ordering::Greater,
            _ if self.pid < other.pid => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}
impl Eq for ProcessEntry {}
impl Ord for ProcessEntry {
    #[inline]
    fn cmp(&self, other: &ProcessEntry) -> Ordering {
        match self.pid {
            _ if self.ppid == other.ppid && self.pid == other.pid => Ordering::Equal,
            _ if self.ppid == other.ppid && self.pid < other.pid => Ordering::Less,
            _ if self.ppid == other.ppid => Ordering::Greater,
            _ if self.pid < other.pid => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}
impl PartialEq for ThreadEntry {
    #[inline]
    fn eq(&self, other: &ThreadEntry) -> bool {
        self.tid == other.tid && self.pid == other.pid
    }
}
impl PartialEq for ProcessEntry {
    #[inline]
    fn eq(&self, other: &ProcessEntry) -> bool {
        self.pid == other.pid && self.ppid == other.ppid && self.user.eq(&other.user) && self.cmdline.eq(&other.cmdline)
    }
}
impl PartialOrd for ThreadEntry {
    #[inline]
    fn partial_cmp(&self, other: &ThreadEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialOrd for ProcessEntry {
    #[inline]
    fn partial_cmp(&self, other: &ProcessEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Writable for ProcessEntry {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u32(self.pid)?;
        w.write_u32(self.ppid)?;
        w.write_str(&self.cmdline)?;
        w.write_str(&self.user)
    }
}
impl Readable for ProcessEntry {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u32(&mut self.pid)?;
        r.read_into_u32(&mut self.ppid)?;
        r.read_into_str(&mut self.cmdline)?;
        r.read_into_str(&mut self.user)
    }
}

#[inline]
pub fn list_processes() -> io::Result<Vec<ProcessEntry>> {
    let mut e = list::processes()?;
    e.shrink_to_fit();
    e.sort();
    Ok(e)
}
#[inline]
pub fn list_threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
    let mut e = list::threads(pid)?;
    e.shrink_to_fit();
    e.sort();
    Ok(e)
}

#[cfg(target_os = "netbsd")]
mod list {
    extern crate libc;

    use core::ffi::CStr;
    use core::mem::size_of;

    use libc::kinfo_proc2;

    use crate::device::unix::sysctl;
    use crate::device::user_info;
    use crate::fs::read_dir;
    use crate::prelude::*;
    use crate::util::{crypt, ToStr};
    use crate::{io, ok_or_continue};

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use crate::process::unix::{ProcessEntry, ThreadEntry};

    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        // 0x1  - CTL_KERN
        // 0x2F - KERN_PROC2
        // 0x0  - KERN_PROC_ALL
        let b = sysctl(
            [
                libc::CTL_KERN,
                libc::KERN_PROC2,
                0,
                libc::KERN_PROC_ALL,
                size_of::<kinfo_proc2>() as i32,
                -1,
            ],
            false,
        )?;
        let s = unsafe { b.as_slice_of::<kinfo_proc2>() };
        let mut r = Vec::with_capacity(s.len());
        if s.len() == 0 {
            return Ok(r);
        }
        for i in s {
            if i.p_addr == 0 || i.p_comm[0] == 0 {
                continue;
            }
            let x = read_args(i).unwrap_or_else(|| unsafe { CStr::from_ptr(i.p_comm.as_ptr()) }.to_string_lossy().to_string());
            r.push(ProcessEntry {
                pid:     i.p_pid as _,
                ppid:    i.p_ppid as _,
                user:    user_info(i.p_uid, |p| {
                    unsafe { CStr::from_ptr(p.pw_name) }.to_string_lossy().to_string()
                })
                .unwrap_or_default(),
                cmdline: x,
            })
        }
        Ok(r)
    }
    pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
        // NOTE(dij): Threads on NetBSD are weird, they "kinda" show up how
        //            *nix does threads and not how BSD does them. Thread ID's
        //            seem to be non-existant? *shrug* At least we can tell IF
        //            there are threads running (outside main).
        let mut b = String::with_capacity(24);
        b.push_str(crypt::get_or(0, "/proc/"));
        let t = unsafe { b.as_mut_vec() };
        pid.into_vec(t);
        t.push(b'/');
        t.push(b't');
        t.push(b'a');
        t.push(b's');
        t.push(b'k');
        t.push(b'/');
        let mut r = Vec::new();
        for i in read_dir(b)? {
            let f = ok_or_continue!(i);
            let e = ok_or_continue!(f.metadata());
            if !e.is_dir() && !e.is_symlink() {
                continue;
            }
            r.push(ThreadEntry {
                pid,
                tid: ok_or_continue!(u32::from_str_radix(&f.file_name().to_string_lossy(), 10)),
            })
        }
        Ok(r)
    }

    fn read_args(p: &kinfo_proc2) -> Option<String> {
        // 0x1  - CTL_KERN
        // 0x30 - KERN_PROC_ARGS
        // 0x1  - KERN_PROC_ARGV
        let a = sysctl(
            [libc::CTL_KERN, libc::KERN_PROC_ARGS, p.p_pid, libc::KERN_PROC_ARGV],
            false,
        )
        .ok()?;
        let mut r = String::new();
        let t = unsafe { r.as_mut_vec() };
        for i in a.as_slice().split(|v| *v == 0) {
            if i.is_empty() {
                break;
            }
            if !t.is_empty() {
                t.push(b' ');
            }
            t.extend_from_slice(&i)
        }
        Some(r)
    }
}
#[cfg(target_vendor = "apple")]
mod list {
    extern crate libc;

    use core::ffi::CStr;
    use core::mem::{size_of, zeroed};

    use crate::device::unix::sysctl;
    use crate::device::user_info;
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use crate::process::unix::{ProcessEntry, ThreadEntry};

    #[repr(C)]
    struct ShortInfo {
        pid:    u32,
        ppid:   u32,
        pgid:   u32,
        status: u32,
        comm:   [u8; 16],
        flags:  u32,
        uid:    u32,
        gid:    u32,
        ruid:   u32,
        rgid:   u32,
        svuid:  u32,
        svgid:  u32,
        rfu:    u32,
    }

    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        let mut b = Vec::with_capacity(0x400);
        b.resize(0x400, 0);
        let n = unsafe { libc::proc_listallpids(b.as_mut_ptr() as _, b.len() as i32) };
        let mut r = Vec::new();
        if n == 0 {
            return Ok(r);
        }
        let s = size_of::<ShortInfo>() as i32;
        for i in 0..n as usize {
            let x: ShortInfo = unsafe { zeroed() };
            // 0xD - PROC_PIDT_SHORTBSDINFO
            let o = unsafe {
                libc::proc_pidinfo(
                    b[i],
                    0xD, // Isn't defined in headers.
                    0,
                    &x as *const ShortInfo as _,
                    s,
                )
            };
            if o != s || x.pid == 0 {
                continue;
            }
            r.push(convert(x))
        }
        Ok(r)
    }
    #[inline]
    pub fn threads(_pid: u32) -> io::Result<Vec<ThreadEntry>> {
        // BUG(dij): There is a way to get the Threads (tasks) for MacOS, but
        //           it now requires a special manifest to use, which we might
        //           not have, so it's not much worth it.
        //
        // But for reference, we could use the following
        //
        // - libc::task_for_pid(libc::mach_host_self(), pid as i32, &mut n)
        // - libc::task_threads(n, kk.as_mut_ptr() as _, &mut c) }
        // - libc::thread_info(target_act, flavor, thread_info_out, thread_info_outCnt)
        //
        // See https://stackoverflow.com/questions/73979485/get-number-of-active-threads-spawned-by-current-process-on-macos
        //     https://stackoverflow.com/questions/32943275/task-for-pid-stops-working-on-os-x-10-11
        //     https://stackoverflow.com/questions/34468640/getting-task-for-pid-to-work-in-el-capitan
        Err(ErrorKind::Unsupported.into())
    }

    fn convert(i: ShortInfo) -> ProcessEntry {
        let n = CStr::from_bytes_until_nul(&i.comm).map_or_else(|_| String::new(), |v| v.to_string_lossy().to_string());
        // 0x26 - KERN_PROCARGS
        // 0x1  - CTL_KERN
        let v = match sysctl([libc::CTL_KERN, libc::KERN_PROCARGS, i.pid as _, 0], false) {
            Ok(b) => b.as_null_str().to_string(),
            Err(_) => n,
        };
        ProcessEntry {
            pid:     i.pid,
            ppid:    i.ppid,
            user:    user_info(i.uid, |p| {
                unsafe { CStr::from_ptr(p.pw_name) }.to_string_lossy().to_string()
            })
            .unwrap_or_default(),
            cmdline: v,
        }
    }
}
#[cfg(target_family = "windows")]
mod list {
    pub(super) type ThreadEntry = winapi::ThreadEntry;
    pub(super) type ProcessEntry = winapi::ProcessEntry;

    use crate::device::winapi;
    use crate::io::{self, Error};
    use crate::prelude::*;

    #[inline]
    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        winapi::acquire_debug();
        let r = winapi::list_processes()
            .map_err(Error::from)
            .map(|e| e.into_iter().map(|v| v.into()).collect());
        winapi::release_debug();
        r
    }
    #[inline]
    pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
        winapi::acquire_debug();
        let r = winapi::list_threads(pid);
        winapi::release_debug();
        r.map_err(Error::from)
    }
}
#[cfg(target_vendor = "fortanix")]
mod list {
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use crate::process::unix::{ProcessEntry, ThreadEntry};

    #[inline]
    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        Err(ErrorKind::Unsupported.into())
    }
    #[inline]
    pub fn threads(_pid: u32) -> io::Result<Vec<ThreadEntry>> {
        Err(ErrorKind::Unsupported.into())
    }
}
#[cfg(all(
    not(target_os = "hurd"),
    not(target_os = "redox"),
    not(target_os = "netbsd"),
    not(target_os = "openbsd"),
    not(target_os = "freebsd"),
    not(target_os = "dragonfly"),
    not(target_family = "windows"),
    not(target_vendor = "apple"),
    not(target_vendor = "fortanix"),
))]
mod list {
    extern crate libc;

    use core::ffi::CStr;
    use std::os::unix::fs::MetadataExt;

    use crate::device::user_info;
    use crate::fs::{self, read_dir};
    use crate::prelude::*;
    use crate::util::{crypt, ToStr};
    use crate::{io, ok_or_continue};

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use crate::process::unix::{ProcessEntry, ThreadEntry};

    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        let mut r = Vec::new();
        for i in read_dir(crypt::get_or(0, "/proc/"))? {
            let e = ok_or_continue!(i);
            let m = ok_or_continue!(e.metadata());
            if !m.is_dir() {
                continue;
            }
            let (v, z) = (e.file_name(), e.path());
            let (n, f) = (v.to_string_lossy(), z.to_string_lossy());
            let i = ok_or_continue!(u64::from_str_radix(&n, 10));
            let (mut x, p) = read_status(&f).unwrap_or_default();
            if let Ok(t) = read_args(&f) {
                if !t.is_empty() {
                    x = t
                }
            }
            if x.is_empty() {
                continue;
            }
            r.push(ProcessEntry {
                pid:     i as u32,
                ppid:    p,
                user:    user_info(m.uid(), |p| {
                    unsafe { CStr::from_ptr(p.pw_name) }.to_string_lossy().to_string()
                })
                .unwrap_or_default(),
                cmdline: x,
            })
        }
        Ok(r)
    }
    pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
        let mut b = String::with_capacity(24);
        b.push_str(crypt::get_or(0, "/proc/"));
        let t = unsafe { b.as_mut_vec() };
        pid.into_vec(t);
        t.push(b'/');
        t.push(b't');
        t.push(b'a');
        t.push(b's');
        t.push(b'k');
        t.push(b'/');
        let mut r = Vec::new();
        for i in read_dir(b)? {
            let e = ok_or_continue!(i);
            if !ok_or_continue!(e.metadata()).is_dir() {
                continue;
            }
            r.push(ThreadEntry {
                pid,
                tid: ok_or_continue!(u32::from_str_radix(&e.file_name().to_string_lossy(), 10)),
            })
        }
        Ok(r)
    }

    fn read_args(p: &str) -> io::Result<String> {
        let mut t = String::with_capacity(p.len() + 8);
        t.push_str(p);
        unsafe {
            let b = t.as_mut_vec();
            b.push(b'/');
            b.push(b'c');
            b.push(b'm');
            b.push(b'd');
            b.push(b'l');
            b.push(b'i');
            b.push(b'n');
            b.push(b'e');
        }
        let mut s = String::new();
        let r = unsafe { s.as_mut_vec() };
        for b in fs::read(t)?.split(|v| *v == 0) {
            if b.is_empty() {
                continue;
            }
            if !r.is_empty() {
                r.push(b' ');
            }
            r.extend_from_slice(b);
        }
        Ok(s)
    }
    fn read_status(p: &str) -> io::Result<(String, u32)> {
        let mut t = String::with_capacity(p.len() + 7);
        t.push_str(p);
        unsafe {
            let b = t.as_mut_vec();
            b.push(b'/');
            b.push(b's');
            b.push(b't');
            b.push(b'a');
            b.push(b't');
            b.push(b'u');
            b.push(b's');
        }
        let (mut n, mut p) = (String::new(), 0);
        #[cfg(any(target_os = "solaris", target_os = "illumos"))]
        {
            // NOTE(dij): Solaris '/status' file is binary and cannot be normally
            //            read, however using some RE, the PPID is a u32 at 12b
            //            in the file, so we can read that. Luckily, '/cmdline' is
            //            the same as *nix so we can read that to get the process
            //            name instead.
            let _ = p; // Ignore unused read
            let _ = unsafe { n.as_mut_vec() }; // Ignore unused mut.
            let mut f = fs::OpenOptions::new().read(true).open(t)?;
            // Solaris PPID is 12b in the binary '/status' file.
            crate::io::Seek::seek(&mut f, io::SeekFrom::Start(0xC))?;
            let mut b = [0u8; 4];
            crate::io::Read::read(&mut f, &mut b)?;
            p = u32::from_ne_bytes(b)
        }
        #[cfg(all(not(target_os = "solaris"), not(target_os = "illumos")))]
        {
            for b in fs::read(t)?.split(|v| *v == b'\n') {
                if !n.is_empty() && p > 0 {
                    break;
                }
                if b.len() > 6 && b[0] == b'N' && b[2] == b'm' && b[4] == b':' {
                    for i in 5..b.len() {
                        if b[i] == b' ' || b[i] == 0x9 || b[i] == b'\t' {
                            continue;
                        }
                        unsafe { n.as_mut_vec().extend_from_slice(&b[i..]) };
                        break;
                    }
                }
                if b.len() > 5 && b[0] == b'P' && b[1] == b'P' && b[3] == b'd' && b[4] == b':' {
                    for i in 5..b.len() {
                        if b[i] == b' ' || b[i] == 0x9 || b[i] == b'\t' {
                            continue;
                        }
                        if let Ok(v) = u64::from_str_radix(unsafe { core::str::from_utf8_unchecked(&b[i..]) }, 10) {
                            p = v as u32;
                        }
                        break;
                    }
                }
            }
        }
        Ok((n, p))
    }
}
#[cfg(all(
    all(not(target_os = "netbsd"), not(target_vendor = "apple")),
    any(
        target_os = "hurd",
        target_os = "redox",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "freebsd",
        target_os = "dragonfly",
    )
))]
mod list {
    extern crate libc;

    use core::ffi::{c_void, CStr};
    use core::ptr;
    use core::slice::from_raw_parts;

    use libc::kinfo_proc;

    use crate::device::user_info;
    use crate::io::{self, Error, ErrorKind};
    use crate::prelude::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use crate::process::unix::{ProcessEntry, ThreadEntry};

    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        let f = unsafe { libc::kvm_openfiles(ptr::null(), ptr::null(), ptr::null(), 0, ptr::null_mut()) };
        if f.is_null() {
            return Err(ErrorKind::OutOfMemory.into());
        }
        let mut c = 0i32;
        // KERN_PROC_PROC - 0x8
        let a = unsafe { libc::kvm_getprocs(f, libc::KERN_PROC_PROC, 0, &mut c) };
        if a.is_null() {
            unsafe { libc::kvm_close(f) };
            return Err(Error::last_os_error());
        }
        let mut r = Vec::with_capacity(c as usize);
        if c == 0 {
            unsafe { libc::kvm_close(f) };
            return Ok(r);
        }
        let u = unsafe { from_raw_parts(a, c as usize) };
        for i in u {
            let x = (if !i.ki_args.is_null() { read_args(f, i) } else { None }).unwrap_or_else(|| unsafe { CStr::from_ptr(i.ki_comm.as_ptr()) }.to_string_lossy().to_string());
            r.push(ProcessEntry {
                pid:     i.ki_pid as _,
                ppid:    i.ki_ppid as _,
                user:    user_info(i.ki_uid, |p| {
                    unsafe { CStr::from_ptr(p.pw_name) }.to_string_lossy().to_string()
                })
                .unwrap_or_default(),
                cmdline: x,
            })
        }
        unsafe { libc::kvm_close(f) };
        Ok(r)
    }
    pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
        let f = unsafe { libc::kvm_openfiles(ptr::null(), ptr::null(), ptr::null(), 0, ptr::null_mut()) };
        if f.is_null() {
            return Err(ErrorKind::OutOfMemory.into());
        }
        let mut c = 0i32;
        // KERN_PROC_ALL - 0x0
        let a = unsafe { libc::kvm_getprocs(f, libc::KERN_PROC_ALL, pid as _, &mut c) };
        if a.is_null() {
            unsafe { libc::kvm_close(f) };
            return Err(Error::last_os_error());
        }
        let mut r = Vec::with_capacity(c as usize);
        if c == 0 {
            unsafe { libc::kvm_close(f) };
            return Ok(r);
        }
        let u = unsafe { from_raw_parts(a, c as usize) };
        for i in u {
            if i.ki_pid != pid as _ {
                continue;
            }
            r.push(ThreadEntry {
                tid: i.ki_tid as u32,
                pid: i.ki_pid as u32,
            });
        }
        unsafe { libc::kvm_close(f) };
        Ok(r)
    }

    fn read_args(b: *mut c_void, p: &kinfo_proc) -> Option<String> {
        let a = unsafe { libc::kvm_getargv(b, p, 0) };
        if a.is_null() {
            return None;
        }
        let mut r = String::new();
        let t = unsafe { r.as_mut_vec() };
        let v = unsafe { from_raw_parts(a, 1024) };
        for i in 0..v.len() {
            if v[i].is_null() {
                break;
            }
            if !t.is_empty() {
                t.push(b' ');
            }
            t.extend_from_slice(unsafe { CStr::from_ptr(v[i]) }.to_bytes())
        }
        Some(r)
    }
}

#[cfg(not(target_family = "windows"))]
mod unix {
    use crate::prelude::*;

    pub struct ThreadEntry {
        pub tid: u32,
        pub pid: u32,
    }
    pub struct ProcessEntry {
        pub pid:     u32,
        pub ppid:    u32,
        pub user:    String,
        pub cmdline: String,
    }
}

#[path = "device/filter.rs"]
mod filter;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "device/winapi/std"]
mod inner {
    mod process;
    pub use self::process::*;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg(all(not(target_vendor = "fortanix"), not(target_family = "windows")))]
    extern crate libc;

    use crate::io;
    use crate::prelude::*;
    use crate::process::ChildExtra;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use std::process::*;

    pub trait CommandExtra {
        fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<ExitStatus>;
        fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus>;
    }

    impl ChildExtra for Child {
        fn wait_with_output_in_combo(self, out: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.wait_with_output()?;
            out.reserve(o.stdout.len() + 1 + o.stderr.len());
            if !o.stdout.is_empty() {
                out.extend_from_slice(&o.stdout);
                if !o.stderr.is_empty() {
                    out.push(b'\n');
                }
            }
            if !o.stderr.is_empty() {
                out.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
        fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.wait_with_output()?;
            if !o.stdout.is_empty() {
                stdout.reserve(o.stdout.len());
                stdout.extend_from_slice(&o.stdout);
            }
            if !o.stderr.is_empty() {
                stderr.reserve(o.stderr.len());
                stderr.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
    }
    impl CommandExtra for Command {
        fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.output()?;
            out.reserve(o.stdout.len() + 1 + o.stderr.len());
            if !o.stdout.is_empty() {
                out.extend_from_slice(&o.stdout);
                if !o.stderr.is_empty() {
                    out.push(b'\n');
                }
            }
            if !o.stderr.is_empty() {
                out.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
        fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.output()?;
            if !o.stdout.is_empty() {
                stdout.extend_from_slice(&o.stdout);
            }
            if !o.stderr.is_empty() {
                stderr.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
    }

    #[inline]
    pub fn parent_id() -> u32 {
        #[cfg(target_family = "windows")]
        {
            crate::winapi::current_process_info().map_or(0u32, |i| i.parent_process_id as u32)
        }
        #[cfg(target_vendor = "fortanix")]
        {
            0
        }
        #[cfg(all(not(target_vendor = "fortanix"), not(target_family = "windows")))]
        {
            unsafe { libc::getppid() as u32 }
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::prelude::*;
    use crate::process::{ProcessEntry, ThreadEntry};

    impl Debug for ThreadEntry {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("ThreadEntry")
                .field("tid", &self.tid)
                .field("pid", &self.pid)
                .finish()
        }
    }
    impl Display for ThreadEntry {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(self, f)
        }
    }

    impl Debug for ProcessEntry {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("ProcessEntry")
                .field("pid", &self.pid)
                .field("ppid", &self.ppid)
                .field("user", &self.user)
                .field("cmdline", &self.cmdline)
                .finish()
        }
    }
    impl Display for ProcessEntry {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(self, f)
        }
    }
}
