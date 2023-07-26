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
#![cfg(unix)]

use crate::device;
use crate::device::machine::{arch, os};
use crate::util::stx::prelude::*;

pub fn system() -> u8 {
    (os::CURRENT as u8) << 4 | arch::CURRENT as u8
}
pub fn elevated() -> u8 {
    0
}
pub fn version() -> String {
    "".to_string()
}
#[inline]
pub fn username() -> String {
    device::whoami().unwrap_or_else(|_| "?".to_string())
}
pub fn system_id() -> Option<Vec<u8>> {
    None
}
