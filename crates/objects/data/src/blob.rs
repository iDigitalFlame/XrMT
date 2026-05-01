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

extern crate core;

mod blob;
mod buffer;
mod slice;

pub use self::blob::*;
pub use self::buffer::*;
pub use self::slice::*;

#[cold]
fn failure_alloc() -> ! {
    // Call the panic handler
    #[cfg(feature = "strip")]
    {
        core::panicking::panic("")
    }
    #[cfg(not(feature = "strip"))]
    {
        core::panicking::panic("allocation failure")
    }
}
#[cold]
fn failure_too_large() -> ! {
    #[cfg(feature = "strip")]
    {
        core::panicking::panic("")
    }
    #[cfg(not(feature = "strip"))]
    {
        core::panicking::panic("allocation too large")
    }
}
#[cold]
fn failure_oob(_i: usize, _n: usize) -> ! {
    #[cfg(feature = "strip")]
    {
        core::panicking::panic("")
    }
    #[cfg(not(feature = "strip"))]
    {
        core::panic!("index {_i} is out-of-bounds (len: {_n})");
    }
}
