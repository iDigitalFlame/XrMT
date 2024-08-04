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
#![cfg(target_family = "windows")]

use crate::device::winapi::{self, OwnedHandle, Win32Error, Win32Result};
use crate::prelude::*;

pub struct ThreadEntry {
    pub tid: u32,
    pub pid: u32,
    sus:     u8,
}
pub struct ProcessItem {
    pub pid:     u32,
    pub ppid:    u32,
    pub name:    String,
    pub threads: u32,
    session:     i32,
}
pub struct ProcessEntry {
    pub pid:     u32,
    pub ppid:    u32,
    pub user:    String,
    pub cmdline: String,
}

impl ThreadEntry {
    #[inline]
    pub fn is_suspended(&self) -> Win32Result<bool> {
        if self.sus > 0 {
            return Ok(self.sus == 2);
        }
        if winapi::GetCurrentThreadID() == self.tid {
            return Err(Win32Error::InvalidOperation);
        }
        // 0x42 - THREAD_QUERY_INFORMATION | THREAD_SUSPEND_RESUME
        let h = winapi::OpenThread(0x42, false, self.tid)?;
        winapi::SuspendThread(&h)?;
        Ok(winapi::ResumeThread(h)? > 1)
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        winapi::OpenThread(access, false, self.tid)
    }
}
impl ProcessItem {
    pub fn user(&self) -> Win32Result<String> {
        // 0x400 - PROCESS_QUERY_INFORMATION
        // 0x8   - TOKEN_QUERY
        winapi::token_username(winapi::OpenProcessToken(
            winapi::OpenProcess(0x400, false, self.pid)?,
            0x8,
        )?)
    }
    #[inline]
    pub fn command_line(&self) -> Win32Result<String> {
        // 0x1010 - PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION
        let h = self.handle(0x1010)?;
        // Split this to detect and use WoW64 functions instead.
        #[cfg(target_pointer_width = "64")]
        {
            winapi::read_remote_cmdline(h)
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            if winapi::in_wow64_process() {
                winapi::wow::read_remote_cmdline(h)
            } else {
                winapi::read_remote_cmdline(h)
            }
        }
    }

    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        winapi::OpenProcess(access, false, self.pid)
    }
    #[inline]
    pub fn info(&self, access: u32, elevated: bool, session: bool) -> Win32Result<(bool, u32)> {
        self.info_ex(access, elevated, session, false).map(|r| (r.1, r.2))
    }
    pub fn info_ex(&self, access: u32, elevated: bool, session: bool, handle: bool) -> Win32Result<(Option<OwnedHandle>, bool, u32)> {
        if !handle && !elevated && !session {
            return Ok((None, false, 0));
        }
        if !handle && !elevated && session && self.session >= 0 {
            return Ok((None, false, self.session as u32));
        }
        // NOTE(dij): The reason we have an access param is so we can only open
        //            the handle once while we're doing this to "check" if we can
        //            access it with the requested access we want.
        //
        //            When Filters call this function, we do a quick 'Handle' check
        //            to make sure we can open it before adding to the eval list.
        //
        // 0x400 - PROCESS_QUERY_INFORMATION
        let h = winapi::OpenProcess(access | 0x400, false, self.pid)?;
        if !elevated && !session {
            return if !handle {
                Ok((None, false, 0))
            } else {
                Ok((Some(h), false, 0))
            };
        }
        // 0x8     - TOKEN_QUERY
        // 0x20008 - TOKEN_QUERY | TOKEN_READ
        //
        // Try one with less perms then if that fails, try with more.
        let t = winapi::OpenProcessToken(&h, 0x8).or_else(|_| winapi::OpenProcessToken(&h, 0x20008))?;
        let v = if self.session >= 0 {
            self.session as u32
        } else {
            let mut s = 0u32;
            // 0xC - TokenSessionInformation
            winapi::GetTokenInformation(&t, 0xC, &mut s, 4)?;
            s
        };
        let e = if winapi::is_token_elevated(&t) {
            winapi::token_info(t, |u| {
                // Instead of using the syscall here, we hack the SID and look at
                // the SID last sub authority. It should be a NT (0, 0, 0, 0, 0, 5)
                // identifier and only one in the authorities t is the following number.
                //
                // 0x13 - LOCAL SERVICE
                // 0X14 - NETWORK SERVICE
                Ok(!u.user.sid.is_well_known(0x13) && !u.user.sid.is_well_known(0x14))
            })
            .unwrap_or(false)
        } else {
            false
        };
        if !handle {
            Ok((None, e, v))
        } else {
            Ok((Some(h), e, v))
        }
    }
}
impl ProcessEntry {
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        winapi::OpenProcess(access, false, self.pid)
    }
}

impl Copy for ThreadEntry {}
impl Clone for ThreadEntry {
    #[inline]
    fn clone(&self) -> ThreadEntry {
        ThreadEntry {
            sus: self.sus,
            tid: self.tid,
            pid: self.pid,
        }
    }
}

impl Clone for ProcessItem {
    #[inline]
    fn clone(&self) -> ProcessItem {
        ProcessItem {
            pid:     self.pid,
            ppid:    self.ppid,
            name:    self.name.clone(),
            threads: self.threads,
            session: self.session,
        }
    }
}
impl Into<ProcessEntry> for ProcessItem {
    #[inline]
    fn into(self) -> ProcessEntry {
        let u = self.user().unwrap_or_default();
        ProcessEntry {
            // Most times, if we can't resolve the username due to perms, we
            // likely can't read it's cmdline either. so don't try.
            cmdline: if self.pid > 4 && !u.is_empty() {
                self.command_line().unwrap_or(self.name)
            } else {
                self.name
            },
            pid:     self.pid,
            ppid:    self.ppid,
            user:    u,
        }
    }
}

impl Clone for ProcessEntry {
    #[inline]
    fn clone(&self) -> ProcessEntry {
        ProcessEntry {
            pid:     self.pid,
            ppid:    self.ppid,
            user:    self.user.clone(),
            cmdline: self.cmdline.clone(),
        }
    }
}

#[inline]
pub fn list_processes() -> Win32Result<Vec<ProcessItem>> {
    let mut e = inner::list_processes()?;
    e.shrink_to_fit();
    Ok(e)
}
#[inline]
pub fn list_threads(pid: u32) -> Win32Result<Vec<ThreadEntry>> {
    let mut e = inner::list_threads(pid)?;
    e.shrink_to_fit();
    Ok(e)
}

#[cfg(feature = "snap")]
mod inner {
    use core::mem::size_of;

    use crate::device::winapi::{self, ProcessItem, ThreadEntry, Win32Result};
    use crate::prelude::*;

    #[repr(C)]
    pub struct ThreadEntry32 {
        pub size:       u32,
        pub usage:      u32,
        pub thread_id:  u32,
        pub process_id: u32,
        pad1:           [i32; 2],
        pub flags:      u32,
    }
    #[repr(C)]
    pub struct ProcessEntry32 {
        pub size:       u32,
        pub usage:      u32,
        pub process_id: u32,
        pub heap_id:    usize,
        pub module_id:  u32,
        pub threads:    u32,
        pub parent_id:  u32,
        pub class_base: i32,
        pub flags:      u32,
        pub exe_file:   [u16; 260],
    }

    impl Default for ThreadEntry32 {
        #[inline]
        fn default() -> ThreadEntry32 {
            ThreadEntry32 {
                pad1:       [0i32; 2],
                size:       0x1C,
                flags:      0u32,
                usage:      0u32,
                thread_id:  0u32,
                process_id: 0u32,
            }
        }
    }
    impl Default for ProcessEntry32 {
        #[inline]
        fn default() -> ProcessEntry32 {
            ProcessEntry32 {
                size:       size_of::<ProcessEntry32>() as u32,
                flags:      0u32,
                usage:      0u32,
                threads:    0u32,
                heap_id:    0usize,
                module_id:  0u32,
                parent_id:  0u32,
                class_base: 0i32,
                process_id: 0u32,
                exe_file:   [0u16; 260],
            }
        }
    }

    impl From<&ThreadEntry32> for ThreadEntry {
        #[inline]
        fn from(v: &ThreadEntry32) -> ThreadEntry {
            ThreadEntry {
                sus: 0u8,
                tid: v.thread_id,
                pid: v.process_id,
            }
        }
    }
    impl From<&ProcessEntry32> for ProcessItem {
        #[inline]
        fn from(v: &ProcessEntry32) -> ProcessItem {
            ProcessItem {
                pid:     v.process_id,
                ppid:    v.parent_id,
                name:    winapi::utf16_to_str_trim(&v.exe_file),
                session: -1i32,
                threads: v.threads,
            }
        }
    }

    pub fn list_processes() -> Win32Result<Vec<ProcessItem>> {
        // 0x2 - TH32CS_SNAPPROCESS
        let h = winapi::CreateToolhelp32Snapshot(0x2, 0)?;
        let mut r = Vec::new();
        let mut e = ProcessEntry32::default();
        if let Err(x) = winapi::Process32First(&h, &mut e) {
            // 0x12 - ERR_NO_MORE_FILES
            return if x.code() == 0x12 { Ok(r) } else { Err(x) };
        }
        loop {
            r.push((&e).into());
            if let Err(x) = winapi::Process32Next(&h, &mut e) {
                // 0x12 - ERR_NO_MORE_FILES
                return if x.code() == 0x12 { Ok(r) } else { Err(x) };
            }
        }
    }
    pub fn list_threads(pid: u32) -> Win32Result<Vec<ThreadEntry>> {
        // 0x4 - TH32CS_SNAPTHREAD
        let h = winapi::CreateToolhelp32Snapshot(0x4, 0)?;
        let mut r = Vec::new();
        let mut e = ThreadEntry32::default();
        if let Err(x) = winapi::Thread32First(&h, &mut e) {
            // 0x12 - ERR_NO_MORE_FILES
            return if x.code() == 0x12 { Ok(r) } else { Err(x) };
        }
        loop {
            if e.process_id == pid {
                r.push((&e).into())
            }
            if let Err(x) = winapi::Thread32Next(&h, &mut e) {
                // 0x12 - ERR_NO_MORE_FILES
                return if x.code() == 0x12 { Ok(r) } else { Err(x) };
            }
        }
    }
}
#[cfg(not(feature = "snap"))]
mod inner {
    use core::mem::size_of;
    use core::ptr;

    use crate::device::winapi::loader::ntdll;
    use crate::device::winapi::{self, ClientID, ProcessItem, ThreadEntry, UnicodeString, Win32Result};
    use crate::prelude::*;

    #[repr(C)]
    struct ThreadInfo {
        pad1:          [u8; 28],
        start_address: usize,
        client_id:     ClientID,
        pad2:          [u8; 12],
        thread_state:  u32,
        wait_reason:   u32,
        pad3:          u32,
    }
    #[repr(C)]
    struct ProcessInfo {
        next_entry:        u32,
        thread_count:      u32,
        pad1:              [i64; 6],
        image_name:        UnicodeString,
        pad2:              i32,
        process_id:        usize,
        parent_process_id: usize,
        pad3:              u32,
        session_id:        u32,
        pad4:              [u8; (winapi::PTR_SIZE * 13) + 48],
    }
    struct ThreadIter<'a> {
        buf:   &'a Vec<u8>,
        pos:   usize,
        count: u32,
        total: u32,
    }
    struct ProcessIter<'a> {
        buf:  &'a Vec<u8>,
        pos:  usize,
        next: usize,
    }

    impl ProcessInfo {
        #[inline]
        fn iter<'a>(buf: &'a Vec<u8>) -> ProcessIter<'a> {
            ProcessIter { buf, pos: 0, next: 0 }
        }

        #[inline]
        fn threads<'b>(&self, pos: usize, buf: &'b Vec<u8>) -> ThreadIter<'b> {
            ThreadIter {
                buf,
                pos: pos + size_of::<ProcessInfo>(),
                count: 0u32,
                total: self.thread_count,
            }
        }
    }

    impl<'a> Iterator for ThreadIter<'a> {
        type Item = &'a ThreadInfo;

        #[inline]
        fn next(&mut self) -> Option<&'a ThreadInfo> {
            if self.count == self.total {
                return None;
            }
            let i = unsafe {
                (self
                    .buf
                    .as_ptr()
                    .add(self.pos + (size_of::<ThreadInfo>() * self.count as usize)) as *const ThreadInfo)
                    .as_ref()?
            };
            self.count += 1;
            Some(i)
        }
    }
    impl<'a> Iterator for ProcessIter<'a> {
        type Item = (&'a ProcessInfo, usize);

        #[inline]
        fn next(&mut self) -> Option<(&'a ProcessInfo, usize)> {
            if self.next == 0 && self.pos > 0 {
                return None;
            }
            self.pos += self.next;
            let p = unsafe { (self.buf.as_ptr().add(self.pos) as *const ProcessInfo).as_ref()? };
            self.next = p.next_entry as usize;
            if p.process_id > 0 {
                Some((p, self.pos))
            } else {
                self.next()
            }
        }
    }
    impl<'a> ExactSizeIterator for ThreadIter<'_> {
        #[inline]
        fn len(&self) -> usize {
            self.total as usize
        }
    }

    impl From<&ThreadInfo> for ThreadEntry {
        #[inline]
        fn from(v: &ThreadInfo) -> ThreadEntry {
            ThreadEntry {
                tid: v.client_id.thread as u32,
                pid: v.client_id.process as u32,
                sus: if v.thread_state == 5 && v.wait_reason == 5 {
                    2u8
                } else {
                    1u8
                },
            }
        }
    }
    impl From<&ProcessInfo> for ProcessItem {
        #[inline]
        fn from(v: &ProcessInfo) -> ProcessItem {
            ProcessItem {
                pid:     v.process_id as u32,
                ppid:    v.parent_process_id as u32,
                name:    v.image_name.to_string(),
                threads: v.thread_count,
                session: v.session_id as i32,
            }
        }
    }

    pub(super) fn list_processes() -> Win32Result<Vec<ProcessItem>> {
        winapi::init_ntdll();
        let func = unsafe {
            winapi::make_syscall!(
                *ntdll::NtQuerySystemInformation,
                extern "stdcall" fn(u32, *mut u8, u32, *mut u32) -> u32
            )
        };
        let mut s = 0u32;
        // 0x5 - SystemProcessInformation
        let r = func(0x5, ptr::null_mut(), 0, &mut s);
        if s == 0 {
            return Err(winapi::nt_error(r));
        }
        let mut b = Vec::with_capacity(s as usize * 2);
        b.resize(s as usize * 2, 0);
        // 0x5 - SystemProcessInformation
        let r = func(0x5, b.as_mut_ptr(), s * 2, &mut s);
        if r > 0 {
            return Err(winapi::nt_error(r));
        }
        Ok(ProcessInfo::iter(&b).map(|(v, _)| v.into()).collect::<Vec<ProcessItem>>())
    }
    pub(super) fn list_threads(pid: u32) -> Win32Result<Vec<ThreadEntry>> {
        winapi::init_ntdll();
        let func = unsafe {
            winapi::make_syscall!(
                *ntdll::NtQuerySystemInformation,
                extern "stdcall" fn(u32, *mut u8, u32, *mut u32) -> u32
            )
        };
        let mut s = 0u32;
        // 0x5 - SystemProcessInformation
        let r = func(0x5, ptr::null_mut(), 0, &mut s);
        if s == 0 {
            return Err(winapi::nt_error(r));
        }
        let mut b = Vec::with_capacity(s as usize * 2);
        b.resize(s as usize * 2, 0);
        // 0x5 - SystemProcessInformation
        let r = func(0x5, b.as_mut_ptr(), s * 2, &mut s);
        if r > 0 {
            return Err(winapi::nt_error(r));
        }
        match ProcessInfo::iter(&b).find(|(v, _)| v.process_id as u32 == pid) {
            Some((p, i)) => Ok(p.threads(i, &b).map(|v| v.into()).collect::<Vec<ThreadEntry>>()),
            None => Err(winapi::Win32Error::FileNotFound),
        }
    }
}
