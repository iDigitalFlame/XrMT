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

use core::ptr;

use crate::device::winapi::loader::netapi32;
use crate::device::winapi::{self, MaybeString, WCharPtr, WChars, Win32Result};
use crate::util::stx::prelude::*;

pub fn NetGetJoinInformation(server: impl MaybeString) -> Win32Result<(String, u8)> {
    winapi::init_netapi32();
    let mut x = 0u32;
    let mut b = WCharPtr::null();
    let r = unsafe {
        let s: WChars = server.into_string().into();
        winapi::syscall!(
            *netapi32::NetGetJoinInformation,
            extern "stdcall" fn(*const u16, *mut *mut u16, *mut u32) -> u32,
            if s.is_empty() { ptr::null() } else { s.as_ptr() },
            &mut b.as_mut_ptr(),
            &mut x
        )
    };
    if r > 0 {
        return Err(winapi::last_error());
    }
    let u = b.to_string();
    winapi::LocalFree(b.as_ptr());
    Ok((u, x as u8))
}
