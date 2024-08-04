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

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::*;

use crate::c2::task::Process;
use crate::prelude::*;

pub trait OsFd {
    fn get_fd(&self) -> Option<Fd>;
}
pub trait OsCommand {
    fn add_compat(&mut self) {}
    fn add_extra(&mut self, _p: &mut Process) {}
}
pub trait OsMetadata {
    fn get_mode(&self) -> u32;
}

#[cfg(target_family = "unix")]
#[path = "os/unix.rs"]
mod inner;
#[cfg(target_family = "windows")]
#[path = "os/windows.rs"]
mod inner;
