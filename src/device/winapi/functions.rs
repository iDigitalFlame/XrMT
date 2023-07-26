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
#![allow(non_snake_case)]

use core::arch::asm;
use core::mem;

use crate::device::winapi::{self, AsHandle, Handle, StringBlock, Win32Error, Win32Result, PEB};
use crate::util::stx::prelude::*;

mod advapi32;
mod iphlpapi;
mod kernel32;
mod netapi32;
mod ntdll;
mod structs;
mod userenv;
mod winsock;
mod wtsapi32;

pub use advapi32::*;
pub use iphlpapi::*;
pub use kernel32::*;
pub use netapi32::*;
pub use ntdll::*;
pub use userenv::*;
pub use winsock::*;
pub use wtsapi32::*;

pub(super) use self::structs::*;

pub const CURRENT_THREAD: Handle = Handle((-2 as isize) as usize);
pub const CURRENT_PROCESS: Handle = Handle((-1 as isize) as usize);

pub(crate) const PTR_SIZE: usize = core::mem::size_of::<usize>();

#[inline(never)]
pub fn GetLastError() -> u32 {
    let e: u32;
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax,   FS:[0x18]
             mov {0:e}, dword ptr [eax+0x34]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax,   qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x68]", out(reg) e
        );
    }
    e
}
#[inline]
pub fn GetCommandLine() -> String {
    unsafe { (*(*GetCurrentProcessPEB()).process_parameters).command_line.to_string() }
}
#[inline(never)]
pub fn GetProcessHeap() -> Handle {
    let mut h = Handle::default();
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov eax, dword ptr [eax+0x30]
             mov {},  dword ptr [eax+0x18]",  out(reg) h.0
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, qword ptr GS:[0x60]
             mov {},  qword ptr [rax+0x30]", out(reg) h.0
        );
    }
    h
}
#[inline(never)]
pub fn GetCurrentThreadID() -> u32 {
    let e: u32;
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax,   FS:[0x18]
             mov {0:e}, dword ptr [eax+0x24]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax,   qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x48]", out(reg) e
        );
    }
    e
}
#[inline(never)]
pub fn GetCurrentProcessID() -> u32 {
    let e: u32;
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax,   FS:[0x18]
             mov {0:e}, dword ptr [eax+0x20]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax,   qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x40]", out(reg) e
        );
    }
    e
}
#[inline]
pub fn GetCurrentDirectory() -> String {
    unsafe {
        (*(*GetCurrentProcessPEB()).process_parameters)
            .current_directory
            .dos_path
            .to_string()
    }
}
#[inline(never)]
pub fn GetCurrentProcessPEB() -> *const PEB {
    let p: *mut PEB;
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov {},  dword ptr [eax+0x30]", out(reg) p
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, qword ptr GS:[0x30]
             mov {},  qword ptr [rax+0x60]", out(reg) p
        );
    }
    p
}
pub fn EmptyWorkingSet() -> Win32Result<()> {
    let p = winapi::acquire_privilege(winapi::SE_INCREASE_PRIORITY_PRIVILEGE).is_ok();
    let q = QuotaLimit::empty();
    let r = winapi::NtSetInformationProcess(
        winapi::CURRENT_PROCESS,
        0x1, // ProcessQuotaLimits
        &q,
        mem::size_of::<QuotaLimit>() as u32,
    );
    if p {
        // Release the Privilege even if we fail.
        // IGNORE ERROR
        let _ = winapi::release_privilege(winapi::SE_INCREASE_PRIORITY_PRIVILEGE);
    }
    r
}
#[inline]
pub fn GetEnvironment<'a>() -> &'a StringBlock {
    unsafe { &(*(*GetCurrentProcessPEB()).process_parameters).environment }
}
pub fn GetModuleFileName(h: impl AsHandle) -> Win32Result<String> {
    unsafe {
        let peb = GetCurrentProcessPEB();
        let v = h.as_handle();
        let m = if v.is_invalid() { (*peb).image_base_address } else { v };
        let mut next = (*(*peb).ldr).module_list.f_link;
        loop {
            if m == (*next).dll_base {
                return Ok((*next).full_name.to_string());
            }
            if (*next).f_link.is_null() || (*(*next).f_link).dll_base.is_invalid() {
                break;
            }
            next = (*next).f_link;
        }
    }
    Err(Win32Error::InvalidHandle)
}
#[inline]
pub fn GetEnvironmentVariable(key: impl AsRef<str>) -> Option<String> {
    unsafe { (*(*GetCurrentProcessPEB()).process_parameters).environment.find(key) }
}
#[inline]
pub fn RtlSetProcessIsCritical(is_critical: bool) -> Win32Result<bool> {
    let (mut c, s) = (0u32, if is_critical { 1u32 } else { 0u32 });
    // 0x1D - ProcessBreakOnTermination
    winapi::NtQueryInformationProcess(winapi::CURRENT_PROCESS, 0x1D, &mut c, 0x4)?;
    // 0x1D - ProcessBreakOnTermination
    winapi::NtSetInformationProcess(winapi::CURRENT_PROCESS, 0x1D, &s, 0x4)?;
    Ok(c == 1)
}

#[inline]
pub(super) fn check_nt_handle(h: Handle) -> usize {
    if h.0 > 0xFFFFFFF6 || h.0 < 0xFFFFFFF4 {
        return h.0;
    }
    unsafe {
        let p = GetCurrentProcessPEB();
        match h.0 {
            0xFFFFFFF6 => (*(*p).process_parameters).standard_input.0,  // -10 | STD_INPUT_HANDLE
            0xFFFFFFF5 => (*(*p).process_parameters).standard_output.0, // -11 | STD_OUTPUT_HANDLE
            0xFFFFFFF4 => (*(*p).process_parameters).standard_error.0,  // -12 | STD_ERROR_HANDLE
            _ => h.0,
        }
    }
}
