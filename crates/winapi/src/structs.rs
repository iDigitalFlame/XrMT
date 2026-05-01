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

#[cfg(target_family = "windows")]
#[path = "structs"]
mod inner {
    mod data;
    mod debug;
    mod handles;
    mod io;
    mod iters;
    mod lazy;
    mod loader;
    mod memory;
    mod namespaces;
    mod network;
    mod peb;
    mod process;
    mod registry;
    mod remote;
    mod security;
    mod session;
    mod system;
    mod window;
    pub mod wow;

    pub use self::data::*;
    pub use self::debug::*;
    pub use self::handles::*;
    pub use self::io::*;
    pub use self::iters::*;
    pub(crate) use self::lazy::*;
    pub use self::loader::*;
    pub use self::memory::*;
    pub use self::namespaces::*;
    pub use self::network::*;
    pub use self::peb::*;
    pub use self::process::*;
    pub use self::registry::*;
    pub use self::remote::*;
    pub use self::security::*;
    pub use self::session::*;
    pub use self::system::*;
    pub use self::window::*;

    pub type TimerFunc = unsafe extern "system" fn(usize, u32, u32);
    pub type CtrlHandlerFunc = unsafe extern "system" fn(u32) -> u32;
}
mod string;

#[cfg(target_family = "windows")]
pub use self::inner::*;
pub use self::string::*;
