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
#![no_std]
#![feature(unchecked_shifts)]

extern crate core;

extern crate xrmt_io;

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::Ord;
use core::default::Default;
use core::marker::Copy;
use core::ops::{Deref, DerefMut};
use core::ptr::copy_nonoverlapping;
use core::result::Result::Ok;

use xrmt_io::{IoResult, Read};

#[cfg(feature = "sys")]
static RAND: RandStatic = RandStatic::empty();

pub struct Rand(u64);
#[cfg(feature = "sys")]
pub struct RandStatic(RandMut);
pub struct RandMut(UnsafeCell<Rand>);

impl Rand {
    #[inline]
    pub const fn empty() -> Rand {
        Rand(0u64)
    }
    #[inline]
    pub const fn with_seed(seed: u64) -> Rand {
        Rand(seed)
    }

    #[cfg(feature = "sys")]
    #[inline]
    pub fn new() -> Rand {
        let mut e = [0u8; 8];
        let _ = sys::system_rand(&mut e);
        Rand(u64::from_be_bytes(e))
    }
    #[cfg(feature = "sys")]
    #[inline]
    pub fn reseed(&mut self) {
        let mut e = [0u8; 8];
        let _ = sys::system_rand(&mut e);
        self.0 = u64::from_be_bytes(e);
    }

    #[inline]
    pub fn rand_u32(&mut self) -> u32 {
        unsafe { self.rand_u64().unchecked_shr(31) as u32 }
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
    #[inline]
    pub fn reseed_with(&mut self, v: u64) {
        self.0 = v
    }
    pub fn rand_u32n(&mut self, n: u32) -> u32 {
        if n == 0 {
            return self.rand_u32();
        }
        if n & (n - 1) == 0 {
            return self.rand_u32() & (n - 1);
        }
        let m = 0x7FFFFFFF - (0x80000000 % n as u32);
        let mut v = self.rand_u32();
        while v > m {
            v = self.rand_u32();
        }
        // https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        // Instead of: v % n
        unsafe { (v as u64 * n as u64).unchecked_shr(32) as u32 }
    }
    pub fn rand_u64n(&mut self, n: u64) -> u64 {
        if n == 0 {
            return self.rand_u64();
        }
        if n & (n - 1) == 0 {
            return self.rand_u64() & (n - 1);
        }
        let m = 0x7FFFFFFFFFFFFFFF - (0x8000000000000000 % n);
        let mut v = self.rand_u64();
        while v > m {
            v = self.rand_u64();
        }
        // https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
        // Instead of: v % n
        unsafe { (v as u128 * n as u128).unchecked_shr(64) as u64 }
    }
    pub fn read_into(&mut self, b: &mut [u8]) -> usize {
        if b.len() < 8 {
            let v = self.rand();
            let i = 8usize.min(b.len());
            unsafe { copy_nonoverlapping(v.as_ptr(), b.as_mut_ptr(), i) };
            return i;
        }
        let (c, r) = b.as_chunks_mut::<8>();
        let mut n = 0;
        if c.len() > 0 {
            for i in c {
                let v = self.rand();
                unsafe { copy_nonoverlapping(v.as_ptr(), i.as_mut_ptr(), 8) };
                n += 8;
            }
        }
        if r.len() > 0 {
            let v = self.rand();
            let i = 8usize.min(r.len());
            unsafe { copy_nonoverlapping(v.as_ptr(), r.as_mut_ptr(), i) };
            n += i;
        }
        n
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

    #[cfg(feature = "sys")]
    #[inline]
    pub fn new() -> RandMut {
        RandMut(UnsafeCell::new(Rand::new()))
    }
    #[cfg(feature = "sys")]
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
    pub fn reseed_with(&self, v: u64) {
        unsafe { &mut *self.0.get() }.reseed_with(v)
    }
    #[inline]
    pub fn rand_u32n(&self, n: u32) -> u32 {
        unsafe { &mut *self.0.get() }.rand_u32n(n)
    }
    #[inline]
    pub fn rand_u64n(&self, n: u64) -> u64 {
        unsafe { &mut *self.0.get() }.rand_u64n(n)
    }
    #[inline]
    pub fn read_into(&self, b: &mut [u8]) -> usize {
        unsafe { &mut *self.0.get() }.read_into(b)
    }
}
#[cfg(feature = "sys")]
impl RandStatic {
    #[inline]
    pub fn copy() -> Rand {
        RandStatic::get().clone()
    }
    #[inline]
    pub fn rand_u32() -> u32 {
        RandStatic::get().rand_u32()
    }
    #[inline]
    pub fn rand_u64() -> u64 {
        RandStatic::get().rand_u64()
    }
    #[inline]
    pub fn rand() -> [u8; 8] {
        RandStatic::get().rand()
    }
    #[inline]
    pub fn rand_u32n(n: u32) -> u32 {
        RandStatic::get().rand_u32n(n)
    }
    #[inline]
    pub fn rand_u64n(n: u64) -> u64 {
        RandStatic::get().rand_u64n(n)
    }
    #[inline]
    pub fn read_into(b: &mut [u8]) -> usize {
        RandStatic::get().read_into(b)
    }

    #[inline]
    const fn empty() -> RandStatic {
        RandStatic(RandMut::empty())
    }

    #[inline]
    fn get<'a>() -> &'a mut Rand {
        let r = unsafe { &mut *RAND.0 .0.get() };
        if r.0 == 0 {
            r.reseed();
        }
        r
    }
}

impl Read for Rand {
    #[inline]
    fn read(&mut self, b: &mut [u8]) -> IoResult<usize> {
        Ok(self.read_into(b))
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
        #[cfg(feature = "sys")]
        {
            Rand::new()
        }
        #[cfg(not(feature = "sys"))]
        {
            Rand::empty()
        }
    }
}

impl Read for RandMut {
    #[inline]
    fn read(&mut self, b: &mut [u8]) -> IoResult<usize> {
        Ok(self.deref_mut().read_into(b))
    }
}
impl Read for &RandMut {
    #[inline]
    fn read(&mut self, b: &mut [u8]) -> IoResult<usize> {
        Ok(unsafe { &mut *self.0.get() }.read_into(b))
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
        #[cfg(feature = "sys")]
        {
            RandMut::new()
        }
        #[cfg(not(feature = "sys"))]
        {
            RandMut::empty()
        }
    }
}
impl DerefMut for RandMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut Rand {
        unsafe { &mut *self.0.get() }
    }
}

#[cfg(feature = "sys")]
unsafe impl core::marker::Sync for RandStatic {}

#[cfg(all(target_family = "windows", feature = "sys"))]
mod sys {
    extern crate core;

    extern crate xrmt_io;
    extern crate xrmt_winapi;

    use core::result::Result::Ok;

    use xrmt_io::IoResult;
    use xrmt_winapi::functions::RtlGenRandom;

    #[inline]
    pub fn system_rand(b: &mut [u8]) -> IoResult<usize> {
        Ok(RtlGenRandom(b)?)
    }
}
#[cfg(all(
    feature = "sys",
    not(target_family = "windows"),
    any(
        target_os = "fuchsia",
        target_os = "netbsd",
        target_vendor = "apple",
        target_vendor = "fortanix",
    )
))]
mod sys {
    extern crate std;

    extern crate xrmt_crypt;
    extern crate xrmt_io;

    use std::fs::File;

    use xrmt_crypt::crypt;
    use xrmt_io::{IoResult, Read};

    #[inline]
    pub fn system_rand(b: &mut [u8]) -> IoResult<usize> {
        File::open(crypt!(0, "/dev/urandom"))?.read(b)
    }
}
#[cfg(all(
    feature = "sys",
    not(target_family = "windows"),
    not(target_os = "netbsd"),
    not(target_os = "fuchsia"),
    not(target_vendor = "apple"),
    not(target_vendor = "fortanix"),
))]
mod sys {
    extern crate std;

    extern crate libc;
    extern crate xrmt_crypt;
    extern crate xrmt_io;

    use std::fs::File;
    use std::result::Result::Ok;

    use xrmt_crypt::crypt;
    use xrmt_io::{IoResult, Read};

    #[inline]
    pub fn system_rand(b: &mut [u8]) -> IoResult<usize> {
        let r = unsafe { libc::getrandom(b.as_mut_ptr() as _, b.len(), libc::GRND_NONBLOCK) };
        if r <= 0 {
            File::open(crypt!(0, "/dev/urandom"))?.read(b)
        } else {
            Ok(r as usize)
        }
    }
}
