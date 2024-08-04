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

use core::sync::atomic::{AtomicU8, Ordering};

use crate::data::blob::Blob;
use crate::device::winapi::{self, AsHandle, Handle, TimeZoneInfo};
use crate::ignore_error;
use crate::prelude::*;

const WINVER_WIN_UNKNOWN: u8 = 0u8;
const WINVER_WIN_XP: u8 = 1u8;
const WINVER_WIN_XP64: u8 = 2u8;
const WINVER_WIN_VISTA: u8 = 3u8;
const WINVER_WIN_7: u8 = 4u8;
const WINVER_WIN_8: u8 = 5u8;
const WINVER_WIN_8_1: u8 = 6u8;
const WINVER_WIN_10: u8 = 7u8;
const WINVER_WIN_11: u8 = 8u8;

// NOTE(dij): This is an atomic as there's no way for this to get deadlocked as
//            this is checked BEFORE a thread can even be created, so only one
//            thread could realistically load this.
static VERSION: AtomicU8 = AtomicU8::new(0xFFu8);

pub fn is_debugged() -> bool {
    let s = winapi::kernel_user_shared();
    if s.ke_debugger_enabled > 1 || s.shared_flags & 0x1 != 0 {
        // s.shared_flags&0x1 == 1
        //
        // NOTE(dij): ^ This returns true when on a Debug/Checked version on
        //            Windows. Not sure if we want to ignore this or not,
        //            but I doubt that actual systems are using
        //            "Multiprocessor Debug/Checked" unless the system is a
        //            driver test or builder.
        return true;
    }
    let mut f = 0u16;
    // 0x23 - SystemKernelDebuggerInformation
    let r = winapi::NtQuerySystemInformation(0x23, &mut f, 2).unwrap_or_default();
    // The SYSTEM_KERNEL_DEBUGGER_INFORMATION short offset 1 (last 8 bits) is not
    // filled out by systems older than Vista, so we ignore them.
    if r == 2 && ((f & 0xFF) > 1 || ((f >> 8) == 0 && winapi::is_min_windows_vista())) {
        return true;
    }
    let p = winapi::GetCurrentProcessPEB();
    // 0x70 - FLG_HEAP_ENABLE_TAIL_CHECK | FLG_HEAP_ENABLE_FREE_CHECK |
    // FLG_HEAP_VALIDATE_PARAMETERS
    if p.being_debugged > 0 || p.nt_global_flag & 0x70 != 0 {
        return true;
    }
    #[cfg(target_arch = "x86_64")]
    {
        // NOTE(dij): x64 specific hack.
        //
        // For some reason, the XMM0 (floating point) register will ALWAYS be
        // non-zero when 'OutputDebugStringA' returns successfully and a debugger
        // is able to receive the Debug string.
        //
        // In Golang, this was the output of the 'r2' on 'syscall', which translates
        // to:
        //   - XMM0 (x64)
        //   - EDX  (x86)
        //   - R1?  (ARM64) [Technically Unimplemented]
        //   - N/A  (ARM) [Not Implemented]
        let b = [b'_', 0u8];
        unsafe {
            winapi::syscall!(
                *winapi::kernel32::OutputDebugStringA,
                extern "stdcall" fn(*const u8),
                b.as_ptr()
            )
        };
        let e: usize;
        unsafe { core::arch::asm!("movq {}, XMM0", out(xmm_reg) e) };
        if e > 0 {
            return true;
        }
    }
    winapi::OpenProcess(0x400, false, winapi::GetCurrentProcessID()).map_or(false, |h| {
        winapi::CheckRemoteDebuggerPresent(h).unwrap_or(false)
    })
}
#[inline]
pub fn is_elevated() -> bool {
    // 0x8 - TOKEN_QUERY
    winapi::OpenThreadToken(winapi::CURRENT_THREAD, 0x8, true)
        .or_else(|_| winapi::OpenProcessToken(winapi::CURRENT_PROCESS, 0x8))
        .map_or(false, |t| {
            if winapi::is_min_windows_vista() {
                check_token_elevated(t)
            } else {
                in_admin_group(t)
            }
        })
}
#[inline]
pub fn is_utc_time() -> bool {
    // We're gonna check KUSER_SHARED first, so we don't have to make any syscalls.
    // if that check fails we do the syscall to check.
    if winapi::kernel_time_offset() == 0 {
        return true;
    }
    // Now that the fast check didn't pass, check here.
    let mut i = TimeZoneInfo::default();
    // 0x2C - SystemCurrentTimeZoneInformation
    winapi::NtQuerySystemInformation(0x2C, &mut i, 0xAC).map_or(false, |_| {
        i.bias == 0 && i.daylight_bias == 0 && i.standard_bias == 0
    })
}
#[inline]
pub fn in_safe_mode() -> bool {
    winapi::kernel_user_shared().safe_boot_mode > 0
}
#[inline]
pub fn is_windows_xp() -> bool {
    version() == WINVER_WIN_XP
}
#[inline]
pub fn is_system_eval() -> bool {
    winapi::kernel_user_shared().expiration_date > 0
}
#[inline]
pub fn is_uac_enabled() -> bool {
    // 0x2 - DbgElevationEnabled
    winapi::kernel_user_shared().shared_flags & 0x2 != 0
}
#[inline]
pub fn is_windows_xp64() -> bool {
    version() == WINVER_WIN_XP64
}
#[inline]
pub fn is_min_windows_7() -> bool {
    version() >= WINVER_WIN_7
}
#[inline]
pub fn is_min_windows_8() -> bool {
    version() >= WINVER_WIN_8
}
#[cfg(target_pointer_width = "64")]
#[inline]
pub fn in_wow64_process() -> bool {
    false // x64/AARCH64 is never WoW
}
#[cfg(not(target_pointer_width = "64"))]
#[inline]
pub fn in_wow64_process() -> bool {
    // We cache this value to prevent from having to call it a lot.
    wow::in_wow64_process()
}
#[inline]
pub fn is_min_windows_10() -> bool {
    version() >= WINVER_WIN_10
}
#[inline]
pub fn is_min_windows_vista() -> bool {
    version() >= WINVER_WIN_VISTA
}
#[inline]
pub fn is_secure_boot_enabled() -> bool {
    // Fastpath, might not work 100%, so we fallback to the syscall.
    // 0x80 - DbgSecureBootEnabled
    if winapi::kernel_user_shared().shared_flags & 0x80 != 0 {
        return true;
    }
    let mut f = 0u16;
    // 0x91 - SystemSecureBootInformation
    winapi::NtQuerySystemInformation(0x91, &mut f, 2).map_or(false, |_| f & 0xFF == 1)
}
#[inline]
pub fn is_stack_tracing_enabled() -> bool {
    winapi::kernel_user_shared().max_stack_trace > 0
}

pub fn is_debugged_using_load(dll: impl AsRef<str>) -> bool {
    let v = dll.as_ref();
    let mut p: Blob<u16, 128> = Blob::new();
    let b = v
        .as_bytes()
        .iter()
        .position(|c| *c == b':' || *c == b'/' || *c == b'\\')
        .is_some();
    if !b {
        let d = winapi::system_dir();
        p.extend_from_slice(&d.data[0..d.len]);
        p.push(b'\\' as u16);
    }
    v.encode_utf16().collect_into(&mut p);
    // The loaded module isn't free'd as we don't need to.
    // 0x2 - GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT
    if winapi::GetModuleHandleExW(0x2, &p).is_ok() {
        // Module is loaded, we can't do it with this one.
        return false;
    }
    let h = ok_or_return!(winapi::LoadLibraryW(&p), false);
    // Have to allocate a string here as duplicating the CreateFile call would
    // be way too much work.
    let s = winapi::utf16_to_str_trim(&p);
    let r = winapi::NtCreateFile(
        s,
        Handle::INVALID,
        0x80000000 | 0x00100000 | 0x80,
        None,
        0,
        0,
        0x1,
        0x40 | 0x20,
    )
    .map_or_else(|e| e.code() != 2, |_| false);
    // Free the library.
    ignore_error!(winapi::FreeLibrary(h));
    r
}

#[inline]
pub fn is_token_elevated(h: impl AsHandle) -> bool {
    if !winapi::is_min_windows_vista() {
        in_admin_group(h)
    } else {
        check_token_elevated(h)
    }
}

#[cfg(not(feature = "std"))]
#[inline]
pub(crate) fn is_user_network_token(t: impl AsHandle) -> bool {
    let h = t.as_handle();
    if h.is_invalid() {
        return false;
    }
    let mut b = [0u8; 16];
    // 0x7 - TokenSource
    winapi::GetTokenInformation(h, 0x7, b.as_mut_ptr(), 0x10).map_or(false, |_| {
        b[0] == 65 && b[1] == 100 && b[7] == 32 && b[7] == 32
    })
    // Match [65 100 118 97 112 105 32 32] == "Advapi"
}

fn version() -> u8 {
    if let Err(r) = VERSION.compare_exchange(0xFF, 0xFE, Ordering::AcqRel, Ordering::Relaxed) {
        return r;
    }
    let (m, x, _) = winapi::GetVersionNumbers();
    let r = match m {
        11 => WINVER_WIN_11,
        10 => WINVER_WIN_10,
        6 => match x {
            0 => WINVER_WIN_VISTA,
            1 => WINVER_WIN_7,
            2 => WINVER_WIN_8,
            3 => WINVER_WIN_8_1,
            _ => WINVER_WIN_UNKNOWN,
        },
        5 => {
            if x == 1 {
                WINVER_WIN_XP
            } else {
                // 5.2 is used for Server 2003 and x64 Xp.
                // They both have the same stats and avaliable feature sets.
                //
                // https://en.wikipedia.org/wiki/List_of_Microsoft_Windows_versions#Server_versions
                WINVER_WIN_XP64
            }
        },
        _ => WINVER_WIN_UNKNOWN,
    };
    VERSION.store(r, Ordering::Release);
    r
}
#[inline]
fn in_admin_group(h: impl AsHandle) -> bool {
    let mut b: Blob<u8, 256> = Blob::new();
    // Use in block to prevent dropping the Blob.
    {
        // For Windows XP hack the SID and check for the Local Administrators group.
        winapi::token_groups(h, &mut b).map_or(false, |e| e.iter().position(|v| v.sid.is_admin()).is_some())
    }
}
fn check_token_elevated(h: impl AsHandle) -> bool {
    let mut buf = [0u8; 32];
    // 0x19 - TokenIntegrityLevel
    let r = winapi::GetTokenInformation(&h, 0x19, &mut buf, 32).unwrap_or_default();
    if r == 0 {
        return false;
    }
    let p = buf[r as usize - 4] as u32 | (buf[r as usize - 3] as u32) << 8 | (buf[r as usize - 2] as u32) << 16 | (buf[r as usize - 1] as u32) << 24;
    if p > 0x3000 {
        return true;
    }
    let mut e = 0u32;
    // 0x14 - TokenElevation
    winapi::GetTokenInformation(h, 0x14, &mut e, 4).unwrap_or_default() == 4 && e != 0
}

#[cfg(not(target_pointer_width = "64"))]
mod wow {
    use core::sync::atomic::{AtomicU8, Ordering};

    use crate::device::winapi::{self, ntdll};
    use crate::prelude::*;

    const WOW_NOT_PRESENT: u8 = 0u8;
    const WOW_PRESENT: u8 = 1u8;

    static WOW: AtomicU8 = AtomicU8::new(0xFFu8);

    #[inline]
    pub fn in_wow64_process() -> bool {
        // We cache this value to prevent from having to call it a lot.
        wow() == WOW_PRESENT
    }

    fn wow() -> u8 {
        if let Err(r) = WOW.compare_exchange(0xFF, 0xFE, Ordering::AcqRel, Ordering::Relaxed) {
            return r;
        }
        winapi::init_ntdll(); // Ensure ntdll.dll is loaded.
                              // Fastpath check. If the function exists inside ntdll.dll, then
                              // we are 100% in a WoW64 process. It's an easy way without having to
                              // run a syscall.
        let r = if ntdll::NtWow64AllocateVirtualMemory64.is_loaded() {
            WOW_PRESENT
        } else if winapi::IsWoW64Process(winapi::CURRENT_PROCESS).unwrap_or(false) {
            WOW_PRESENT
        } else {
            WOW_NOT_PRESENT
        };
        WOW.store(r, Ordering::Release);
        r
    }
}
