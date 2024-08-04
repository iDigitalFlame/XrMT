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
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::kernel32::{K32EnumDeviceDrivers, K32GetDeviceDriverFileName, K32GetModuleInformation};
use crate::device::winapi::loader::Loader;

pub(super) static DLL: Loader = Loader::new(|psapi| {
    // These are only loaded if kernelbase.dll doesn't exist (ie: < Win7)
    psapi.proc(&K32EnumDeviceDrivers, 0x36EBB2F5);
    psapi.proc(&K32GetModuleInformation, 0xC94AC5BB);
    psapi.proc(&K32GetDeviceDriverFileName, 0x1F449D97);
});
