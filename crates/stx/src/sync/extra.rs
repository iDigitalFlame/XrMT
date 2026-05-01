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

use core::option::Option::{self, Some};
use core::time::Duration;

pub const ZERO: Option<Duration> = Some(Duration::ZERO);

mod event;
mod flag;
mod group;
mod lazy;
mod mutant;
mod refs;
mod semaphore;

pub use self::event::*;
pub use self::flag::*;
pub use self::group::*;
#[cfg(target_family = "windows")]
pub use self::inner::*;
pub use self::lazy::*;
#[cfg(target_family = "windows")]
pub use self::mutant::*;
pub use self::refs::*;
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "solaris"),
    not(target_vendor = "apple")
))]
pub use self::semaphore::*;

#[cfg(target_family = "windows")]
#[path = "extra"]
mod inner {
    #[cfg(not(feature = "std"))]
    mod chan;
    mod lazy_signal;
    mod signal;
    mod timer;

    #[cfg(not(feature = "std"))]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(crate) use self::chan::*;

    pub use self::lazy_signal::*;
    pub use self::signal::*;
    pub use self::timer::*;
}
