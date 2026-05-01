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

mod handler;
mod signals;

pub(super) use self::handler::*;
pub use self::signals::*;

#[macro_export]
macro_rules! errcheck {
    ($f:expr) => {
        errcheck!($f, -1)
    };
    ($f:expr, $ec:expr) => {
        match unsafe { $f } {
            $ec => core::result::Result::Err(xrmt_stx::io::IoError::last_os_error()),
            r => core::result::Result::Ok(r),
        }
    };
}
