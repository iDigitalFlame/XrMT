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

use crate::data::time::Time;
use crate::util::stx::prelude::*;

#[cfg(unix)]
pub mod fs {
    extern crate std;
    pub use std::fs::*;
}
#[cfg(windows)]
#[path = "device/winapi/std"]
pub mod fs {
    mod fs;
    pub use self::fs::*;
}

#[cfg(unix)]
mod sys {
    mod unix;
    pub use self::unix::*;
}
#[cfg(windows)]
mod sys {
    mod windows;
    pub use self::windows::*;
}

mod id;
mod machine;
mod network;
pub mod rand;
pub mod winapi;

pub use self::id::ID;
pub(crate) use self::machine::system_id;
pub use self::machine::Machine;
pub use self::network::{Address, HardwareAddress, Interface, Network};
pub use self::sys::*;

pub struct Device {
    pub shell:   String,
    pub machine: Machine,
    // Maybe we can use this to save constants?
}

#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct Login {
    pub login_time: Time,
    pub last_input: Time,
    pub user:       String,
    pub host:       String,
    pub from:       Address,
    pub id:         u32,
    pub status:     u8,
}
