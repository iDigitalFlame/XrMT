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

extern crate core;

use core::clone::Clone;
use core::marker::Copy;

core::cfg_match! {
    cfg(all(feature = "limit_no_frag", not(feature = "limit_tiny"), not(feature = "limit_small"), not(feature = "limit_medium"), not(feature = "limit_large"))) => {
        const fn _limit() -> u32 { 0 }
    }
    cfg(all(feature = "limit_tiny", not(feature = "limit_small"), not(feature = "limit_medium"), not(feature = "limit_large"), not(feature = "limit_no_frag"))) => {
        const fn _limit() -> u32 { LIMIT_TINY }
    }
    cfg(all(feature = "limit_small", not(feature = "limit_tiny"), not(feature = "limit_medium"), not(feature = "limit_large"), not(feature = "limit_no_frag"))) => {
        const fn _limit() -> u32 { LIMIT_SMALL }
    }
    cfg(all(feature = "limit_medium", not(feature = "limit_tiny"), not(feature = "limit_small"), not(feature = "limit_large"), not(feature = "limit_no_frag"))) => {
        const fn _limit() -> u32 { LIMIT_MEDIUM }
    }
    cfg(all(feature = "limit_large", not(feature = "limit_tiny"), not(feature = "limit_small"), not(feature = "limit_medium"), not(feature = "limit_no_frag"))) => {
        const fn _limit() -> u32 { LIMIT_LARGE }
    }
    _ => {
        const fn _limit() -> u32 { LIMIT_STANDARD }
    }
}

/*
Unused:
    pub const CAP_RANDOM_TYPE: u32 = 0x2000; // Rust doesn't have fastrand.
    pub const CAP_MEMORY_SWEEPER: u32 = 0x4000; // Memory sweeper isn't needed in Rust.
*/

pub const NULL: Capabilities = Capabilities(0);
pub const ENABLED: Capabilities = Capabilities(
    0x20 // Key: Always Enabled
    | 0x8000u32 // FuncMap: Always enabled
    | 0x10000u32 // AltLoad: Always enabled
    | 0x20000u32 // Chunk Heaps: Always enabled
    | 0x40000u32 // Rust: Always enabled
    | LIMIT
    | PROXY_TYPE
    | if cfg!(feature = "ews") { EWS } else { 0u32 }
    | if cfg!(feature = "bugs") { BUGS } else { 0u32 }
    | if cfg!(feature = "crypt") { CRYPT } else { 0u32 }
    | if cfg!(feature = "strip") { STRIP } else { 0u32 }
    | if cfg!(feature = "snap") { PROC_ENUM } else { 0u32 },
);

pub const MEMORY_MAP: u32 = 0x1u32;
pub const BUGS: u32 = 0x2u32;
pub const STRIP: u32 = 0x4u32;
pub const CRYPT: u32 = 0x8u32;
pub const EWS: u32 = 0x10u32;
pub const PROXY: u32 = 0x40u32;
pub const MULTI_PROXY: u32 = 0x80u32;
pub const PROC_ENUM: u32 = 0x100u32;
pub const REGEXP: u32 = 0x200u32;
pub const LIMIT_LARGE: u32 = 0x400u32;
pub const LIMIT_MEDIUM: u32 = 0x800u32;
pub const LIMIT_SMALL: u32 = 0x1000u32;
pub const FUNCMAP: u32 = 0u32;

pub const LIMIT_STANDARD: u32 = LIMIT_LARGE | LIMIT_MEDIUM;
pub const LIMIT_TINY: u32 = LIMIT_LARGE | LIMIT_MEDIUM | LIMIT_SMALL;

const LIMIT: u32 = _limit();
const PROXY_TYPE: u32 = if cfg!(feature = "no_proxy") {
    0u32
} else {
    PROXY | MULTI_PROXY
};
// Rust is either no_proxy or multi proxy, it has no single.

pub struct Capabilities(pub u32);

impl Copy for Capabilities {}
impl Clone for Capabilities {
    #[inline]
    fn clone(&self) -> Capabilities {
        Capabilities(self.0)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::device::machine::capabilities::{Capabilities, LIMIT_LARGE, LIMIT_MEDIUM, LIMIT_STANDARD, LIMIT_TINY, MULTI_PROXY};
    use crate::prelude::*;
    use crate::util::BinaryIter;

    impl Debug for Capabilities {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Capabilities").field(&self.0).finish()
        }
    }
    impl Display for Capabilities {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            for (i, e) in BinaryIter::u32(self.0) {
                match i {
                    0 if e => f.write_str("mem_sections;"),
                    0 => f.write_str("mem_alloc_write;"),
                    1 if e => f.write_str("bugtrack;"),
                    2 if e => f.write_str("stripped;"),
                    3 if e => f.write_str("crypt_mapper;"),
                    4 if e => f.write_str("encrypt_while_sleep;"),
                    5 if e => f.write_str("key_crypt;"),
                    6 if e && self.0 & MULTI_PROXY == MULTI_PROXY => f.write_str("proxy_multi;"),
                    6 if e => f.write_str("proxy_single;"),
                    8 if e => f.write_str("enum_snap;"),
                    8 => f.write_str("enum_qsi;"),
                    9 if e => f.write_str("fast_regexp;"),
                    10 if self.0 & LIMIT_TINY == LIMIT_TINY => f.write_str("limits_tiny;"),
                    10 if self.0 & LIMIT_STANDARD == LIMIT_STANDARD => f.write_str("limits_standard;"),
                    10 if self.0 & LIMIT_LARGE == LIMIT_LARGE => f.write_str("limits_large;"),
                    10 if self.0 & LIMIT_MEDIUM == LIMIT_MEDIUM => f.write_str("limits_medium;"),
                    10 => f.write_str("limits_disabled;"),
                    13 if e => f.write_str("fast_rand;"),
                    14 if e => f.write_str("mem_sweep;"),
                    15 if e => f.write_str("funcmap;"),
                    16 if e => f.write_str("alt_loader;"),
                    17 if e => f.write_str("chunk_heap;"),
                    18 if e => f.write_str("rust;"),
                    _ => Ok(()),
                }?;
            }
            Ok(())
        }
    }
}
