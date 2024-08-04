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

use core::alloc::Allocator;
use core::cmp;

use crate::c2::cfg::OwnedConfig;
use crate::prelude::*;

pub const WRAP_HEX: u8 = 0xD0u8;
pub const WRAP_ZLIB: u8 = 0xD1u8;
pub const WRAP_GZIP: u8 = 0xD2u8;
pub const WRAP_BASE64: u8 = 0xD3u8;

pub(super) const WRAP_XOR: u8 = 0xD4u8;
pub(super) const WRAP_CBK: u8 = 0xD5u8;
pub(super) const WRAP_AES: u8 = 0xD6u8;

impl<A: Allocator> OwnedConfig<A> {
    pub fn wrap_xor(mut self, key: impl AsRef<[u8]>) -> OwnedConfig<A> {
        let b = key.as_ref();
        let c = cmp::min(0xFFFF, b.len());
        self.0.reserve(3 + c);
        self.0.push(WRAP_XOR);
        self.0.push((c >> 8) as u8);
        self.0.push(c as u8);
        self.0.extend_from_slice(&b[0..c as usize]);
        self
    }
    pub fn wrap_cbk(mut self, a: u8, b: u8, c: u8, d: u8) -> OwnedConfig<A> {
        self
    }
    pub fn wrap_aes<T: AsRef<[u8]>>(mut self, key: T, iv: T) -> OwnedConfig<A> {
        self
    }
    pub fn wrap_cbk_size(mut self, size: u8, a: u8, b: u8, c: u8, d: u8) -> OwnedConfig<A> {
        self
    }
}
