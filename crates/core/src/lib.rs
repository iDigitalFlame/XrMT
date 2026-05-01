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

extern crate xrmt_builtins;
extern crate xrmt_crypt;

#[cfg_attr(feature = "std", allow(unused_imports))]
pub use xrmt_builtins::*;
pub use xrmt_crypt::*;

pub mod io {
    extern crate xrmt_io;
    pub use xrmt_io::*;
}
pub mod text {
    extern crate xrmt_text;
    pub use xrmt_text::*;
}
pub mod time {
    extern crate xrmt_time;
    pub use xrmt_time::*;
}
