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

extern crate core;

extern crate xrmt_data;

use core::cmp::min;
use core::convert::Into;
use core::default::Default;
use core::iter::Iterator;
use core::mem::size_of;
use core::option::Option::Some;
use core::result::Result::Ok;

use xrmt_data::Blob;

// TODO(dij): this

#[inline]
pub fn read_cmdline_wow(h: impl AsHandle) -> Win32Result<String> {
    let mut i = crate::structs::ProcessBasicInfo64::default();
    // 0x0 - ProcessBasicInformation
    crate::functions::NtWow64QueryInformationProcess64(&h, 0, &mut i, 0x30)?;
    let p = crate::structs::RemotePEB64::read(&h, i.peb_base)?;
    p.command_line(h)
}
