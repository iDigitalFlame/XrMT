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
#![allow(non_snake_case)]

use core::arch::asm;
use core::mem::size_of;

use crate::device::winapi::{self, AsHandle, Handle, StringBlock, Win32Error, Win32Result, PEB};
use crate::ignore_error;
use crate::prelude::*;

mod advapi32;
mod crypt32;
mod dbghelp;
mod gdi32;
mod iphlpapi;
mod kernel32;
mod netapi32;
mod ntdll;
mod structs;
mod user32;
mod userenv;
mod winsock;
mod wtsapi32;

pub use advapi32::*;
#[allow(unused_imports)]
pub use crypt32::*; // TODO(dij): Finish
pub use dbghelp::*;
pub use gdi32::*;
pub use iphlpapi::*;
pub use kernel32::*;
pub use netapi32::*;
pub use ntdll::*;
pub use user32::*;
pub use userenv::*;
pub use winsock::*;
pub use wtsapi32::*;

pub(super) use self::structs::*;

pub const CURRENT_THREAD: Handle = Handle(-2isize as usize);
pub const CURRENT_PROCESS: Handle = Handle(-1isize as usize);

pub(crate) const PTR_SIZE: usize = size_of::<usize>();

#[inline(never)]
pub fn SetLastError(e: u32) {
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetLastError'.
        asm!(
            "push       {{rll, lr}}
             mov          rll, sp
             mrc          p15, 0x0, r3, cr13, cr0, 0x2
             ldr  [r3, #0x34], r2", in("r2") e
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov          x8, x18
             ldr [x8, #0x68], x9", in("x9") e
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov                  eax, FS:[0x18]
             mov dword ptr [eax+0x34], ecx", in("ecx") e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov                  rax, qword ptr GS:[0x30]
             mov dword ptr [rax+0x68], ecx", in("ecx") e
        );
    }
}
#[inline(never)]
pub fn GetLastError() -> u32 {
    let e: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): I'm not 100% sure if this works correctly. For the most
        //            part Windows or ARM has a very limited set of machines
        //            _currently_ supported for ARM and is focusing more on
        //            AARCH64.
        //            Also, why is ARM opcode so different than AARCH64?
        asm!(
            "push {{rll, lr}}
             mov    rll, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr  {0:e}, [r3, #0x34]", out(reg) e
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:e}, [x8, #0x68]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x34]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x68]", out(reg) e
        );
    }
    e
}
#[inline]
pub fn GetCommandLine() -> String {
    GetCurrentProcessPEB().process_params().command_line.to_string()
}
#[inline(never)]
pub fn GetProcessHeap() -> Handle {
    let mut h = Handle::default();
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetLastError'.
        asm!(
            "push {{rll, lr}}
             mov    rll, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     r0, [r3, #0x30]
             ldr     {}, [r0, #0x18]", out(reg) h.0
        );
        // NOTE(dij): This is some guesstimating on my part based on the other
        //            ASM samples I found. *shrug*
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov x8, x18
             ldr x8, [x8, #0x60]
             ldr {}, [x8, #0x30]", out(reg) h.0
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov eax, dword ptr [eax+0x30]
             mov {},  dword ptr [eax+0x18]", out(reg) h.0
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
    let i: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetLastError'.
        asm!(
            "push {{rll, lr}}
             mov    rll, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr  {0:e}, [r3, #0x24]", out(reg) i
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:e}, [x8, #0x48]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x24]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x48]", out(reg) i
        );
    }
    i
}
#[inline(never)]
pub fn GetCurrentProcessID() -> u32 {
    let i: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetLastError'.
        asm!(
            "push {{rll, lr}}
             mov    rll, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr  {0:e}, [r3, #0x20]", out(reg) i
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:e}, [x8, #0x40]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x20]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x40]", out(reg) i
        );
    }
    i
}
#[inline]
pub fn GetCurrentDirectory() -> String {
    GetCurrentProcessPEB()
        .process_params()
        .current_directory
        .dos_path
        .to_string()
}
pub fn EmptyWorkingSet() -> Win32Result<()> {
    let p = winapi::acquire_privilege(winapi::SE_INCREASE_PRIORITY_PRIVILEGE).is_ok();
    let q = QuotaLimit::empty();
    let r = winapi::NtSetInformationProcess(
        winapi::CURRENT_PROCESS,
        0x1, // ProcessQuotaLimits
        &q,
        size_of::<QuotaLimit>() as u32,
    );
    if p {
        // Release the Privilege even if we fail.
        ignore_error!(winapi::release_privilege(
            winapi::SE_INCREASE_PRIORITY_PRIVILEGE
        ));
    }
    r
}
#[inline(never)]
pub fn GetCurrentProcessPEB<'a>() -> &'a PEB {
    let p: *mut PEB;
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetLastError'.
        asm!(
            "push {{rll, lr}}
             mov    rll, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     {}, [r3, #0x30]", out(reg) p
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov x8, x18
             ldr {}, [x8, #0x60]", out(reg) p
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov  {}, dword ptr [eax+0x30]", out(reg) p
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, qword ptr GS:[0x30]
             mov  {}, qword ptr [rax+0x60]", out(reg) p
        );
    }
    unsafe { &*p }
}
#[inline]
pub fn GetEnvironment<'a>() -> &'a StringBlock {
    &GetCurrentProcessPEB().process_params().environment
}
pub fn GetModuleFileName(h: impl AsHandle) -> Win32Result<String> {
    let v = h.as_handle();
    let p = GetCurrentProcessPEB();
    let m = if v.is_invalid() { p.image_base_address } else { v };
    p.load_list().iter().find(|i| i.dll_base == m).map_or_else(
        || Err(Win32Error::InvalidHandle),
        |v| Ok(v.full_name.to_string()),
    )
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
    let p = GetCurrentProcessPEB();
    match h.0 {
        0xFFFFFFF6 => p.process_params().standard_input.0,  // -10 | STD_INPUT_HANDLE
        0xFFFFFFF5 => p.process_params().standard_output.0, // -11 | STD_OUTPUT_HANDLE
        0xFFFFFFF4 => p.process_params().standard_error.0,  // -12 | STD_ERROR_HANDLE
        _ => h.0,
    }
}
