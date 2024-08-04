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

use core::sync::atomic::{AtomicBool, Ordering};

use crate::device::winapi::loader::LoadType;
use crate::device::winapi::{self, Handle, LoaderEntry};
use crate::ignore_error;
use crate::prelude::*;
use crate::util::crypt;

static DLL_INIT: AtomicBool = AtomicBool::new(false);

#[inline]
pub(crate) fn init_ntdll() {
    init();
}
#[inline]
pub(crate) fn init_amsi() {
    init();
    super::amsi::DLL.load_if_name(|| crypt::get_or(0, "amsi.dll"))
}
#[inline]
pub(crate) fn init_gdi32() {
    init();
    super::gdi32::DLL.load_if_name(|| crypt::get_or(0, "gdi32.dll"))
}
#[allow(dead_code)] // TODO(dij): Finish
#[inline]
pub(crate) fn init_psapi() {
    init();
    super::psapi::DLL.load_if_name(|| crypt::get_or(0, "psapi.dll"))
}
#[inline]
pub(crate) fn init_user32() {
    init();
    super::user32::DLL.load_if_name(|| crypt::get_or(0, "user32.dll"))
}
#[allow(dead_code)] // TODO(dij): Finish
#[inline]
pub(crate) fn init_crypt32() {
    init();
    super::crypt32::DLL.load_if_name(|| crypt::get_or(0, "crypt32.dll"))
}
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
#[allow(dead_code)] // TODO(dij): Finish
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
        ignore_error!(d.load_if_name(|| crypt::get_or(0, "kernelbase.dll")));
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
    //
    // NOTE(dij): Don't make an iter for this, we want to have the least amount
    //            of moving parts here.
    let mut e = [(Handle(0usize), LoadType::Linked); 0x10];
    for i in winapi::GetCurrentProcessPEB().load_list().iter() {
        bugtrack!(
            "Mapper::init(): Found PEB DLL '{}' ({:X}).",
            i.full_name.to_string(),
            i.dll_base
        );
        // Match DLL name by its lower FNV32 hash.
        match i.base_name.hash() {
            // ntdll.dll
            // NOTE(dij): We don't increase the load count for ntdll as it will
            //            never be unloaded.
            0xA9ACADD3 => e[0x0] = (i.dll_base, LoadType::Linked),
            // kernelbase.dll
            0x55AA707B => e[0x1] = load(i),
            // kernel32.dll
            0xD741ACCF => e[0x2] = load(i),
            // advapi32.dll
            0x316180CD => e[0x3] = load(i),
            // user32.dll
            0xB1CC909D => e[0x4] = load(i),
            // userenv.dll
            0x614A9C5B => e[0x5] = load(i),
            // ws2_32.dll
            0x6CFD82A3 => e[0x6] = load(i),
            // iphlpapi.dll
            0x86DA0E28 => e[0x7] = load(i),
            // netapi32.dll
            0x0AB85C5F => e[0x8] = load(i),
            // winhttp.dll
            0x198007FF => e[0x9] = load(i),
            // psapi.dll
            0x914A0BCC => e[0xA] = load(i),
            // crypt32.dll
            0xC38FCEAE => e[0xB] = load(i),
            // dbhhelp.dll
            0xB674F88D => e[0xC] = load(i),
            // gpi32.dll
            0x6C411560 => e[0xD] = load(i),
            // wtsapi32.dll
            0xEABBD160 => e[0xE] = load(i),
            // asmi.dll
            0x9D8C6359 => e[0xF] = load(i),
            _ => (),
        }
    }
    // Load DLLs in a specific order so we can resolve references easily.
    for i in 0..e.len() {
        if e[i].0 .0 == 0 {
            continue;
        }
        match i {
            0x0 => super::ntdll::DLL.load_by_handle(true, e[i].0, e[i].1),
            0x1 => super::kernel32::KERNELBASE.load_by_handle(false, e[i].0, e[i].1),
            0x2 => super::kernel32::KERNEL32.load_by_handle(false, e[i].0, e[i].1),
            0x3 => super::advapi32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0x4 => super::user32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0x5 => super::userenv::DLL.load_by_handle(false, e[i].0, e[i].1),
            0x7 => super::iphlpapi::DLL.load_by_handle(false, e[i].0, e[i].1),
            0x8 => super::netapi32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0x9 => super::winhttp::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xA => super::psapi::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xB => super::crypt32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xC => super::dbghelp::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xD => super::gdi32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xE => super::wtsapi32::DLL.load_by_handle(false, e[i].0, e[i].1),
            0xF => super::amsi::DLL.load_by_handle(false, e[i].0, e[i].1),
            _ => (),
        }
    }
    bugtrack!("Mapper::init(): PEB Loading complete!")
}
#[inline]
unsafe fn load(e: &mut LoaderEntry) -> (Handle, LoadType) {
    // Load and increase load count so it won't get unloaded if the parent
    // process frees it.
    //
    // Indictate and skip staticly linked DLLs.
    if e.load_count == -1 {
        (e.dll_base, LoadType::Linked)
    } else {
        bugtrack!("Mapper::load(): Increasing load_count for {:X}", e.dll_base);
        e.load_count += 1;
        (e.dll_base, LoadType::Dynamic)
    }
}
