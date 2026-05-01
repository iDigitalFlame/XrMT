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

extern crate xrmt_bugtrack;
extern crate xrmt_coco;
extern crate xrmt_core;
extern crate xrmt_objects;
extern crate xrmt_stx;

pub mod log {}
pub mod crypto {}
pub mod device {}

pub mod fs {
    extern crate xrmt_stx;

    #[cfg(all(target_family = "windows", not(feature = "std")))]
    pub use xrmt_stx::fs::extra::*;
    pub use xrmt_stx::fs::*;
}
pub mod process {
    extern crate xrmt_stx;

    #[cfg(all(target_family = "windows", not(feature = "std")))]
    pub use xrmt_stx::process::extra::*;
    pub use xrmt_stx::process::*;
}

pub mod stx {
    extern crate xrmt_stx;
    pub use xrmt_stx::*;
}
pub mod coco {
    extern crate xrmt_coco;
    pub use xrmt_coco::*;
}
pub mod winapi {
    extern crate xrmt_winapi;
    pub use xrmt_winapi::*;
}

pub use xrmt_bugtrack::bugtrack;
pub use xrmt_coco::stxa;
pub use xrmt_core::{crypt, text, time};
pub use xrmt_objects::*;
pub use xrmt_stx::runtime;
