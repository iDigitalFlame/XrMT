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

pub use self::arc::*;
pub use self::group::*;
pub use self::inner::*;

mod arc;
mod group;

#[cfg(not(target_family = "windows"))]
#[path = "sync"]
mod inner {
    extern crate std;
    pub use std::sync::*;

    mod event;
    pub use self::event::*;
}
#[cfg(all(target_family = "windows", feature = "std"))]
#[path = "device/winapi/std/sync"]
mod inner {
    extern crate core;
    extern crate std;

    mod event;
    mod lazy;
    pub mod mpsc;
    mod mutant;
    mod semaphore;
    mod timer;

    pub use core::sync::*;
    pub use std::sync::*;

    pub use self::event::*;
    pub(crate) use self::lazy::*;
    pub use self::mutant::*;
    pub use self::semaphore::*;
    pub use self::timer::*;
}
#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "device/winapi/std/sync"]
mod inner {
    extern crate core;

    mod barrier;
    mod event;
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
    pub(crate) use self::lazy::*;
    pub use self::lazylock::*;
    pub use self::mutant::*;
    pub use self::mutex::*;
    pub use self::once::*;
    pub use self::semaphore::*;
    pub use self::timer::*;
}
