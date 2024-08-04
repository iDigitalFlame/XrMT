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
#![cfg(target_family = "windows")]

mod data;
mod debug;
mod handles;
mod list;
mod memory;
mod network;
mod process;
mod registry;
mod screen;
mod security;
mod sessions;
mod string;
mod system;

pub use self::data::*;
pub use self::debug::*;
pub use self::handles::*;
pub use self::list::*;
pub use self::memory::*;
pub use self::network::*;
pub use self::process::*;
pub use self::registry::*;
pub use self::screen::*;
pub use self::security::*;
pub use self::sessions::*;
pub use self::string::*;
pub use self::system::*;

pub type TimerFunc = unsafe extern "stdcall" fn(usize, u32, u32);
