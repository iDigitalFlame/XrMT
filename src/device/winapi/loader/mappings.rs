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

use core::sync::atomic::{AtomicBool, Ordering};

use crate::device::winapi::{self, Handle};
use crate::util::crypt;
use crate::util::stx::prelude::*;

static DLL_INIT: AtomicBool = AtomicBool::new(false);

#[inline]
pub(crate) fn init_ntdll() {
    init();
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_amsi() {
    init();
    super::amsi::DLL.load_if_name(|| crypt::get_or(0, "amsi.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_gdi32() {
    init();
    super::gdi32::DLL.load_if_name(|| crypt::get_or(0, "gdi32.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_psapi() {
    init();
    super::psapi::DLL.load_if_name(|| crypt::get_or(0, "psapi.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_user32() {
    init();
    super::user32::DLL.load_if_name(|| crypt::get_or(0, "user32.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_crypt32() {
    init();
    super::crypt32::DLL.load_if_name(|| crypt::get_or(0, "crypt32.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_dbghelp() {
    init();
    super::dbghelp::DLL.load_if_name(|| crypt::get_or(0, "dbghelp.dll"))
}
#[inline]
pub(crate) fn init_userenv() {
    init();
    super::userenv::DLL.load_if_name(|| crypt::get_or(0, "userenv.dll"))
}
#[allow(dead_code)] // TODO(dij)
#[inline]
pub(crate) fn init_winhttp() {
    init();
    super::winhttp::DLL.load_if_name(|| crypt::get_or(0, "winhttp.dll"))
}
#[inline]
pub(crate) fn init_winsock() {
    init();
    super::winsock::DLL.load_if_name(|| crypt::get_or(0, "ws2_32.dll"))
}
#[inline]
pub(crate) fn init_advapi32() {
    init();
    super::advapi32::DLL.load_if_name(|| crypt::get_or(0, "advapi32.dll"))
}
#[inline]
pub(crate) fn init_kernel32() {
    init();
    super::kernel32::KERNELBASE.load(false, |d| {
        // NOTE(dij): This may fail on older Windows versions, so we're going to
        //            ignore the error instead of crashing.
        let _ = d.load_name(crypt::get_or(0, "kernelbase.dll")); // IGNORE ERROR
        Ok(())
    });
    super::kernel32::KERNEL32.load_if_name(|| crypt::get_or(0, "kernel32.dll"))
}
#[inline]
pub(crate) fn init_iphlpapi() {
    init();
    super::iphlpapi::DLL.load_if_name(|| crypt::get_or(0, "iphlpapi.dll"))
}
#[inline]
pub(crate) fn init_netapi32() {
    init();
    super::netapi32::DLL.load_if_name(|| crypt::get_or(0, "netapi32.dll"))
}
#[inline]
pub(crate) fn init_wtsapi32() {
    init();
    super::wtsapi32::DLL.load_if_name(|| crypt::get_or(0, "wtsapi32.dll"))
}

pub(super) fn init() {
    if DLL_INIT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        unsafe { init_inner() }
    }
}

unsafe fn init_inner() {
    // There CANNOT be any other instructions that call syscalls in here
    // besides the 'Ldr' syscalls.
    //
    // If any other calls are made, this WILL cause a CRASH!!!
    let mut next = (*(*winapi::GetCurrentProcessPEB()).ldr).module_list.f_link;
    let mut e = [Handle(0); 0x10];
    loop {
        crate::bugprint!(
            "LOADER: Found loaded DLL '{}'.",
            (*next).base_name.to_string()
        );
        // Match DLL name by its lower FNV32 hash.
        match (*next).base_name.hash() {
            // ntdll.dll
            0xA9ACADD3 => e[0x0] = (*next).dll_base,
            // kernelbase.dll
            0x55AA707B => e[0x1] = (*next).dll_base,
            // kernel32.dll
            0xD741ACCF => e[0x2] = (*next).dll_base,
            // advapi32.dll
            0x316180CD => e[0x3] = (*next).dll_base,
            // user32.dll
            0xB1CC909D => e[0x4] = (*next).dll_base,
            // userenv.dll
            0x614A9C5B => e[0x5] = (*next).dll_base,
            // ws2_32.dll
            0x6CFD82A3 => e[0x6] = (*next).dll_base,
            // iphlpapi.dll
            0x86DA0E28 => e[0x7] = (*next).dll_base,
            // netapi32.dll
            0x0AB85C5F => e[0x8] = (*next).dll_base,
            // winhttp.dll
            0x198007FF => e[0x9] = (*next).dll_base,
            // psapi.dll
            0x914A0BCC => e[0xA] = (*next).dll_base,
            // crypt32.dll
            0xC38FCEAE => e[0xB] = (*next).dll_base,
            // dbhhelp.dll
            0xB674F88D => e[0xC] = (*next).dll_base,
            // gpi32.dll
            0x6C411560 => e[0xD] = (*next).dll_base,
            // wtsapi32.dll
            0xEABBD160 => e[0xE] = (*next).dll_base,
            // asmi.dll
            0x9D8C6359 => e[0xF] = (*next).dll_base,
            _ => (),
        }
        if (*next).f_link.is_null() || (*(*next).f_link).dll_base.is_invalid() {
            break;
        }
        next = (*next).f_link;
    }
    // Load DLLs in a specific order so we can resolve references easily.
    for i in 0..0x10 {
        if e[i].0 == 0 {
            continue;
        }
        match i {
            0x0 => super::ntdll::DLL.load_handle(true, e[i]),
            0x1 => super::kernel32::KERNELBASE.load_handle(false, e[i]),
            0x2 => super::kernel32::KERNEL32.load_handle(false, e[i]),
            0x3 => super::advapi32::DLL.load_handle(false, e[i]),
            0x4 => super::user32::DLL.load_handle(false, e[i]),
            0x5 => super::userenv::DLL.load_handle(false, e[i]),
            0x7 => super::iphlpapi::DLL.load_handle(false, e[i]),
            0x8 => super::netapi32::DLL.load_handle(false, e[i]),
            0x9 => super::winhttp::DLL.load_handle(false, e[i]),
            0xA => super::psapi::DLL.load_handle(false, e[i]),
            0xB => super::crypt32::DLL.load_handle(false, e[i]),
            0xC => super::dbghelp::DLL.load_handle(false, e[i]),
            0xD => super::gdi32::DLL.load_handle(false, e[i]),
            0xE => super::wtsapi32::DLL.load_handle(false, e[i]),
            0xF => super::amsi::DLL.load_handle(false, e[i]),
            _ => (),
        }
    }
}
