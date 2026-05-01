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

extern crate core;

extern crate xrmt_io;

use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::Into;
use core::default::Default;
use core::hash::Hasher;
use core::marker::Copy;
use core::option::Option;
use core::result::Result::Ok;

use xrmt_io::{IoResult, Write};

use crate::{Digest, DigestHasher};

pub struct Fnv32(u32);
pub struct Fnv64(u64);

impl Fnv32 {
    #[inline]
    pub const fn new() -> Fnv32 {
        Fnv32(0x811C9DC5u32)
    }

    #[inline]
    pub fn reset(&mut self) {
        self.0 = 0x811C9DC5u32;
    }
    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
    #[inline]
    pub fn update(&mut self, b: &[u8]) {
        let mut h = self.0;
        for i in b.iter() {
            h = h.wrapping_mul(0x1000193);
            h ^= *i as u32;
        }
        self.0 = h;
    }
}
impl Fnv64 {
    #[inline]
    pub const fn new() -> Fnv64 {
        Fnv64(0xCBF29CE484222325u64)
    }

    #[inline]
    pub fn reset(&mut self) {
        self.0 = 0xCBF29CE484222325u64;
    }
    #[inline]
    pub fn value(&self) -> u64 {
        self.0
    }
    #[inline]
    pub fn update(&mut self, b: &[u8]) {
        let mut h = self.0;
        for i in b.iter() {
            h = h.wrapping_mul(0x100000001B3);
            h ^= *i as u64;
        }
        self.0 = h;
    }
}

impl Eq for Fnv32 {}
impl Ord for Fnv32 {
    #[inline]
    fn cmp(&self, other: &Fnv32) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Copy for Fnv32 {}
impl Write for Fnv32 {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        self.update(b);
        Ok(b.len())
    }
}
impl Clone for Fnv32 {
    #[inline]
    fn clone(&self) -> Fnv32 {
        Fnv32(self.0)
    }
}
impl Hasher for Fnv32 {
    #[inline]
    fn finish(&self) -> u64 {
        self.0 as u64
    }
    #[inline]
    fn write(&mut self, b: &[u8]) {
        self.update(b);
    }
}
impl Default for Fnv32 {
    #[inline]
    fn default() -> Fnv32 {
        Fnv32::new()
    }
}
impl PartialEq for Fnv32 {
    #[inline]
    fn eq(&self, other: &Fnv32) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for Fnv32 {
    #[inline]
    fn partial_cmp(&self, other: &Fnv32) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl DigestHasher<4> for Fnv32 {
    #[inline]
    fn digest(&self) -> Digest<4> {
        self.0.to_be_bytes().into()
    }
}

impl Eq for Fnv64 {}
impl Ord for Fnv64 {
    #[inline]
    fn cmp(&self, other: &Fnv64) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Copy for Fnv64 {}
impl Write for Fnv64 {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        self.update(b);
        Ok(b.len())
    }
}
impl Clone for Fnv64 {
    #[inline]
    fn clone(&self) -> Fnv64 {
        Fnv64(self.0)
    }
}
impl Hasher for Fnv64 {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, b: &[u8]) {
        self.update(b)
    }
}
impl Default for Fnv64 {
    #[inline]
    fn default() -> Fnv64 {
        Fnv64::new()
    }
}
impl PartialEq for Fnv64 {
    #[inline]
    fn eq(&self, other: &Fnv64) -> bool {
        self.0.eq(&other.0)
    }
}
impl PartialOrd for Fnv64 {
    #[inline]
    fn partial_cmp(&self, other: &Fnv64) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl DigestHasher<8> for Fnv64 {
    #[inline]
    fn digest(&self) -> Digest<8> {
        self.0.to_be_bytes().into()
    }
}
