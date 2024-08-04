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

use crate::device::winapi::loader::{Function, Loader};

pub(crate) static BitBlt: Function = Function::new();
pub(crate) static GetDIBits: Function = Function::new();

pub(crate) static SelectObject: Function = Function::new();
pub(crate) static DeleteObject: Function = Function::new();

pub(crate) static DeleteDC: Function = Function::new();
pub(crate) static CreateCompatibleDC: Function = Function::new();
pub(crate) static CreateCompatibleBitmap: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|gdi32| {
    gdi32.proc(&BitBlt, 0x4C7E7258);
    gdi32.proc(&GetDIBits, 0x35F5C026);

    gdi32.proc(&SelectObject, 0xFBC3B004);
    gdi32.proc(&DeleteObject, 0x2AAC1D49);

    gdi32.proc(&DeleteDC, 0x3C53364B);
    gdi32.proc(&CreateCompatibleDC, 0xD5203D54);
    gdi32.proc(&CreateCompatibleBitmap, 0xC2BE1C3E);
});
