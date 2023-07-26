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

use core::cmp;

use crate::data::blob::Blob;
use crate::device::winapi::loader::userenv;
use crate::device::winapi::{self, AsHandle, DecodeUtf16, Handle, Win32Result};
use crate::util::stx::prelude::*;

pub fn GetUserProfileDirectory(h: impl AsHandle) -> Win32Result<String> {
    winapi::init_userenv();
    let mut s = 128u32;
    let mut b: Blob<u16, 256> = Blob::new();
    let v = h.as_handle();
    let func = unsafe {
        winapi::make_syscall!(
            *userenv::GetUserProfileDirectory,
            extern "stdcall" fn(Handle, *mut u16, *mut u32) -> u32
        )
    };
    loop {
        b.resize_as_bytes(s as usize);
        match func(v, b.as_mut_ptr(), &mut s) {
            // SAFETY: The size returned is always smaller than the buffer.
            1 => return Ok(unsafe { &b.get_unchecked(0..cmp::min(s as usize - 1, b.len())) }.decode_utf16()),
            _ if s < b.len() as u32 => return Err(winapi::last_error()),
            _ => (),
        }
    }
}
