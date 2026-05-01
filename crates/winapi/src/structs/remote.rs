// Copyright (C) 2023 - 2025 iDigitalFlame
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

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::string::String;
use core::default::Default;
use core::hint::unreachable_unchecked;
use core::iter::Iterator;
use core::option::Option::Some;
use core::result::Result::Ok;

use xrmt_data::text::utf16_to_string;
use xrmt_data::Blob;

use crate::functions::{NtQueryInformationProcess, NtReadVirtualMemory, NtWoW64ReadVirtualMemory64, NtWow64QueryInformationProcess64};
use crate::info::is_wow_process;
use crate::structs::{Handle, ReadInto, WChars};
use crate::Win32Result;

pub enum RPEB {
    X32(PEB32),
    X64(PEB64),
}
pub enum RParams {
    X32(ProcessParams32),
    X64(ProcessParams64),
}

#[repr(C)]
pub struct PEB32 {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    pub mutant:                   u32,
    pub image_base_address:       u32,
    pub ldr:                      u32,
    pub process_parameters:       u32,
}
#[repr(C)]
pub struct PEB64 {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    pub mutant:                   u64,
    pub image_base_address:       u64,
    pub ldr:                      u64,
    pub process_parameters:       u64,
}
pub struct RemotePEB<'a> {
    pub peb: RPEB,
    h:       &'a Handle,
}
#[repr(C)]
pub struct ProcessParams32 {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           u32,
    pub console_flags:     u32,
    pub standard_input:    u32,
    pub standard_output:   u32,
    pub standard_error:    u32,
    pub current_directory: CurrentDirectory32,
    pub dll_path:          UnicodeString32,
    pub image_name:        UnicodeString32,
    pub command_line:      UnicodeString32,
    pub environment:       u32,
    pub start_x:           u32,
    pub start_y:           u32,
    pub count_x:           u32,
    pub count_y:           u32,
    pub count_chars_x:     u32,
    pub count_chars_y:     u32,
    pub fill_attribute:    u32,
    pub window_flags:      u32,
    pub show_window_flags: u32,
    pub window_title:      UnicodeString32,
    pub desktop_info:      UnicodeString32,
    pub shell_info:        UnicodeString32,
    pub runtime_data:      UnicodeString32,
}
#[repr(C)]
pub struct ProcessParams64 {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           u64,
    pub console_flags:     u32,
    pub standard_input:    u64,
    pub standard_output:   u64,
    pub standard_error:    u64,
    pub current_directory: CurrentDirectory64,
    pub dll_path:          UnicodeString64,
    pub image_name:        UnicodeString64,
    pub command_line:      UnicodeString64,
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
    pub window_title:      UnicodeString64,
    pub desktop_info:      UnicodeString64,
    pub shell_info:        UnicodeString64,
    pub runtime_data:      UnicodeString64,
}
#[repr(C)]
pub struct UnicodeString32 {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     u32,
}
#[repr(C)]
pub struct UnicodeString64 {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     u64,
}
#[repr(C)]
pub struct CurrentDirectory32 {
    pub dos_path: UnicodeString32,
    pub handle:   u32,
}
#[repr(C)]
pub struct CurrentDirectory64 {
    pub dos_path: UnicodeString64,
    pub handle:   u64,
}
pub struct RemoteParameters<'a> {
    pub params: RParams,
    h:          &'a Handle,
}

#[repr(C)]
struct ProcessInfo64 {
    pub exit_status:       u32,
    pub peb_base:          u64,
    pad1:                  u64,
    pad2:                  u32,
    pub process_id:        u64,
    pub parent_process_id: u64,
}
#[repr(C)]
struct ProcessInfo32 {
    pub exit_status:       u32,
    pub peb_base:          u32,
    pad1:                  u32,
    pad2:                  u32,
    pub process_id:        u32,
    pub parent_process_id: u32,
}

impl UnicodeString32 {
    #[inline]
    pub fn len(&self) -> usize {
        self.length as usize
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.max_length == 0 || self.buffer == 0
    }
    pub fn to_wchar(&self, h: &Handle) -> WChars {
        if self.is_empty() {
            return Blob::new();
        }
        let mut b: WChars = Blob::with_size(self.length as usize / 2);
        let r = NtReadVirtualMemory(
            h,
            self.buffer as usize,
            self.length as usize,
            ReadInto::Pointer(b.as_mut_ptr()),
        )
        .unwrap_or(0); // If this fails, we just return an empty String.
        if r > 0 {
            if let Some(i) = unsafe { b.get_unchecked(0..r) }.iter().rposition(|v| *v == 0x20) {
                b.truncate(i);
            }
        }
        b
    }
    #[inline]
    pub fn to_string(&self, h: &Handle) -> String {
        utf16_to_string(self.to_wchar(h).as_slice())
    }
}
impl UnicodeString64 {
    #[inline]
    pub fn len(&self) -> usize {
        self.length as usize
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.max_length == 0 || self.buffer == 0
    }
    pub fn to_wchar(&self, h: &Handle) -> WChars {
        if self.is_empty() {
            return Blob::new();
        }
        let mut b: WChars = Blob::with_size(self.length as usize / 2);
        let r = if is_wow_process() {
            NtWoW64ReadVirtualMemory64(
                h,
                self.buffer,
                self.length as u64,
                ReadInto::Pointer(b.as_mut_ptr()),
            )
            .map(|v| v as usize)
        } else {
            NtReadVirtualMemory(
                h,
                self.buffer as usize,
                self.length as usize,
                ReadInto::Pointer(b.as_mut_ptr()),
            )
        }
        .unwrap_or(0);
        if r > 0 {
            if let Some(i) = unsafe { b.get_unchecked(0..(r / 2)) }.iter().rposition(|v| *v == 0x20) {
                b.truncate(i);
            }
        }
        b
    }
    #[inline]
    pub fn to_string(&self, h: &Handle) -> String {
        utf16_to_string(self.to_wchar(h).as_slice())
    }
}
impl<'a> RemotePEB<'a> {
    pub fn new(h: &'a Handle) -> Win32Result<RemotePEB<'a>> {
        let v = match (is_wow_process(), cfg!(target_pointer_width = "64")) {
            // Not possible to have WoW without x64/AARCH64
            (true, true) => unsafe { unreachable_unchecked() },
            // WoW Process on x64 Machine
            (true, false) => {
                let mut i = ProcessInfo64::default();
                // 0x0 - ProcessBasicInformation
                NtWow64QueryInformationProcess64(&h, 0, &mut i, 0x30)?;
                let mut p = PEB64::default();
                NtWoW64ReadVirtualMemory64(h, i.peb_base, 0x28, ReadInto::Direct(&mut p))?;
                RPEB::X64(p)
            },
            // Native (x86) Process on x86 Machine
            (false, false) => {
                let mut i = ProcessInfo32::default();
                // 0x0 - ProcessBasicInformation
                NtQueryInformationProcess(h, 0, &mut i, 0x18)?;
                let mut p = PEB32::default();
                NtReadVirtualMemory(h, i.peb_base as usize, 0x14, ReadInto::Direct(&mut p))?;
                RPEB::X32(p)
            },
            // Native (x64) Process on x64 Machine
            (_, true) => {
                let mut i = ProcessInfo64::default();
                // 0x0 - ProcessBasicInformation
                NtQueryInformationProcess(h, 0, &mut i, 0x30)?;
                let mut p = PEB64::default();
                NtReadVirtualMemory(h, i.peb_base as usize, 0x28, ReadInto::Direct(&mut p))?;
                RPEB::X64(p)
            },
        };
        Ok(RemotePEB { h, peb: v })
    }

    #[inline]
    pub fn command_line(&self) -> Win32Result<String> {
        self.process_parameters()?.command_line()
    }
    #[inline]
    pub fn command_line_wchar(&self) -> Win32Result<WChars> {
        self.process_parameters()?.command_line_wchar()
    }
    pub fn process_parameters(&self) -> Win32Result<RemoteParameters<'a>> {
        let p = match &self.peb {
            RPEB::X64(v) if is_wow_process() => {
                let mut a = ProcessParams64::default();
                NtWoW64ReadVirtualMemory64(self.h, v.process_parameters, 0xF0, ReadInto::Direct(&mut a))?;
                RParams::X64(a)
            },
            RPEB::X32(v) => {
                let mut a = ProcessParams32::default();
                NtReadVirtualMemory(
                    self.h,
                    v.process_parameters as usize,
                    0x90,
                    ReadInto::Direct(&mut a),
                )?;
                RParams::X32(a)
            },
            RPEB::X64(v) => {
                let mut a = ProcessParams64::default();
                NtReadVirtualMemory(
                    self.h,
                    v.process_parameters as usize,
                    0xF0,
                    ReadInto::Direct(&mut a),
                )?;
                RParams::X64(a)
            },
        };
        Ok(RemoteParameters { h: self.h, params: p })
    }
}
impl<'a> RemoteParameters<'a> {
    #[inline]
    pub fn command_line(&self) -> Win32Result<String> {
        match &self.params {
            RParams::X32(v) => Ok(v.command_line.to_string(self.h)),
            RParams::X64(v) => Ok(v.command_line.to_string(self.h)),
        }
    }
    #[inline]
    pub fn command_line_wchar(&self) -> Win32Result<WChars> {
        match &self.params {
            RParams::X32(v) => Ok(v.command_line.to_wchar(self.h)),
            RParams::X64(v) => Ok(v.command_line.to_wchar(self.h)),
        }
    }
}

impl Default for PEB32 {
    #[inline]
    fn default() -> PEB32 {
        PEB32 {
            ldr:                      0u32,
            mutant:                   0u32,
            bitflags:                 0u8,
            being_debugged:           0u8,
            image_base_address:       0u32,
            process_parameters:       0u32,
            read_image_file_exec:     0u8,
            inheritied_address_space: 0u8,
        }
    }
}
impl Default for PEB64 {
    #[inline]
    fn default() -> PEB64 {
        PEB64 {
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
impl Default for ProcessInfo32 {
    #[inline]
    fn default() -> ProcessInfo32 {
        ProcessInfo32 {
            pad1:              0u32,
            pad2:              0u32,
            peb_base:          0u32,
            process_id:        0u32,
            exit_status:       0u32,
            parent_process_id: 0u32,
        }
    }
}
impl Default for ProcessInfo64 {
    #[inline]
    fn default() -> ProcessInfo64 {
        ProcessInfo64 {
            pad1:              0u64,
            pad2:              0u32,
            peb_base:          0u64,
            process_id:        0u64,
            exit_status:       0u32,
            parent_process_id: 0u64,
        }
    }
}
impl Default for UnicodeString32 {
    #[inline]
    fn default() -> UnicodeString32 {
        UnicodeString32 {
            buffer:     0u32,
            length:     0u16,
            max_length: 0u16,
        }
    }
}
impl Default for UnicodeString64 {
    #[inline]
    fn default() -> UnicodeString64 {
        UnicodeString64 {
            buffer:     0u64,
            length:     0u16,
            max_length: 0u16,
        }
    }
}
impl Default for ProcessParams32 {
    #[inline]
    fn default() -> ProcessParams32 {
        ProcessParams32 {
            flags:             0u32,
            length:            0u32,
            console:           0u32,
            start_x:           0u32,
            start_y:           0u32,
            count_x:           0u32,
            count_y:           0u32,
            dll_path:          UnicodeString32::default(),
            image_name:        UnicodeString32::default(),
            max_length:        0u32,
            shell_info:        UnicodeString32::default(),
            environment:       0u32,
            debug_flags:       0u32,
            command_line:      UnicodeString32::default(),
            window_title:      UnicodeString32::default(),
            window_flags:      0u32,
            runtime_data:      UnicodeString32::default(),
            desktop_info:      UnicodeString32::default(),
            console_flags:     0u32,
            count_chars_x:     0u32,
            count_chars_y:     0u32,
            standard_input:    0u32,
            standard_error:    0u32,
            fill_attribute:    0u32,
            standard_output:   0u32,
            current_directory: CurrentDirectory32::default(),
            show_window_flags: 0u32,
        }
    }
}
impl Default for ProcessParams64 {
    #[inline]
    fn default() -> ProcessParams64 {
        ProcessParams64 {
            flags:             0u32,
            length:            0u32,
            console:           0u64,
            start_x:           0u32,
            start_y:           0u32,
            count_x:           0u32,
            count_y:           0u32,
            dll_path:          UnicodeString64::default(),
            image_name:        UnicodeString64::default(),
            max_length:        0u32,
            shell_info:        UnicodeString64::default(),
            environment:       0u64,
            debug_flags:       0u32,
            command_line:      UnicodeString64::default(),
            window_title:      UnicodeString64::default(),
            window_flags:      0u32,
            runtime_data:      UnicodeString64::default(),
            desktop_info:      UnicodeString64::default(),
            console_flags:     0u32,
            count_chars_x:     0u32,
            count_chars_y:     0u32,
            standard_input:    0u64,
            standard_error:    0u64,
            fill_attribute:    0u32,
            standard_output:   0u64,
            current_directory: CurrentDirectory64::default(),
            show_window_flags: 0u32,
        }
    }
}
impl Default for CurrentDirectory32 {
    #[inline]
    fn default() -> CurrentDirectory32 {
        CurrentDirectory32 {
            handle:   0u32,
            dos_path: UnicodeString32::default(),
        }
    }
}
impl Default for CurrentDirectory64 {
    #[inline]
    fn default() -> CurrentDirectory64 {
        CurrentDirectory64 {
            handle:   0u64,
            dos_path: UnicodeString64::default(),
        }
    }
}
