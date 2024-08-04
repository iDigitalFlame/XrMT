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
//pub use self::entries::*;
pub(super) use self::os::*;
//pub(super) use self::io::*;
pub(super) use self::process::*;

mod entries;
mod io;
mod os;
mod process;

pub struct TID;

impl TID {
    // Prefix keys:
    // - Sv: System ID Value, actions preformed inside the network loop In rust
    //   these are anything that modifies the Session struct, as it's memory might
    //   be frozen, so we can't edit it while in that state.
    //
    // - Rv: Result ID Value.
    //
    // - Mv: Mux ID Value, performed outside the network loop in a separate Thread.
    //   These values consist of "core" functions.
    //
    // - Tv: Task ID Value. Performed in the same Thread as the Mv values, these are
    //   "not" core functions and can be extended or modified by custom code.

    pub const SV_ECHO: u8 = 0x00u8;
    pub const SV_RESYNC: u8 = 0x01u8;
    pub const SV_HELLO: u8 = 0x02u8;
    pub const SV_REGISTER: u8 = 0x03u8;
    pub const SV_COMPLETE: u8 = 0x04u8;
    pub const SV_SHUTDOWN: u8 = 0x05u8;
    pub const SV_DROP: u8 = 0x06u8;
    pub const SV_REFRESH: u8 = 0x07u8; // Convert from a Mv* in Golang to Sv
    pub const SV_TIME: u8 = 0x08u8; // Convert from a Mv* in Golang to Sv
    pub const SV_PROFILE: u8 = 0x12u8; // Convert from a Mv* in Golang to Sv
    pub const SV_PROXY: u8 = 0x0Bu8;
    pub const SV_SPAWN: u8 = 0x0Cu8;
    pub const SV_MIGRATE: u8 = 0x0Du8;

    pub const MV_PWD: u8 = 0x09u8;
    pub const MV_CWD: u8 = 0x0Au8;
    pub const MV_DEBUG_CHECK: u8 = 0x0Eu8;
    pub const MV_LIST: u8 = 0x0Fu8;
    pub const MV_MOUNTS: u8 = 0x10u8;
    pub const MV_PS: u8 = 0x11u8;
    pub const MV_WHOAMI: u8 = 0x13u8;
    pub const MV_SCRIPT: u8 = 0xF0u8;

    pub const TV_DOWNLOAD: u8 = 0xC0u8;
    pub const TV_UPLOAD: u8 = 0xC1u8;
    pub const TV_EXECUTE: u8 = 0xC2u8;
    pub const TV_ASSEMBLY: u8 = 0xC3u8;
    pub const TV_ZOMBIE: u8 = 0xC4u8;
    pub const TV_DLL: u8 = 0xC5u8;
    pub const TV_CHECK: u8 = 0xC6u8;
    pub const TV_PATCH: u8 = 0xC7u8;
    pub const TV_PULL: u8 = 0xC8u8;
    pub const TV_PULL_EXECUTE: u8 = 0xC9u8;
    pub const TV_RENAME: u8 = 0xCAu8;
    pub const TV_SCREEN_SHOT: u8 = 0xCBu8;
    pub const TV_DUMP_PROC: u8 = 0xCCu8;
    pub const TV_REV_TO_SELF: u8 = 0xCDu8;
    pub const TV_REGISTRY: u8 = 0xCEu8;
    pub const TV_IO: u8 = 0xCFu8;
    pub const TV_EVADE: u8 = 0xD0u8;
    pub const TV_TROLL: u8 = 0xD1u8;
    pub const TV_UI: u8 = 0xD2u8;
    pub const TV_WINDOW_LIST: u8 = 0xD3u8;
    pub const TV_LOGIN: u8 = 0xD4u8;
    pub const TV_ELEVATE: u8 = 0xD5u8;
    pub const TV_WAIT: u8 = 0xD6u8;
    pub const TV_UNTRUST: u8 = 0xD7u8;
    pub const TV_POWER: u8 = 0xD8u8;
    pub const TV_NETCAT: u8 = 0xD9u8;
    pub const TV_LOGINS: u8 = 0xDAu8;
    pub const TV_LOGINS_ACTION: u8 = 0xDBu8;
    pub const TV_LOGINS_PROC: u8 = 0xDCu8;
    pub const TV_FUNCMAP: u8 = 0xDDu8;
    pub const TV_FUNCMAP_LIST: u8 = 0xDEu8;

    pub const RV_RESULT: u8 = 0x14u8;
}
