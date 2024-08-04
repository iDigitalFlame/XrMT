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
#![cfg(all(target_family = "windows", not(target_pointer_width = "64")))]

use core::cmp;
use core::mem::{size_of, zeroed};

use crate::data::blob::Blob;
use crate::device::winapi::{self, AsHandle, ReadInto, Win32Result, WriteFrom};
use crate::prelude::*;

#[repr(C)]
pub struct RemotePEB64 {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    pub mutant:                   u64,
    pub image_base_address:       u64,
    pub ldr:                      u64,
    pub process_parameters:       u64,
}
#[repr(C)]
pub struct ProcessBasicInfo64 {
    pub exit_status:       u32,
    pub peb_base:          u64,
    pad1:                  u64,
    pad2:                  u32,
    pub process_id:        u64,
    pub parent_process_id: u64,
}
#[repr(C)]
pub struct RemoteProcessParams64 {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           u64,
    pub console_flags:     u32,
    pub standard_input:    u64,
    pub standard_output:   u64,
    pub standard_error:    u64,
    pub current_directory: RemoteCurrentDirectory64,
    pub dll_path:          RemoteUnicodeString64,
    pub image_name:        RemoteUnicodeString64,
    pub command_line:      RemoteUnicodeString64,
    pub environment:       u64,
    pub start_x:           u32,
    pub start_y:           u32,
    pub count_x:           u32,
    pub count_y:           u32,
    pub count_chars_x:     u32,
    pub count_chars_y:     u32,
    pub fill_attribute:    u32,
    pub window_flags:      u32,
    pub show_window_flags: u32,
    pub window_title:      RemoteUnicodeString64,
    pub desktop_info:      RemoteUnicodeString64,
    pub shell_info:        RemoteUnicodeString64,
    pub runtime_data:      RemoteUnicodeString64,
}
#[repr(C)]
pub struct RemoteUnicodeString64 {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     u64,
}
#[repr(C)]
pub struct RemoteCurrentDirectory64 {
    pub dos_path: RemoteUnicodeString64,
    pub handle:   u64,
}

impl RemotePEB64 {
    #[inline]
    pub fn read(h: impl AsHandle, ptr: u64) -> Win32Result<RemotePEB64> {
        let mut p = RemotePEB64::default();
        winapi::NtWoW64ReadVirtualMemory64(
            h,
            ptr,
            size_of::<RemotePEB64>() as u64,
            ReadInto::Direct(&mut p),
        )?;
        Ok(p)
    }

    #[inline]
    pub fn command_line(&self, h: impl AsHandle) -> Win32Result<String> {
        Ok(self.process_params(&h)?.command_line.to_string(h))
    }
    #[inline]
    pub fn process_params(&self, h: impl AsHandle) -> Win32Result<RemoteProcessParams64> {
        let mut p = unsafe { zeroed() };
        winapi::NtWoW64ReadVirtualMemory64(
            h,
            self.process_parameters,
            size_of::<RemoteProcessParams64>() as u64,
            ReadInto::Direct(&mut p),
        )?;
        Ok(p)
    }
}
impl RemoteUnicodeString64 {
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
        let r = winapi::NtWoW64ReadVirtualMemory64(
            h,
            self.buffer,
            self.length as u64,
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
        winapi::NtWoW64WriteVirtualMemory64(&h, (self.buffer - 8) as u64, 2, WriteFrom::Direct(&t))?;
        winapi::NtWoW64WriteVirtualMemory64(
            &h,
            self.buffer,
            s.len_as_bytes() as u64,
            WriteFrom::Pointer(s.as_ptr()),
        )
        .map(|v| v as usize)
    }
}

impl Default for RemotePEB64 {
    #[inline]
    fn default() -> RemotePEB64 {
        RemotePEB64 {
            ldr:                      0u64,
            mutant:                   0u64,
            bitflags:                 0u8,
            being_debugged:           0u8,
            image_base_address:       0u64,
            process_parameters:       0u64,
            read_image_file_exec:     0u8,
            inheritied_address_space: 0u8,
        }
    }
}
impl Default for ProcessBasicInfo64 {
    #[inline]
    fn default() -> ProcessBasicInfo64 {
        ProcessBasicInfo64 {
            pad1:              0u64,
            pad2:              0u32,
            peb_base:          0u64,
            process_id:        0u64,
            exit_status:       0u32,
            parent_process_id: 0u64,
        }
    }
}

#[inline]
pub fn read_remote_cmdline(h: impl AsHandle) -> Win32Result<String> {
    let mut i = ProcessBasicInfo64::default();
    // 0x0 - ProcessBasicInformation
    winapi::NtWow64QueryInformationProcess64(&h, 0, &mut i, 0x30)?;
    let p = RemotePEB64::read(&h, i.peb_base)?;
    p.command_line(h)
}
