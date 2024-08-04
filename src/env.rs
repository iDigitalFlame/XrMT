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

pub const PATH: [u8; 4] = [b'P', b'A', b'T', b'H'];

pub use inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "device/winapi/std"]
mod inner {
    mod env;
    pub use self::env::*;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;
    pub use std::env::*;
}
