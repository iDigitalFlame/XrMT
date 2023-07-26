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

use crate::data::blob::Blob;
use crate::device::winapi::loader::iphlpapi;
use crate::device::winapi::{self, Adapter, Win32Error, Win32Result};
use crate::util::stx::prelude::*;

pub fn GetAdaptersAddresses(family: u32, flags: u32, buf: &mut Blob<u8, 256>) -> Win32Result<Vec<&Adapter>> {
    winapi::init_iphlpapi();
    let mut s: u32 = 15000u32;
    let func = unsafe {
        winapi::make_syscall!(
            *iphlpapi::GetAdaptersAddresses,
            extern "stdcall" fn(u32, u32, u32, *mut u8, *mut u32) -> u32
        )
    };
    loop {
        buf.resize(s as usize);
        let r = func(family, flags, 0, buf.as_mut_ptr(), &mut s);
        match r {
            0x6F => continue, // 0x6F - ERROR_BUFFER_OVERFLOW
            0 => return Ok(buf.as_ref_of::<Adapter>().enumerate().collect()),
            _ => return Err(Win32Error::Code(r)),
        }
    }
}
