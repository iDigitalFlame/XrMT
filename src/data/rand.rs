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

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

use crate::io::{self, Read};
use crate::prelude::*;
use crate::{ignore_error, util};

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::system_rand;

pub struct Rand(u64);
pub struct RandMut(UnsafeCell<Rand>);

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
        ignore_error!(inner::system_rand(&mut e));
        Rand(u64::from_be_bytes(e))
    }

    #[inline]
    pub fn reseed(&mut self) {
        let mut e = [0u8; 8];
        ignore_error!(inner::system_rand(&mut e));
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
    pub fn read_into(&mut self, buf: &mut [u8]) {
        let n = buf.len();
        if n < 8 {
            util::copy(buf, &self.rand());
            return;
        }
        let c = buf.len() / 8;
        let mut v = 0;
        for _ in 0..c {
            v += util::copy(&mut buf[v..], &self.rand());
        }
        if v < n {
            util::copy(&mut buf[v..], &self.rand());
        }
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
impl RandMut {
    #[inline]
    pub const fn empty() -> RandMut {
        RandMut(UnsafeCell::new(Rand::empty()))
    }
    #[inline]
    pub const fn with_seed(seed: u64) -> RandMut {
        RandMut(UnsafeCell::new(Rand::with_seed(seed)))
    }

    #[inline]
    pub fn new() -> RandMut {
        RandMut(UnsafeCell::new(Rand::new()))
    }

    #[inline]
    pub fn reseed(&self) {
        unsafe { &mut *self.0.get() }.reseed()
    }
    #[inline]
    pub fn rand_u32(&self) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32()
    }
    #[inline]
    pub fn rand_u64(&self) -> u64 {
        unsafe { &mut *self.0.get() }.rand_u64()
    }
    #[inline]
    pub fn rand(&self) -> [u8; 8] {
        unsafe { &mut *self.0.get() }.rand()
    }
    #[inline]
    pub fn read_into(&self, buf: &mut [u8]) {
        unsafe { &mut *self.0.get() }.read_into(buf)
    }
    #[inline]
    pub fn rand_u32n(&self, n: u32) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32n(n)
    }
    #[inline]
    pub fn rand_u64n(&self, n: u64) -> u64 {
        unsafe { &mut *self.0.get() }.rand_u64n(n)
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

impl Read for RandMut {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.deref_mut().read(buf)
    }
}
impl Clone for RandMut {
    #[inline]
    fn clone(&self) -> RandMut {
        RandMut(UnsafeCell::new(*self.deref()))
    }
}
impl Deref for RandMut {
    type Target = Rand;

    #[inline]
    fn deref(&self) -> &Rand {
        unsafe { &*self.0.get() }
    }
}
impl Default for RandMut {
    #[inline]
    fn default() -> RandMut {
        RandMut::new()
    }
}
impl DerefMut for RandMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut Rand {
        unsafe { &mut *self.0.get() }
    }
}

#[cfg(target_family = "windows")]
mod inner {
    use crate::device::winapi;
    use crate::io::{self, Error};
    use crate::prelude::*;

    #[inline]
    pub fn system_rand(buf: &mut [u8]) -> io::Result<usize> {
        winapi::RtlGenRandom(buf).map_err(Error::from)
    }
}
#[cfg(all(
    not(target_family = "windows"),
    any(
        target_os = "fuchsia",
        target_os = "netbsd",
        target_vendor = "apple",
        target_vendor = "fortanix",
    )
))]
mod inner {
    use crate::fs::File;
    use crate::io::{self, Read};
    use crate::util::crypt;

    #[inline]
    pub fn system_rand(buf: &mut [u8]) -> io::Result<usize> {
        File::open(crypt::get_or(0, "/dev/urandom"))?.read(buf)
    }
}
#[cfg(all(
    not(target_family = "windows"),
    not(target_os = "netbsd"),
    not(target_os = "fuchsia"),
    not(target_vendor = "apple"),
    not(target_vendor = "fortanix"),
))]
mod inner {
    extern crate libc;

    use crate::fs::File;
    use crate::io::{self, Read};
    use crate::prelude::*;
    use crate::util::crypt;

    #[inline]
    pub fn system_rand(buf: &mut [u8]) -> io::Result<usize> {
        // 0x1 - GRND_NONBLOCK
        let r = unsafe { libc::getrandom(buf.as_mut_ptr() as _, buf.len(), libc::GRND_NONBLOCK) };
        if r == -1 || buf.len() > 0 && r == 0 {
            File::open(crypt::get_or(0, "/dev/urandom"))?.read(buf)
        } else {
            Ok(r as usize)
        }
    }
}
