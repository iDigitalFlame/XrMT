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

use crate::util;
use crate::util::stx::io::{self, Read};
use crate::util::stx::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::system_rand;

pub struct Rand(u64);

impl Rand {
    #[inline]
    pub const fn empty() -> Rand {
        Rand(0)
    }
    #[inline]
    pub const fn with_seed(seed: u64) -> Rand {
        Rand(seed)
    }

    #[inline]
    pub fn new() -> Rand {
        let mut e = [0u8; 8];
        let _ = inner::system_rand(&mut e); // IGNORE ERROR
        Rand(u64::from_be_bytes(e))
    }

    #[inline]
    pub fn reseed(&mut self) {
        let mut e = [0u8; 8];
        let _ = inner::system_rand(&mut e); // IGNORE ERROR
        self.0 = u64::from_be_bytes(e);
    }
    #[inline]
    pub fn rand_u32(&mut self) -> u32 {
        (self.rand_u64() >> 31) as u32
    }
    #[inline]
    pub fn rand_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0xA0761D6478BD642F);
        let v = (self.0 as u128).wrapping_mul((self.0 ^ 0xE7037ED1A0B428DB) as u128);
        (v.wrapping_shr(64) ^ v) as u64
    }
    #[inline]
    pub fn rand(&mut self) -> [u8; 8] {
        self.rand_u64().to_be_bytes()
    }
    pub fn rand_u32n(&mut self, n: u32) -> u32 {
        if n == 0 {
            return self.rand_u32();
        }
        if n & (n - 1) == 0 {
            return self.rand_u32() & (n - 1);
        }
        let m = ((1 << 31) - 1 - (1 << 31) % n as u32) as u32;
        let mut v = self.rand_u32();
        while v > m {
            v = self.rand_u32();
        }
        // https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        // Instead of: v % n
        ((v as u64 * n as u64) >> 32) as u32
    }
    pub fn rand_u64n(&mut self, n: u64) -> u64 {
        if n == 0 {
            return self.rand_u64();
        }
        if n & (n - 1) == 0 {
            return self.rand_u64() & (n - 1);
        }
        let m = (1 << 63) - 1 - (1 << 63) % n;
        let mut v = self.rand_u64();
        while v > m {
            v = self.rand_u64();
        }
        // https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        // Instead of: v % n
        ((v as u128 * n as u128) >> 64) as u64
    }
}

impl Read for Rand {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len();
        if n < 8 {
            return Ok(util::copy(buf, &self.rand()));
        }
        let c = buf.len() / 8;
        let mut v = 0;
        for _ in 0..c {
            v += util::copy(&mut buf[v..], &self.rand());
        }
        if v < n {
            util::copy(&mut buf[v..], &self.rand());
        }
        Ok(n)
    }
}
impl Copy for Rand {}
impl Clone for Rand {
    #[inline]
    fn clone(&self) -> Rand {
        Rand(self.0)
    }
}
impl Default for Rand {
    #[inline]
    fn default() -> Rand {
        Rand::new()
    }
}

#[cfg(unix)]
mod inner {
    use crate::device::fs::File;
    use crate::util::crypt;
    use crate::util::stx::io::{self, Read};
    use crate::util::stx::prelude::*;

    #[inline]
    pub fn system_rand(buf: &mut [u8]) -> io::Result<usize> {
        File::open(crypt::get_or(0, "/dev/urandom"))?.read(buf)
    }
}
#[cfg(windows)]
mod inner {
    use crate::device::winapi;
    use crate::util::stx::io::{self, Error};
    use crate::util::stx::prelude::*;

    #[inline]
    pub fn system_rand(buf: &mut [u8]) -> io::Result<usize> {
        winapi::RtlGenRandom(buf).map_err(Error::from)
    }
}
