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
#![no_std]

pub mod data {
    extern crate xrmt_data;
    pub use xrmt_data::*;
}
pub mod hash {
    extern crate xrmt_hash;
    pub use xrmt_hash::*;
}
pub mod memory {
    extern crate xrmt_memory;
    pub use xrmt_memory::*;
}
pub mod random {
    extern crate xrmt_random;
    pub use xrmt_random::*;
}
