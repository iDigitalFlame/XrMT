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
#![allow(non_snake_case)]

extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_crypt;
extern crate xrmt_winapi_fnv;

use core::mem::transmute;

use xrmt_crypt::crypt;

use crate::structs::NonZeroHandle;

crate::dll!(
    C Kernel32,
    KERNEL32,
    kernel32,
    || crypt!(0, "kernel32.dll"),
    CopyFileExW,
    CreateProcessW,
    CreateRemoteThread,
    DebugBreak,
    DuplicateHandle,
    LocalFree,
    MoveFileExW,
    OutputDebugStringA,
    SetConsoleCtrlHandler,
    WriteConsoleA
);

crate::dll!(
    C KernelBase,
    KERNELBASE,
    kernelbase,
    || crypt!(0, "kernelbase.dll"),
    CopyFileExW,
    CreateProcessW,
    CreateRemoteThread,
    DebugBreak,
    DuplicateHandle,
    LocalFree,
    MoveFileExW,
    OutputDebugStringA,
    SetConsoleCtrlHandler,
    WriteConsoleA
);

#[inline]
pub fn kernel32_or_base<'a>() -> &'a Kernel32 {
    let v = kernel32();
    // Loading like this will try to load kernel32 first. If it's loaded, there's
    // a 99% chance that kernelbase was also loaded.
    if KERNELBASE.is_loaded() {
        unsafe { transmute(&kernelbase().v) }
    } else {
        &v.v
    }
}
#[inline]
pub fn kernel32_or_base_address<'a>() -> NonZeroHandle {
    let v = kernel32();
    // Loading like this will try to load kernel32 first. If it's loaded, there's
    // a 99% chance that kernelbase was also loaded.
    if KERNELBASE.is_loaded() {
        kernelbase().address()
    } else {
        v.address()
    }
}
