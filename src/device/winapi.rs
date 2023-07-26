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
// ^ NOTE(dij): We eventually need to remove this, as *nix versions can to some
//              "windows-like" things, like PE unlinking and extracting.
//              Have to find a rust-like way to do this.

extern crate core;

mod alloc;
mod errors;
mod functions;
mod helpers;
mod loader;
mod path;
pub mod registry;
mod stdio;
mod structs;

macro_rules! syscall {
    ($address:expr, $t:ty, $($x:expr),*) => {
        core::mem::transmute::<*const (), $t>($address as _)($($x,)*)
    };
}
macro_rules! make_syscall {
    ($address:expr, $t:ty) => {
        core::mem::transmute::<*const (), $t>($address as _)
    };
}

use {make_syscall, syscall};

pub use self::alloc::*;
pub use self::errors::{Win32Error, Win32Result};
pub use self::functions::*;
pub use self::helpers::*;
pub use self::path::*;
pub use self::stdio::*;
pub use self::structs::*;

#[inline]
pub fn unload_libraries() {
    let _ = loader::unload_dlls(); // IGNORE ERROR
}

use self::errors::nt_error;
use self::loader::*;
