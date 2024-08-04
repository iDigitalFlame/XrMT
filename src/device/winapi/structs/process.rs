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

use core::marker::PhantomData;
use core::mem::{size_of, zeroed, MaybeUninit};
use core::{cmp, matches, ptr};

use crate::data::blob::Blob;
use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle, ReadInto, StringBlock, UnicodeString, WCharPtr, Win32Result, WriteFrom};
use crate::prelude::*;

pub enum StartInfo<'a> {
    None,
    Basic(&'a StartupInfo),
    Extended(&'a StartupInfoEx),
}

#[repr(C)]
pub struct RemotePEB {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    pub mutant:                   usize,
    pub image_base_address:       usize,
    pub ldr:                      usize,
    pub process_parameters:       usize,
}

#[repr(C)]
pub struct PEB {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    // bitflags has the following
    //  0 - ImageUsesLargePages
    //  1 - IsProtectedProcess
    //  2 - IsImageDynamicallyRelocated
    //  3 - SkipPatchingUser32Forwarders
    //  4 - IsPackagedProcess
    //  5 - IsAppContainer
    //  6 - IsProtectedProcessLight
    //  7 - IsLongPathAwareProcess
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
    pub flags:       u32,
    pub load_count:  i16,
    pub tls_index:   u16,
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
pub struct LoaderIter<'a> {
    cur: *mut LoaderEntry,
    _p:  PhantomData<&'a LoaderEntry>,
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
pub struct RemoteProcessParams {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           usize,
    pub console_flags:     u32,
    pub standard_input:    usize,
    pub standard_output:   usize,
    pub standard_error:    usize,
    pub current_directory: RemoteCurrentDirectory,
    pub dll_path:          RemoteUnicodeString,
    pub image_name:        RemoteUnicodeString,
    pub command_line:      RemoteUnicodeString,
    pub environment:       usize,
    pub start_x:           u32,
    pub start_y:           u32,
    pub count_x:           u32,
    pub count_y:           u32,
    pub count_chars_x:     u32,
    pub count_chars_y:     u32,
    pub fill_attribute:    u32,
    pub window_flags:      u32,
    pub show_window_flags: u32,
    pub window_title:      RemoteUnicodeString,
    pub desktop_info:      RemoteUnicodeString,
    pub shell_info:        RemoteUnicodeString,
    pub runtime_data:      RemoteUnicodeString,
}
#[repr(C)]
pub struct RemoteUnicodeString {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     usize,
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
#[repr(C)]
pub struct RemoteCurrentDirectory {
    pub dos_path: RemoteUnicodeString,
    pub handle:   usize,
}

impl PEB {
    #[inline]
    pub fn load_list<'a>(&self) -> &'a LoadList {
        unsafe { &*(self.ldr) }
    }
    #[inline]
    pub fn process_params<'a>(&self) -> &'a ProcessParams {
        unsafe { &*(self.process_parameters) }
    }
}
impl LoadList {
    #[inline]
    pub fn iter<'a>(&self) -> LoaderIter<'a> {
        LoaderIter {
            cur: self.module_list.f_link,
            _p:  PhantomData,
        }
    }
}
impl StartInfo<'_> {
    #[inline]
    pub fn is_extended(&self) -> bool {
        matches!(self, StartInfo::Extended(_))
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

impl Default for RemotePEB {
    #[inline]
    fn default() -> RemotePEB {
        RemotePEB {
            ldr:                      0usize,
            mutant:                   0usize,
            bitflags:                 0u8,
            being_debugged:           0u8,
            image_base_address:       0usize,
            process_parameters:       0usize,
            read_image_file_exec:     0u8,
            inheritied_address_space: 0u8,
        }
    }
}
impl Default for ProcessInfo {
    #[inline]
    fn default() -> ProcessInfo {
        ProcessInfo {
            thread:     OwnedHandle::empty(),
            process:    OwnedHandle::empty(),
            thread_id:  0u32,
            process_id: 0u32,
        }
    }
}
impl Default for StartupInfo {
    #[inline]
    fn default() -> StartupInfo {
        StartupInfo {
            size:           size_of::<StartupInfo>() as u32,
            pad1:           ptr::null(),
            pad2:           0u16,
            pad3:           ptr::null(),
            pos_x:          0u32,
            pos_y:          0u32,
            flags:          0u32,
            title:          WCharPtr::null(),
            stdin:          0usize,
            stdout:         0usize,
            stderr:         0usize,
            size_x:         0u32,
            size_y:         0u32,
            desktop:        WCharPtr::null(),
            show_window:    0u16,
            count_chars_x:  0u32,
            count_chars_y:  0u32,
            fill_attribute: 0u32,
        }
    }
}
impl Default for ThreadBasicInfo {
    #[inline]
    fn default() -> ThreadBasicInfo {
        ThreadBasicInfo {
            pad1:        0u64,
            pad2:        0u32,
            teb_base:    0usize,
            client_id:   ClientID { process: 0usize, thread: 0usize },
            exit_status: 0u32,
        }
    }
}
impl Default for ProcessBasicInfo {
    #[inline]
    fn default() -> ProcessBasicInfo {
        ProcessBasicInfo {
            pad1:              0usize,
            pad2:              0u32,
            peb_base:          ptr::null_mut(),
            process_id:        0usize,
            exit_status:       0u32,
            parent_process_id: 0usize,
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
            pad:   0u32,
            unk:   0usize,
            mask:  0u32,
            size:  0u32,
            count: 0u32,
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

impl<'a> Iterator for LoaderIter<'a> {
    type Item = &'a mut LoaderEntry;

    #[inline]
    fn next(&mut self) -> Option<&'a mut LoaderEntry> {
        if self.cur.is_null() || unsafe { &(*self.cur) }.dll_base.is_invalid() {
            return None;
        }
        let r = unsafe { &mut *(self.cur) };
        self.cur = unsafe { (*self.cur).f_link };
        Some(r)
    }
}

impl RemotePEB {
    #[inline]
    pub fn read(h: impl AsHandle, ptr: *mut PEB) -> Win32Result<RemotePEB> {
        let mut p = RemotePEB::default();
        winapi::NtReadVirtualMemory(
            h,
            ptr.into(),
            size_of::<RemotePEB>(),
            ReadInto::Direct(&mut p),
        )?;
        Ok(p)
    }

    #[inline]
    pub fn command_line(&self, h: impl AsHandle) -> Win32Result<String> {
        Ok(self.process_params(&h)?.command_line.to_string(h))
    }
    #[inline]
    pub fn process_params(&self, h: impl AsHandle) -> Win32Result<RemoteProcessParams> {
        let mut p = unsafe { zeroed() };
        winapi::NtReadVirtualMemory(
            h,
            self.process_parameters.into(),
            size_of::<RemoteProcessParams>(),
            ReadInto::Direct(&mut p),
        )?;
        Ok(p)
    }
}
impl RemoteUnicodeString {
    #[inline]
    pub fn len(&self) -> usize {
        self.length as usize
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.max_length == 0 || self.buffer == 0
    }
    #[inline]
    pub fn to_string(&self, h: impl AsHandle) -> String {
        if self.length == 0 || self.max_length == 0 || self.buffer == 0 {
            return String::new();
        }
        let mut b: Blob<u16, 128> = Blob::with_size(self.length as usize / 2);
        let r = winapi::NtReadVirtualMemory(
            h,
            self.buffer.into(),
            self.length as usize,
            ReadInto::Pointer(b.as_mut_ptr()),
        )
        .unwrap_or(0); // If this fails, we just return an empty String.
        if r == 0 {
            return String::new();
        }
        match b.iter().rposition(|v| *v == b' ' as u16) {
            Some(i) => winapi::utf16_to_str(&b[..i]),
            None => winapi::utf16_to_str(&b),
        }
    }
    pub fn write(&self, h: impl AsHandle, v: impl AsRef<str>) -> Win32Result<usize> {
        if self.length == 0 || self.max_length == 0 {
            return Ok(0);
        }
        let s = v.as_ref().encode_utf16().collect::<Blob<u16, 128>>();
        let t = cmp::min(s.len_as_bytes(), self.length as usize) as u16;
        // x32 - 4
        // x64 - 8 PTR_SIZE??
        winapi::NtWriteVirtualMemory(
            &h,
            (self.buffer - winapi::PTR_SIZE).into(),
            2,
            WriteFrom::Direct(&t),
        )?;
        winapi::NtWriteVirtualMemory(
            &h,
            self.buffer.into(),
            s.len_as_bytes(),
            WriteFrom::Pointer(s.as_ptr()),
        )
    }
}

#[inline]
pub fn read_remote_cmdline(h: impl AsHandle) -> Win32Result<String> {
    let mut i = ProcessBasicInfo::default();
    // 0x0 - ProcessBasicInformation
    winapi::NtQueryInformationProcess(&h, 0, &mut i, size_of::<ProcessBasicInfo>() as u32)?;
    let p = RemotePEB::read(&h, i.peb_base)?;
    p.command_line(h)
}
