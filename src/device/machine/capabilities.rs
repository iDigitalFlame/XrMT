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
#![allow(dead_code)]

pub const CAP_MEMORY_MAPPER: u32 = 0x1;
pub const CAP_BUGS: u32 = 0x2;
pub const CAP_IMPLANT: u32 = 0x4;
pub const CAP_CRYPT: u32 = 0x8;
pub const CAP_EWS: u32 = 0x10;
pub const CAP_KEY: u32 = 0x20;
pub const CAP_PROXY: u32 = 0x40;
pub const CAP_MULTI_PROXY: u32 = 0x80;
pub const CAP_PROC_ENUM: u32 = 0x100;
pub const CAP_REGEXP: u32 = 0x200;
pub const CAP_LIMIT_LARGE: u32 = 0x400;
pub const CAP_LIMIT_MEDIUM: u32 = 0x800;
pub const CAP_LIMIT_SMALL: u32 = 0x1000;
pub const CAP_RANDOM_TYPE: u32 = 0x2000;
pub const CAP_MEMORY_SWEEPER: u32 = 0x4000;
pub const CAP_FUNCMAP: u32 = 0x8000;
pub const CAP_ALTLOAD: u32 = 0x10000;
pub const CAP_CHUNK_HEAP: u32 = 0x20000;
pub const CAP_RUST: u32 = 0x40000;
pub const CAP_LIMIT_STANDARD: u32 = CAP_LIMIT_LARGE | CAP_LIMIT_MEDIUM;
pub const CAP_LIMIT_TINY: u32 = CAP_LIMIT_LARGE | CAP_LIMIT_MEDIUM | CAP_LIMIT_SMALL;

const CAP_SET_PROXY: u32 = if cfg!(all(feature = "noproxy", not(feature = "multiproxy"))) {
    0
} else if cfg!(all(not(feature = "noproxy"), feature = "multiproxy")) {
    CAP_PROXY | CAP_MULTI_PROXY
} else {
    CAP_PROXY
};

const CAP_LIMIT: u32 = if cfg!(all(
    feature = "limit_nofrag",
    not(feature = "limit_tiny"),
    not(feature = "limit_small"),
    not(feature = "limit_medium"),
    not(feature = "limit_large")
)) {
    0
} else if cfg!(all(
    feature = "limit_tiny",
    not(feature = "limit_small"),
    not(feature = "limit_medium"),
    not(feature = "limit_large"),
    not(feature = "limit_nofrag")
)) {
    CAP_LIMIT_TINY
} else if cfg!(all(
    feature = "limit_small",
    not(feature = "limit_tiny"),
    not(feature = "limit_medium"),
    not(feature = "limit_large"),
    not(feature = "limit_nofrag")
)) {
    CAP_LIMIT_SMALL
} else if cfg!(all(
    feature = "limit_medium",
    not(feature = "limit_tiny"),
    not(feature = "limit_small"),
    not(feature = "limit_large"),
    not(feature = "limit_nofrag")
)) {
    CAP_LIMIT_MEDIUM
} else if cfg!(all(
    feature = "limit_large",
    not(feature = "limit_tiny"),
    not(feature = "limit_small"),
    not(feature = "limit_medium"),
    not(feature = "limit_nofrag")
)) {
    CAP_LIMIT_LARGE
} else {
    CAP_LIMIT_STANDARD
};

pub const ENABLED: u32 = CAP_RUST
    | CAP_ALTLOAD
    | CAP_FUNCMAP
    | CAP_LIMIT
    | CAP_SET_PROXY
    | if cfg!(feature = "nosweep") {
        0
    } else {
        CAP_MEMORY_SWEEPER
    }
    | if cfg!(feature = "crypt") { CAP_CRYPT } else { 0 }
    | if cfg!(feature = "implant") { CAP_IMPLANT } else { 0 }
    | if cfg!(feature = "swap") { CAP_PROC_ENUM } else { 0 }
    | if cfg!(feature = "ews") { CAP_EWS } else { 0 }
    | if cfg!(feature = "nokeyset") { 0 } else { CAP_KEY }
    | if cfg!(feature = "bugs") { CAP_BUGS } else { 0 };
