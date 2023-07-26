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

pub use self::inner::*;

#[cfg(unix)]
mod inner {
    extern crate std;
    pub use std::sync::*;
}
#[cfg(windows)]
#[path = "device/winapi/std/sync"]
mod inner {
    extern crate core;

    mod barrier;
    mod event;
    mod group;
    mod lazy;
    mod lazylock;
    pub mod mpsc;
    mod mutant;
    mod mutex;
    mod once;
    mod semaphore;
    mod timer;

    pub use core::sync::*;

    pub use self::barrier::*;
    pub use self::event::*;
    pub use self::group::*;
    pub(crate) use self::lazy::*;
    pub use self::lazylock::*;
    pub use self::mutant::*;
    pub use self::mutex::*;
    pub use self::once::*;
    pub use self::semaphore::*;
    pub use self::timer::*;
}
