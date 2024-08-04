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

use core::cmp::Ordering;
use core::fmt;
use core::hash::Hasher;

use crate::io::{self, Write};
use crate::prelude::*;

pub struct Fnv64(u64);

impl Fnv64 {
    #[inline]
    pub const fn new() -> Fnv64 {
        Fnv64(0xCBF29CE484222325)
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
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Hasher::write(self, buf);
        Ok(buf.len())
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
    fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        for i in bytes.iter() {
            h = h.wrapping_mul(0x100000001B3);
            h ^= *i as u64;
        }
        self.0 = h;
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
impl fmt::Write for Fnv64 {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Hasher::write(self, s.as_bytes());
        Ok(())
    }
}
