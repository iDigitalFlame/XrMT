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
#![cfg(windows)]

use core::mem::MaybeUninit;
use core::{mem, ptr};

use crate::data::blob::Blob;
use crate::device::winapi::{self, Handle, OwnedHandle, StringBlock, TokenUser, UnicodeString, WCharPtr, Win32Error, Win32Result};
use crate::util::stx::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::*;

pub enum StartInfo<'a> {
    None,
    Basic(&'a StartupInfo),
    Extended(&'a StartupInfoEx),
}

#[repr(C)]
pub struct PEB {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    pub mutant:                   usize,
    pub image_base_address:       Handle,
    pub ldr:                      *mut LoadList,
    pub process_parameters:       *mut ProcessParams,
    pub sub_system_data:          usize,
    pub process_heap:             usize,
    pad1:                         usize,
    pub alt_thunk_list_ptr:       usize,
    pad2:                         usize,
    pub cross_process_flags:      u32,
    pub kernel_callback_table:    usize,
    pad3:                         u32,
    pub alt_thunk_list_ptr32:     u32,
    pub api_map:                  usize,
    pub tls_ex_count:             u32,
    pub tls_bitmap:               usize,
    pub tls_bitmap_bits:          [u32; 2],
    pub readonly_shared_base:     usize,
    pub hotpatch_into:            usize,
    pad4:                         usize,
    pub ansi_code_page:           usize,
    pub oem_code_page:            usize,
    pub unicode_case_table:       usize,
    pub number_of_processors:     u32,
    pub nt_global_flag:           u32,
    pad5:                         u64,
    pad6:                         [usize; 4],
    pub number_of_heaps:          u32,
    pub max_heaps:                u32,
    pub process_heaps:            usize,
    pad7:                         [usize; 2],
    pad8:                         u32,
    pub loader_lock:              usize,
    pub os_major_version:         u32,
    pub os_minor_version:         u32,
    pub os_build:                 u16,
    pub os_csd_version:           u16,
    pub os_platform_id:           u32,
    pub image_subsystem:          u32,
    pub image_subsystem_major:    u32,
    pub image_subsystem_minor:    u32,
    pub process_affinity_mask:    usize,
    pad9:                         [u32; if winapi::PTR_SIZE == 8 { 60 } else { 34 }],
    pad10:                        usize,
    pub tls_ex_bitmap:            usize,
    pub tls_ex_bitmap_bits:       [u32; 32],
    pub session_id:               u32,
    pub appcompat_flags:          u64,
    pub appcompat_flags_user:     u64,
    pub shim_data:                usize,
    pub app_compat:               usize,
    pub csd_version:              UnicodeString,
    pad12:                        [usize; 4],
    pub min_stack_commit:         usize,
    pad13:                        [usize; 3],
    pad14:                        [u32; 4],
    pad15:                        u32,
    pad16:                        [usize; 3],
    pub image_header_hash:        usize,
}
#[repr(C)]
pub struct LoadList {
    pad1:            [u8; 8],
    pad2:            [usize; 3],
    pub module_list: LoaderEntry,
}
#[repr(C)]
pub struct ClientID {
    pub process: usize,
    pub thread:  usize,
}
#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct ThreadEntry {
    pub thread_id:  u32,
    pub process_id: u32,
    sus:            u8,
}
#[repr(C)]
pub struct StartupInfo {
    pub size:           u32,
    pad1:               *const u16,
    pub desktop:        WCharPtr,
    pub title:          WCharPtr,
    pub pos_x:          u32,
    pub pos_y:          u32,
    pub size_x:         u32,
    pub size_y:         u32,
    pub count_chars_x:  u32,
    pub count_chars_y:  u32,
    pub fill_attribute: u32,
    pub flags:          u32,
    pub show_window:    u16,
    pad2:               u16,
    pad3:               *const u8,
    pub stdin:          usize,
    pub stdout:         usize,
    pub stderr:         usize,
}
#[repr(C)]
pub struct ProcessInfo {
    pub process:    OwnedHandle,
    pub thread:     OwnedHandle,
    pub process_id: u32,
    pub thread_id:  u32,
}
#[repr(C)]
pub struct LoaderEntry {
    pad1:            usize,
    pub f_link:      *mut LoaderEntry,
    pub b_link:      *mut LoaderEntry,
    pub links:       usize,
    pub dll_base:    Handle,
    pub entry_point: Handle,
    pub image_size:  usize,
    pub full_name:   UnicodeString,
    pub base_name:   UnicodeString,
    pub flags:       usize,
}
#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct ProcessEntry {
    pub name:         String,
    pub process_id:   u32,
    pub parent_id:    u32,
    pub thread_count: u32,
    session:          i32,
}
#[repr(C)]
pub struct ProcessParams {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           Handle,
    pub console_flags:     u32,
    pub standard_input:    Handle,
    pub standard_output:   Handle,
    pub standard_error:    Handle,
    pub current_directory: CurrentDirectory,
    pub dll_path:          UnicodeString,
    pub image_name:        UnicodeString,
    pub command_line:      UnicodeString,
    pub environment:       StringBlock,
    pub start_x:           u32,
    pub start_y:           u32,
    pub count_x:           u32,
    pub count_y:           u32,
    pub count_chars_x:     u32,
    pub count_chars_y:     u32,
    pub fill_attribute:    u32,
    pub window_flags:      u32,
    pub show_window_flags: u32,
    pub window_title:      UnicodeString,
    pub desktop_info:      UnicodeString,
    pub shell_info:        UnicodeString,
    pub runtime_data:      UnicodeString,
    pub directories:       [u8; (12 + winapi::PTR_SIZE) * 32],
    pub environment_size:  usize,
    pub package_dep_data:  usize,
    pub process_group_id:  u32,
    pub loader_threads:    u32,
}
#[repr(C)]
pub struct StartupInfoEx {
    pub info:  StartupInfo,
    pub attrs: *const ProcessThreadAttrList,
}
#[repr(C)]
pub struct ThreadBasicInfo {
    pub exit_status: u32,
    pub teb_base:    usize,
    pub client_id:   ClientID,
    pad1:            u64,
    pad2:            u32,
}
#[repr(C)]
pub struct ProcessBasicInfo {
    pub exit_status:       u32,
    pub peb_base:          *mut PEB,
    pad1:                  usize,
    pad2:                  u32,
    pub process_id:        usize,
    pub parent_process_id: usize,
}
#[repr(C)]
pub struct CurrentDirectory {
    pub dos_path: UnicodeString,
    pub handle:   Handle,
}
#[repr(C)]
pub struct ProcessThreadAttr {
    attr:  usize,
    size:  usize,
    value: *const usize,
}
#[repr(C)]
pub struct ProcessThreadAttrList {
    mask:  u32,
    size:  u32,
    count: u32,
    pad:   u32,
    unk:   usize,
    attrs: [MaybeUninit<ProcessThreadAttr>; 5],
}

impl ThreadEntry {
    #[inline]
    pub fn is_suspended(&self) -> Win32Result<bool> {
        if self.sus > 0 {
            return Ok(self.sus == 2);
        }
        if winapi::GetCurrentThreadID() == self.thread_id {
            return Err(Win32Error::InvalidOperation);
        }
        // 0x42 - THREAD_QUERY_INFORMATION | THREAD_SUSPEND_RESUME
        let h = winapi::OpenThread(0x42, false, self.thread_id)?;
        winapi::SuspendThread(&h)?;
        Ok(winapi::ResumeThread(h)? > 1)
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        winapi::OpenThread(access, false, self.thread_id)
    }
}
impl ProcessEntry {
    pub fn user(&self) -> Win32Result<String> {
        // 0x400 - PROCESS_QUERY_INFORMATION
        let h = winapi::OpenProcess(0x400, false, self.process_id)?;
        // 0x8 - TOKEN_QUERY
        let t = winapi::OpenProcessToken(h, 0x8)?;
        let mut buf = Blob::new();
        TokenUser::from_token(t, &mut buf)?.user.sid.user()
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        winapi::OpenProcess(access, false, self.process_id)
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
        // 0x400 - PROCESS_QUERY_INFORMATION
        //
        // NOTE(dij): The reason we have an access param is so we can only open
        //            the handle once while we're doing this to "check" if we can
        //            access it with the requested access we want.
        //            When Filters call this function, we do a quick 'Handle' check
        //            to make sure we can open it before adding to the eval list.
        let h = winapi::OpenProcess(access | 0x400, false, self.process_id)?;
        if !elevated && !session {
            if !handle {
                return Ok((None, false, 0));
            }
            return Ok((Some(h), false, 0));
        }
        // 0x2000A - TOKEN_READ | TOKEN_QUERY
        let t = winapi::OpenProcessToken(&h, 0x2000A)?;
        let v = if self.session >= 0 {
            self.session as u32
        } else {
            let mut s: u32 = 0;
            // 0xC - TokenSessionInformation
            winapi::GetTokenInformation(&t, 0xC, &mut s, 4)?;
            s
        };
        let e = if winapi::is_token_elevated(&h) {
            let mut buf = Blob::new();
            // 0x17 - WinLocalServiceSid
            // 0x18 - WinNetworkServiceSid
            TokenUser::from_token(t, &mut buf).map_or(false, |u| {
                u.user.sid.is_well_known(0x17) || u.user.sid.is_well_known(0x18)
            })
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
impl StartInfo<'_> {
    #[inline]
    pub fn is_extended(&self) -> bool {
        match self {
            StartInfo::Extended(_) => true,
            _ => false,
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const usize {
        match self {
            StartInfo::None => ptr::null(),
            StartInfo::Basic(i) => *i as *const StartupInfo as *const usize,
            StartInfo::Extended(i) => *i as *const StartupInfoEx as *const usize,
        }
    }
}
impl ProcessThreadAttrList {
    #[inline]
    pub fn set_parent(&mut self, pos: usize, h: &OwnedHandle) {
        self.set(pos, ProcessThreadAttr {
            size:  winapi::PTR_SIZE,
            attr:  0x20000, // PROC_THREAD_ATTRIBUTE_PARENT_PROCESS
            value: &h.0,
        })
    }
    #[inline]
    pub fn set_mitigation(&mut self, pos: usize, v: *const u64) {
        self.set(pos, ProcessThreadAttr {
            attr:  0x20007, // PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY
            size:  8,
            value: v as *const usize,
        })
    }
    #[inline]
    pub fn set_handles(&mut self, pos: usize, size: usize, v: *const usize) {
        self.set(pos, ProcessThreadAttr {
            attr:  0x20002, // PROC_THREAD_ATTRIBUTE_HANDLE_LIST
            size:  size * winapi::PTR_SIZE,
            value: v,
        })
    }

    #[inline]
    fn set(&mut self, pos: usize, v: ProcessThreadAttr) {
        if pos > 4 {
            return;
        }
        self.size += 1;
        self.count += 1;
        self.mask |= 1 << (v.attr - 0x20000);
        self.attrs[pos].write(v);
    }
}

impl Copy for ThreadEntry {}
impl Clone for ThreadEntry {
    #[inline]
    fn clone(&self) -> ThreadEntry {
        ThreadEntry {
            sus:        self.sus,
            thread_id:  self.thread_id,
            process_id: self.process_id,
        }
    }
}
impl Clone for ProcessEntry {
    #[inline]
    fn clone(&self) -> ProcessEntry {
        ProcessEntry {
            name:         self.name.clone(),
            session:      self.session,
            parent_id:    self.parent_id,
            process_id:   self.process_id,
            thread_count: self.thread_count,
        }
    }
}

impl Default for ProcessInfo {
    #[inline]
    fn default() -> ProcessInfo {
        ProcessInfo {
            thread:     OwnedHandle::empty(),
            process:    OwnedHandle::empty(),
            thread_id:  0,
            process_id: 0,
        }
    }
}
impl Default for StartupInfo {
    #[inline]
    fn default() -> StartupInfo {
        StartupInfo {
            size:           mem::size_of::<StartupInfo>() as u32,
            pad1:           ptr::null(),
            pad2:           0,
            pad3:           ptr::null(),
            pos_x:          0,
            pos_y:          0,
            flags:          0,
            title:          WCharPtr::null(),
            stdin:          0,
            stdout:         0,
            stderr:         0,
            size_x:         0,
            size_y:         0,
            desktop:        WCharPtr::null(),
            show_window:    0,
            count_chars_x:  0,
            count_chars_y:  0,
            fill_attribute: 0,
        }
    }
}
impl Default for ThreadBasicInfo {
    #[inline]
    fn default() -> ThreadBasicInfo {
        ThreadBasicInfo {
            pad1:        0,
            pad2:        0,
            teb_base:    0,
            client_id:   ClientID { process: 0, thread: 0 },
            exit_status: 0,
        }
    }
}
impl Default for ProcessBasicInfo {
    #[inline]
    fn default() -> ProcessBasicInfo {
        ProcessBasicInfo {
            pad1:              0,
            pad2:              0,
            peb_base:          ptr::null_mut(),
            process_id:        0,
            exit_status:       0,
            parent_process_id: 0,
        }
    }
}

impl Drop for ProcessThreadAttrList {
    #[inline]
    fn drop(&mut self) {
        for i in 0..self.count as usize {
            unsafe { self.attrs[i].assume_init_drop() }
        }
    }
}
impl Default for ProcessThreadAttrList {
    #[inline]
    fn default() -> ProcessThreadAttrList {
        ProcessThreadAttrList {
            pad:   0,
            unk:   0,
            mask:  0,
            size:  0,
            count: 0,
            attrs: [
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }
}

#[cfg(feature = "snap")]
mod inner {
    use core::mem;

    use crate::device::winapi::{self, ProcessEntry, ThreadEntry, Win32Result};
    use crate::util::stx::prelude::*;

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
                pad1:       [0; 2],
                size:       0x1C,
                flags:      0,
                usage:      0,
                thread_id:  0,
                process_id: 0,
            }
        }
    }
    impl Default for ProcessEntry32 {
        #[inline]
        fn default() -> ProcessEntry32 {
            ProcessEntry32 {
                size:       mem::size_of::<ProcessEntry32>() as u32,
                flags:      0,
                usage:      0,
                threads:    0,
                heap_id:    0,
                module_id:  0,
                parent_id:  0,
                class_base: 0,
                process_id: 0,
                exe_file:   [0; 260],
            }
        }
    }

    impl From<&ThreadEntry32> for ThreadEntry {
        #[inline]
        fn from(v: &ThreadEntry32) -> ThreadEntry {
            ThreadEntry {
                sus:        0,
                thread_id:  v.thread_id,
                process_id: v.process_id,
            }
        }
    }
    impl From<&ProcessEntry32> for ProcessEntry {
        #[inline]
        fn from(v: &ProcessEntry32) -> ProcessEntry {
            ProcessEntry {
                name:         winapi::utf16_to_str_trim(&v.exe_file),
                session:      -1,
                parent_id:    v.parent_id,
                process_id:   v.process_id,
                thread_count: v.threads,
            }
        }
    }

    pub fn list_processes() -> Win32Result<Vec<ProcessEntry>> {
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
    use core::{mem, ptr};

    use crate::device::winapi::loader::ntdll;
    use crate::device::winapi::{self, ClientID, ProcessEntry, ThreadEntry, UnicodeString, Win32Result};
    use crate::util::stx::prelude::*;

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
                pos: pos + mem::size_of::<ProcessInfo>(),
                count: 0,
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
                    .add(self.pos + (mem::size_of::<ThreadInfo>() * self.count as usize)) as *const ThreadInfo)
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
    impl<'a> ExactSizeIterator for ThreadIter<'a> {
        #[inline]
        fn len(&self) -> usize {
            self.total as usize
        }
    }

    impl From<&ThreadInfo> for ThreadEntry {
        #[inline]
        fn from(v: &ThreadInfo) -> ThreadEntry {
            ThreadEntry {
                thread_id:  v.client_id.thread as u32,
                process_id: v.client_id.process as u32,
                sus:        if v.thread_state == 5 && v.wait_reason == 5 {
                    2
                } else {
                    1
                },
            }
        }
    }
    impl From<&ProcessInfo> for ProcessEntry {
        #[inline]
        fn from(v: &ProcessInfo) -> ProcessEntry {
            ProcessEntry {
                name:         v.image_name.to_string(),
                session:      v.session_id as i32,
                parent_id:    v.parent_process_id as u32,
                process_id:   v.process_id as u32,
                thread_count: v.thread_count,
            }
        }
    }

    pub fn list_processes() -> Win32Result<Vec<ProcessEntry>> {
        winapi::init_ntdll();
        let func = unsafe {
            winapi::make_syscall!(
                *ntdll::NtQuerySystemInformation,
                extern "stdcall" fn(u32, *mut u8, u32, *mut u32) -> u32
            )
        };
        let mut s = 0;
        // 0x5 - SystemProcessInformation
        let r = func(0x5, ptr::null_mut(), 0, &mut s);
        if s == 0 {
            return Err(winapi::nt_error(r));
        }
        let mut b = vec![0; s as usize * 2];
        // 0x5 - SystemProcessInformation
        let r = func(0x5, b.as_mut_ptr(), s * 2, &mut s);
        if r > 0 {
            return Err(winapi::nt_error(r));
        }
        Ok(ProcessInfo::iter(&b).map(|(v, _)| v.into()).collect::<Vec<ProcessEntry>>())
    }
    pub fn list_threads(pid: u32) -> Win32Result<Vec<ThreadEntry>> {
        winapi::init_ntdll();
        let func = unsafe {
            winapi::make_syscall!(
                *ntdll::NtQuerySystemInformation,
                extern "stdcall" fn(u32, *mut u8, u32, *mut u32) -> u32
            )
        };
        let mut s = 0;
        // 0x5 - SystemProcessInformation
        let r = func(0x5, ptr::null_mut(), 0, &mut s);
        if s == 0 {
            return Err(winapi::nt_error(r));
        }
        let mut b = vec![0; s as usize * 2];
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
