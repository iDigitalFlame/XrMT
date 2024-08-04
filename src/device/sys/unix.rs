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
#![cfg(all(
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_family = "windows"),
))]

#[cfg_attr(rustfmt, rustfmt_skip)]
#[cfg(not(target_vendor = "fortanix"))]
pub(crate) use inner::user_info;

use core::alloc::Allocator;
use core::cmp;
use core::str::from_utf8_unchecked;

use crate::data::str::{Fiber, MaybeString};
use crate::device::{Evasion, Login};
use crate::env::{split_paths, var_os, PATH};
use crate::fs::{self, exists, File, OpenOptions};
use crate::io::{self, Error, ErrorKind, Read, Seek, SeekFrom, Write};
use crate::path::PathBuf;
use crate::prelude::*;
use crate::process::Filter;
use crate::util::{crypt, ToStr};

pub const SHELL_ARGS: [u8; 2] = [b'-', b'c'];

pub fn is_virtual() -> bool {
    false
}
#[inline]
pub fn is_debugged() -> bool {
    sys::is_debugged()
}
#[inline]
pub fn home_dir() -> Option<PathBuf> {
    match var_os(crypt::get_or(0, "HOME")) {
        Some(d) => Some(d.into()),
        None => inner::home_dir(),
    }
}
#[inline]
pub fn revert_to_self() -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn evade(_flags: Evasion) -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}
pub fn shell_in<A: Allocator>(alloc: A) -> Fiber<A> {
    let b = crypt::get_or(0, "sh");
    if let Some(p) = var_os(unsafe { from_utf8_unchecked(&PATH) }) {
        for i in split_paths(&p) {
            let r = i.join(b);
            if exists(&r) {
                return r.to_string_lossy().into_alloc(alloc);
            }
        }
    }
    b.into_alloc(alloc)
}
#[inline]
pub fn set_critical(_is_critical: bool) -> io::Result<bool> {
    Err(ErrorKind::Unsupported.into())
}
pub fn powershell_in<A: Allocator>(alloc: A) -> Option<Fiber<A>> {
    let b = crypt::get_or(0, "pwsh");
    for i in split_paths(&var_os(unsafe { from_utf8_unchecked(&PATH) })?) {
        let r = i.join(b);
        if exists(&r) {
            return Some(r.to_string_lossy().into_alloc(alloc));
        }
    }
    None
}
#[inline]
pub fn whoami_in<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
    inner::whoami(alloc)
}
#[inline]
pub fn set_process_name(_cmd: impl AsRef<str>) -> io::Result<bool> {
    // TODO(dij): Due to how rust handles the args, we can't easily
    //            grab a pointer to it to change it. Maybe in the future??.
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn hostname_in<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
    inner::hostname(alloc)
}
#[inline]
pub fn impersonate<A: Allocator>(_proc: &Filter<A>) -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn impersonate_thread<A: Allocator>(_proc: &Filter<A>) -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn logins_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Login<A>, A>> {
    let mut l = inner::logins_in(alloc)?;
    l.sort();
    Ok(l)
}
#[inline]
pub fn mounts_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Fiber<A>, A>> {
    let mut m = sys::mounts(alloc)?;
    m.sort();
    Ok(m)
}
pub fn dump_process<A: Allocator>(proc: &Filter<A>, w: &mut impl Write) -> io::Result<usize> {
    // TODO(dij): Unless we find a way do this without /proc/, we'll have to omit
    //            BSD systems that don't auto mount it.
    let p = proc.select_func(None).map_err(|_| Error::from(ErrorKind::NotFound))?;
    let mut s = String::with_capacity(16);
    s.push_str(crypt::get_or(0, "/proc/"));
    p.into_vec(unsafe { s.as_mut_vec() });
    let mut i = s.clone();
    i.push_str(crypt::get_or(0, "/maps"));
    s.push_str(crypt::get_or(0, "/mem"));
    let m = fs::read(i)?;
    let mut f = OpenOptions::new().read(true).open(s)?;
    let (mut t, mut e) = (0, None);
    for i in m.split(|v| *v == b'\n') {
        match dump(i, &mut f, w) {
            Err(v) => {
                e = Some(v);
                break;
            },
            Ok(v) => t += v,
        }
    }
    match e {
        Some(e) => Err(e),
        None => Ok(t),
    }
}
#[inline]
pub fn impersonate_user<U: AsRef<str>, M: MaybeString>(_user: U, _domain: M, _pass: M) -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}
#[inline]
pub fn impersonate_user_network<U: AsRef<str>, M: MaybeString>(_user: U, _domain: M, _pass: M) -> io::Result<()> {
    Err(ErrorKind::Unsupported.into())
}

fn dump(map: &[u8], f: &mut File, w: &mut impl Write) -> io::Result<usize> {
    let mut d = 0;
    while d < map.len() && map[d] != b'-' {
        d += 1;
    }
    let mut s = d + 1;
    while s < map.len() && map[s] != b' ' {
        s += 1;
    }
    if map.len() < s + 21 || map[s + 1] != b'r' {
        return Ok(0);
    }
    let mut x = s + 6;
    while x < map.len() && map[x] != b' ' {
        x += 1;
    }
    x += 1;
    while x < map.len() && map[x] != b' ' {
        x += 1;
    }
    if map[x + 1] == b'0' && (map[x + 2] == b' ' || map[x + 2] == 0x9 || map[x + 2] == b'\t') {
        return Ok(0);
    }
    let v = u64::from_str_radix(unsafe { from_utf8_unchecked(&map[0..d]) }, 16).map_err(|_| Error::from(ErrorKind::InvalidData))? as usize;
    let g = u64::from_str_radix(unsafe { from_utf8_unchecked(&map[d + 1..s]) }, 16).map_err(|_| Error::from(ErrorKind::InvalidData))? as usize;
    let mut b = [0u8; 4096];
    let (mut i, mut t) = (v, 0);
    while i < g {
        let k = cmp::min(g - i, 4096);
        f.seek(SeekFrom::Start(i as _))?;
        let q = f.read(&mut b[0..k])?;
        if q == 0 {
            break;
        }
        t += w.write(&b[0..q])?;
        i += q;
        if i >= g {
            break;
        }
    }
    Ok(t)
}

#[cfg(any(
    target_os = "hurd",
    target_os = "redox",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_vendor = "apple",
))]
pub mod unix {
    extern crate libc;

    use core::ptr;

    use crate::data::blob::Blob;
    use crate::io::{self, Error};
    use crate::prelude::*;

    pub fn sysctl_by_name(name: &str, strip_null: bool) -> io::Result<Blob<u8, 256>> {
        let mut v = name.to_string();
        unsafe { v.as_mut_vec().push(0) };
        let mut n = 0;
        let r = unsafe {
            libc::sysctlbyname(
                v.as_ptr() as *const i8,
                ptr::null_mut(),
                &mut n,
                ptr::null_mut(),
                0,
            )
        };
        if n == 0 {
            return Err(Error::from_raw_os_error(r));
        }
        let mut b: Blob<u8, 256> = Blob::with_size(n);
        let s = unsafe {
            libc::sysctlbyname(
                v.as_ptr() as *const i8,
                b.as_mut_ptr() as *mut _,
                &mut n,
                ptr::null_mut(),
                0,
            )
        };
        // Remove NULL
        if strip_null {
            b.truncate(n - 1)
        }
        if s == -1 {
            Err(Error::from_raw_os_error(s))
        } else {
            Ok(b)
        }
    }
    pub fn sysctl<const N: usize>(mib: [i32; N], strip_null: bool) -> io::Result<Blob<u8, 256>> {
        let (mut m, mut n) = (mib, 0);
        let r = unsafe {
            libc::sysctl(
                m.as_mut_ptr() as _,
                m.len() as u32,
                ptr::null_mut(),
                &mut n,
                ptr::null_mut(),
                0,
            )
        };
        if r != 0 {
            return Err(Error::from_raw_os_error(r));
        }
        let mut b = Blob::with_size(n);
        let s = unsafe {
            libc::sysctl(
                m.as_mut_ptr() as _,
                m.len() as u32,
                b.as_mut_ptr() as _,
                &mut n,
                ptr::null_mut(),
                0,
            )
        };
        // Remove NULL
        if strip_null {
            b.truncate(n - 1)
        }
        if s != 0 {
            Err(Error::from_raw_os_error(s))
        } else {
            Ok(b)
        }
    }
}

#[cfg(any(
    target_os = "hurd",
    target_os = "redox",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_vendor = "apple",
))]
mod sys {
    extern crate libc;

    use core::alloc::Allocator;
    use core::ffi::CStr;
    use core::slice::from_raw_parts;
    use core::{cmp, ptr};

    use crate::data::blob::Blob;
    use crate::data::str::Fiber;
    use crate::device::unix::sysctl;
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[cfg(target_os = "netbsd")]
    #[inline]
    pub(super) fn is_debugged() -> bool {
        // 0x1  - KERN_PROC_PID
        // 0x2F - KERN_PROC2
        // 0x1  - CTL_KERN
        sysctl(
            [
                libc::KERN_PROC_PID,
                libc::KERN_PROC2,
                libc::CTL_KERN,
                unsafe { libc::getpid() },
                core::mem::size_of::<libc::kinfo_proc2>() as i32,
                1,
            ],
            false,
        )
        .map_or(false, |v| {
            unsafe { v.as_ptr_of::<libc::kinfo_proc2>().as_ref() }.map_or(false, |r| {
                // Check for the P_TRACED flag, '0x800' or if any traces are set by
                // the kernel.
                r.p_flag & 0x800 != 0 || r.p_tracep > 0 || r.p_traceflag > 0
            })
        })
    }
    #[cfg(not(target_os = "netbsd"))]
    #[inline]
    pub(super) fn is_debugged() -> bool {
        // 0x1 - KERN_PROC_PID
        // 0xE - KERN_PROC
        // 0x1 - CTL_KERN
        let r = sysctl(
            [libc::KERN_PROC_PID, libc::KERN_PROC, libc::CTL_KERN, unsafe {
                libc::getpid()
            }],
            false,
        );
        #[cfg(target_vendor = "apple")]
        {
            // NOTE(dij): It seems that the returned kproc_info struct (or whatever
            //            it is, I'm not sure tbh) has a u32 flag at offset 0x20.
            //            ONLY when under a debugger is the 7th bit (0x80/128) ever
            //            set! This has to be some kind of undocumented debug flag
            //            as I can't find it anywhere.
            //
            // See https://stackoverflow.com/questions/2200277/detecting-debugger-on-mac-os-x
            //     https://github.com/apple-opensource/xnu/blob/24525736ba5b8a67ce3a8a017ced469abe101ad5/bsd/sys/proc.h
            //     https://opensource.apple.com/source/xnu/xnu-3789.1.32/bsd/sys/proc_info.h.auto.html
            //
            r.map_or(false, |v| {
                v.len() > 36 && u32::from_ne_bytes(crate::data::s2a_u32(&v[32..36])) & 0x80 != 0
            })
        }
        #[cfg(all(not(target_os = "netbsd"), not(target_vendor = "apple")))]
        {
            r.map_or(false, |v| {
                unsafe { v.as_ptr_of::<libc::kinfo_proc>().as_ref() }.map_or(false, |r| r.ki_tracer > 0 || r.ki_traceflag > 0)
            })
        }
    }
    pub(super) fn mounts<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Fiber<A>, A>> {
        // 0x2 - MNT_NOWAIT
        let e = unsafe {
            #[cfg(target_os = "netbsd")]
            {
                libc::getvfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT)
            }
            #[cfg(not(target_os = "netbsd"))]
            {
                libc::getfsstat(ptr::null_mut(), 0, libc::MNT_NOWAIT)
            }
        } as usize;
        if e == 0 {
            return Err(ErrorKind::NotFound.into());
        }
        // 0x928 is the real size of 'statfs' but in older BSD versions it's 0x1D8.
        // But this shouldn't matter as it's just the buffer. We'll deal with that
        // gap lower down.
        let s = e * 0x928;
        let mut b: Blob<u8, 512, A> = Blob::with_size_in(s, alloc.clone());
        // 0x2 - MNT_NOWAIT
        let r = unsafe {
            #[cfg(target_os = "netbsd")]
            {
                libc::getvfsstat(b.as_mut_ptr() as _, s as _, libc::MNT_NOWAIT)
            }
            #[cfg(not(target_os = "netbsd"))]
            {
                libc::getfsstat(b.as_mut_ptr() as _, s as _, libc::MNT_NOWAIT)
            }
        } as usize;
        // This version number means that the sizes of the name arrays are 0x400
        // instead of 0x58.
        //
        // We set 't' to the size of one entry, and 'r' to the offset of 'f_mntonname'.
        //
        // Technically (s/t) == c.
        //
        // MAC IS DIFFERENT!
        // It's array size is still 0x400, but struct size is still off, so we'll
        // get it automatically. The offset is fixed at 0x58 at least.
        let (t, n, z) = {
            #[cfg(target_vendor = "apple")]
            {
                (core::mem::size_of::<libc::statfs>(), 0x58, 0x400)
            }
            #[cfg(target_os = "netbsd")]
            {
                (core::mem::size_of::<libc::statvfs>(), 0xCC, 0x400)
            }
            #[cfg(all(not(target_os = "netbsd"), not(target_vendor = "apple")))]
            {
                if (*b.as_ref_of::<u32>()) == 0x20140518 {
                    (0x928, 0x528, 0x400)
                } else {
                    (0x1D8, 0x180, 0x058)
                }
            }
        };
        let q = cmp::min(r, s / t);
        let mut o = Vec::with_capacity_in(q, alloc.clone());
        for i in 0..q {
            let x = unsafe { b.as_ptr().add(i * t) };
            if let Ok(v) = CStr::from_bytes_until_nul(unsafe { from_raw_parts(x.add(n), z) }) {
                o.push(v.to_bytes().into_alloc(alloc.clone()))
            }
        }
        Ok(o)
    }
}
#[cfg(all(
    not(target_os = "hurd"),
    not(target_os = "redox"),
    not(target_os = "netbsd"),
    not(target_os = "openbsd"),
    not(target_os = "freebsd"),
    not(target_os = "dragonfly"),
    not(target_vendor = "apple"),
))]
mod sys {
    use core::alloc::Allocator;

    use crate::data::str::Fiber;
    use crate::prelude::*;
    use crate::util::crypt;
    use crate::{fs, io, ok_or_return};

    pub(super) fn is_debugged() -> bool {
        #[cfg(any(target_os = "solaris", target_os = "illumos"))]
        {
            // NOTE(dij): Solaris '/status' file is binary and cannot be normally
            //            read, however using some RE, there's a section of the file
            //            that gets set to [254, 255, 255, 255] when a process is
            //            being debugged or is attached to.
            //
            //            When debugging the debugger (lol) I found that Solaris
            //            tries to open the proc dir '/proc/<pid>' with the following
            //            open options 'O_RDONLY|O_CLOEXEC|O_DIRECTORY|O_XPG4OPEN'
            //            which would return 'EBUSY' when it's already attached.
            //            That's another stable way to do it, but reading out
            //            status seems more 'stealthy'.
            let mut f = ok_or_return!(
                fs::OpenOptions::new()
                    .read(true)
                    .open(crypt::get_or(0, "/proc/self/status")),
                false
            );
            // Solaris "Debug Flagset" is 240b in the binary '/status' file.
            if crate::io::Seek::seek(&mut f, io::SeekFrom::Start(0xF0)).is_err() {
                return false;
            }
            let mut b = [0u8; 4];
            if crate::io::Read::read(&mut f, &mut b).is_ok() {
                return b[1] == 0xFF && b[2] == 0xFF && b[3] == 0xFF;
            }
        }
        #[cfg(all(not(target_os = "solaris"), not(target_os = "illumos")))]
        {
            let b = ok_or_return!(fs::read(crypt::get_or(0, "/proc/self/status")), false);
            for i in b.split(|v| *v == b'\n') {
                if i.len() <= 9 {
                    continue;
                }
                if i[0] == b'T' && i[9] == b':' && i[8] == b'd' && i[1] == b'r' && i[5] == b'r' {
                    let n = i.len();
                    match i[n - 2] {
                        b' ' | b'\t' => (),
                        _ if i[n - 1] != b'0' => return true,
                        _ => (),
                    }
                    break;
                }
            }
        }
        false
    }
    pub(super) fn mounts<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Fiber<A>, A>> {
        let d = fs::read(crypt::get_or(0, "/proc/self/mounts"))
            .or_else(|_| fs::read(crypt::get_or(0, "/etc/mtab")))
            .or_else(|_| fs::read(crypt::get_or(0, "/etc/mnttab")))?;
        let mut r = Vec::new_in(alloc.clone());
        for i in d.split(|v| *v == b'\n') {
            if i.is_empty() {
                continue;
            }
            let (mut x, mut e) = (0, 0);
            for s in 0..2 {
                e = x;
                while x < i.len() - 1 && i[x] != b' ' && i[x] != b'\t' && i[x] != b'\x0C' {
                    x += 1
                }
                if x < i.len() - 1 && s == 0 {
                    x += 1
                }
            }
            if x == e {
                continue;
            }
            r.push((&i[e..x]).into_alloc(alloc.clone()))
        }
        Ok(r)
    }
}

#[cfg(all(
    not(target_os = "android"),
    not(target_os = "fuchsia"),
    not(target_vendor = "fortanix"),
))]
mod who {
    extern crate libc;

    use core::alloc::Allocator;
    use core::ffi::CStr;

    use libc::utmpx;

    use crate::data::time::Time;
    use crate::device::Login;
    use crate::io;
    use crate::prelude::*;

    #[cfg(any(target_env = "musl", target_env = "ohos"))]
    const USER_PROCESS: i16 = 7i16;
    #[cfg(all(not(target_env = "musl"), not(target_env = "ohos")))]
    const USER_PROCESS: i16 = libc::USER_PROCESS as _;

    #[cfg(any(target_env = "musl", target_env = "ohos"))]
    #[repr(C)]
    struct User {
        ut_type:    i16,
        ut_pid:     i32,
        ut_line:    [i8; 32],
        ut_id:      [i8; 4],
        ut_user:    [i8; 32],
        ut_host:    [i8; 256],
        pad1:       u32,
        pad2:       u32,
        ut_tv:      [i32; 2],
        ut_addr_v6: [i32; 4],
        pad3:       [u8; 20],
    }

    pub(super) fn logins_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Login<A>, A>> {
        let mut r = Vec::new_in(alloc.clone());
        #[cfg(any(target_env = "musl", target_env = "ohos"))]
        {
            let d = crate::fs::read(crate::util::crypt::get_or(0, "/var/run/utmp"))?;
            let (b, mut i, z) = (d.as_ptr(), 0, 0x180);
            r.reserve_exact(d.len() / z);
            loop {
                if i * z >= d.len() || z * (i + 1) > d.len() {
                    break;
                }
                match unsafe { (b.add(i * z) as *const User).as_ref() } {
                    None => break,
                    Some(u) => {
                        let t = convert_inner(u);
                        if let Some(v) = convert(&t, alloc.clone()) {
                            r.push(v)
                        }
                    },
                }
                i += 1;
            }
        }
        #[cfg(all(not(target_env = "musl"), not(target_env = "ohos")))]
        {
            loop {
                match unsafe { libc::getutxent().as_ref() } {
                    None => break,
                    Some(e) => {
                        if let Some(v) = convert(e, alloc.clone()) {
                            r.push(v)
                        }
                    },
                }
            }
        }
        Ok(r)
    }

    #[cfg(any(target_env = "musl", target_env = "ohos"))]
    #[inline]
    fn convert_inner(u: &User) -> utmpx {
        let mut r: utmpx = unsafe { core::mem::zeroed() };
        r.ut_id = unsafe { core::mem::transmute_copy(&u.ut_id) };
        r.ut_pid = u.ut_pid;
        r.ut_type = u.ut_type;
        r.ut_line = unsafe { core::mem::transmute_copy(&u.ut_line) };
        r.ut_user = unsafe { core::mem::transmute_copy(&u.ut_user) };
        r.ut_host = unsafe { core::mem::transmute_copy(&u.ut_host) };
        r.ut_addr_v6 = unsafe { core::mem::transmute_copy(&u.ut_addr_v6) };
        r.ut_tv.tv_sec = u.ut_tv[0] as _;
        r.ut_tv.tv_usec = u.ut_tv[1] as _;
        // The transmute blocks are here to allow for targets that use 'u8' instead
        // of 'i8'.
        r
    }
    fn convert<A: Allocator + Clone>(v: &utmpx, alloc: A) -> Option<Login<A>> {
        if v.ut_type as i16 != USER_PROCESS {
            return None;
        }
        let u = {
            #[cfg(target_os = "netbsd")]
            {
                v.ut_name.as_ptr()
            }
            #[cfg(not(target_os = "netbsd"))]
            {
                v.ut_user.as_ptr()
            }
        };
        Some(Login {
            id:         v.ut_pid as _,
            user:       unsafe { CStr::from_ptr(u as _) }.to_bytes().into_alloc(alloc.clone()),
            host:       unsafe { CStr::from_ptr(v.ut_line.as_ptr() as _) }
                .to_bytes()
                .into_alloc(alloc.clone()),
            from:       super::addr::convert(v).unwrap_or_default(),
            status:     0,
            last_input: Time::ZERO,
            login_time: Time::from_unix(v.ut_tv.tv_sec as i64, v.ut_tv.tv_usec as i64),
        })
    }
}
#[cfg(target_vendor = "fortanix")]
mod who {
    use core::alloc::Allocator;

    use crate::device::Login;
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[inline]
    pub(super) fn logins_in<A: Allocator + Clone>(_alloc: A) -> io::Result<Vec<Login<A>, A>> {
        Err(ErrorKind::Unsupported.into())
    }
}

#[cfg(all(
    not(target_os = "fuchsia"),
    not(target_os = "android"),
    not(target_vendor = "fortanix"),
    any(
        target_os = "hurd",
        target_os = "redox",
        target_os = "netbsd",
        target_os = "illumos",
        target_os = "openbsd",
        target_os = "fuchsia",
        target_os = "freebsd",
        target_os = "solaris",
        target_os = "dragonfly",
        target_vendor = "apple",
    )
))]
mod addr {
    extern crate libc;

    use core::ffi::CStr;
    use core::net::IpAddr;

    use libc::utmpx;

    use crate::device::Address;
    use crate::prelude::*;

    #[inline]
    pub(super) fn convert(v: &utmpx) -> Option<Address> {
        Some(
            unsafe { CStr::from_ptr(v.ut_host.as_ptr() as _) }
                .to_str()
                .ok()?
                .parse::<IpAddr>()
                .ok()?
                .into(),
        )
    }
}
#[cfg(all(
    not(target_os = "redox"),
    not(target_os = "netbsd"),
    not(target_os = "android"),
    not(target_os = "illumos"),
    not(target_os = "openbsd"),
    not(target_os = "fuchsia"),
    not(target_os = "freebsd"),
    not(target_os = "solaris"),
    not(target_os = "dragonfly"),
    not(target_vendor = "apple"),
    not(target_vendor = "fortanix"),
))]
mod addr {
    extern crate libc;

    use core::mem::transmute;

    use libc::utmpx;

    use crate::device::Address;
    use crate::prelude::*;

    #[inline]
    pub(super) fn convert(v: &utmpx) -> Option<Address> {
        Some(Address::from(unsafe {
            transmute::<_, [u8; 16]>(v.ut_addr_v6)
        }))
    }
}

#[cfg(target_vendor = "fortanix")]
mod inner {
    use core::alloc::Allocator;

    use crate::data::str::Fiber;
    use crate::device::Login;
    use crate::io::{self, ErrorKind};
    use crate::path::PathBuf;
    use crate::prelude::*;

    #[inline]
    pub(super) fn home_dir() -> Option<PathBuf> {
        None
    }
    #[inline]
    pub(super) fn whoami<A: Allocator>(_alloc: A) -> io::Result<Fiber<A>> {
        Err(ErrorKind::Unsupported.into())
    }
    #[inline]
    pub(super) fn hostname<A: Allocator>(_alloc: A) -> io::Result<Fiber<A>> {
        Err(ErrorKind::Unsupported.into())
    }
    #[inline]
    pub(super) fn logins_in<A: Allocator + Clone>(_alloc: A) -> io::Result<Vec<Login<A>, A>> {
        Err(ErrorKind::Unsupported.into())
    }
}
#[cfg(not(target_vendor = "fortanix"))]
mod inner {
    extern crate libc;

    use core::alloc::Allocator;
    use core::ffi::CStr;
    use core::mem::{transmute, zeroed};
    use core::{cmp, ptr};

    use libc::passwd;

    use crate::data::blob::Blob;
    use crate::data::str::Fiber;
    use crate::device::Login;
    use crate::io::{self, Error, ErrorKind};
    use crate::path::PathBuf;
    use crate::prelude::*;

    #[inline]
    pub(super) fn home_dir() -> Option<PathBuf> {
        user_info(unsafe { libc::getuid() }, |p| {
            PathBuf::from(unsafe { CStr::from_ptr(p.pw_dir) }.to_string_lossy().to_string())
        })
        .ok()
    }
    #[inline]
    pub(super) fn whoami<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
        Ok(user_info(unsafe { libc::getuid() }, |p| {
            unsafe { CStr::from_ptr(p.pw_name) }.to_bytes().into_alloc(alloc)
        })?)
    }
    #[inline]
    pub(super) fn hostname<A: Allocator>(alloc: A) -> io::Result<Fiber<A>> {
        // 0x48 - _SC_HOST_NAME_MAX
        let n = unsafe {
            cmp::max(
                64,
                cmp::min(libc::sysconf(libc::_SC_HOST_NAME_MAX) as usize, 256),
            )
        };
        let mut b: Blob<_, 256> = Blob::with_size(n);
        let r = unsafe { libc::gethostname(b.as_mut_ptr(), n) };
        if r == 0 {
            Ok(unsafe { transmute::<_, &[u8]>(&b[0..b.iter().position(|&v| v == 0).unwrap_or(n)]) }.into_alloc(alloc))
        } else {
            Err(Error::from_raw_os_error(r))
        }
    }
    #[cfg_attr(any(target_os = "fuchsia", target_os = "android"), allow(unused_variables))]
    #[inline]
    pub(super) fn logins_in<A: Allocator + Clone>(alloc: A) -> io::Result<Vec<Login<A>, A>> {
        #[cfg(any(target_os = "fuchsia", target_os = "android"))]
        {
            Err(ErrorKind::Unsupported.into())
        }
        #[cfg(all(not(target_os = "fuchsia"), not(target_os = "android")))]
        {
            super::who::logins_in(alloc)
        }
    }

    pub(crate) fn user_info<T, F: FnOnce(&passwd) -> T>(uid: u32, f: F) -> io::Result<T> {
        let mut n = cmp::max(64, sysconf_size());
        let mut p = unsafe { zeroed() };
        let mut v: *mut passwd = ptr::null_mut();
        let mut b: Blob<_, 256> = Blob::with_size(n);
        loop {
            let r = unsafe { libc::getpwuid_r(uid, &mut p, b.as_mut_ptr(), n, &mut v) };
            match r {
                libc::ERANGE => {
                    n *= 2;
                    b.resize(n);
                    continue;
                },
                0 => {
                    b.truncate(n);
                    return Ok(f(
                        unsafe { v.as_ref() }.ok_or_else(|| Error::from(ErrorKind::InvalidInput))?
                    ));
                },
                _ => return Err(Error::from_raw_os_error(r)),
            }
        }
    }

    #[inline]
    fn sysconf_size() -> usize {
        #[cfg(target_os = "redox")]
        {
            512
        }
        #[cfg(not(target_os = "redox"))]
        {
            // 0x47 - _SC_GETPW_R_SIZE_MAX
            cmp::min(
                unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) as usize },
                2048,
            )
        }
    }
}
