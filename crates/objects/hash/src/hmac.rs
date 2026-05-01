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

extern crate core;

extern crate xrmt_io;

use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::AsRef;
use core::default::Default;
use core::hash::Hasher;
use core::iter::Iterator;
use core::ops::BitXorAssign;
use core::result::Result::Ok;

use xrmt_io::{IoResult, Write};

use crate::{Digest, DigestHasher, Sha256};

pub struct HMAC<const N: usize, T: DigestHasher<N>> {
    h:   T,
    buf: [u8; 64],
}

pub type Sha256HMAC = HMAC<32, Sha256>;

impl<const N: usize, T: DigestHasher<N>> HMAC<N, T> {
    #[inline]
    pub fn update(&mut self, b: &[u8]) {
        self.h.write(b);
    }
}
impl<const N: usize, T: Default + DigestHasher<N>> HMAC<N, T> {
    #[inline]
    pub fn new(key: impl AsRef<[u8]>) -> HMAC<N, T> {
        let (b, mut v) = (key.as_ref(), [0u8; N]);
        let r = if b.len() > 64 {
            let mut t = T::default();
            t.write(b);
            v.copy_from_slice(&t.digest());
            v.as_slice()
        } else {
            b
        };
        let mut p = [0x36; 64];
        for (i, k) in p.iter_mut().zip(r.iter()) {
            i.bitxor_assign(*k);
        }
        let mut h = T::default();
        h.write(p.as_slice());
        HMAC { buf: p, h }
    }
}

impl<const N: usize, T: DigestHasher<N>> Write for HMAC<N, T> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.update(buf);
        Ok(buf.len())
    }
}
impl<const N: usize, T: Eq + DigestHasher<N>> Eq for HMAC<N, T> {}
impl<const N: usize, T: Clone + DigestHasher<N>> Clone for HMAC<N, T> {
    #[inline]
    fn clone(&self) -> HMAC<N, T> {
        HMAC {
            h:   self.h.clone(),
            buf: self.buf.clone(),
        }
    }
}
impl<const N: usize, T: Default + DigestHasher<N>> Hasher for HMAC<N, T> {
    #[inline]
    fn finish(&self) -> u64 {
        let mut h = 0xCBF29CE484222325u64;
        for i in self.digest().iter() {
            h = h.wrapping_mul(0x100000001B3);
            h ^= *i as u64;
        }
        h
    }
    #[inline]
    fn write(&mut self, b: &[u8]) {
        self.update(b);
    }
}
impl<const N: usize, T: PartialEq + DigestHasher<N>> PartialEq for HMAC<N, T> {
    #[inline]
    fn eq(&self, other: &HMAC<N, T>) -> bool {
        self.buf.eq(&other.buf) && self.h.eq(&other.h)
    }
}
impl<const N: usize, T: Default + DigestHasher<N>> DigestHasher<N> for HMAC<N, T> {
    #[inline]
    fn digest(&self) -> Digest<N> {
        let mut b = self.buf.clone();
        for i in b.iter_mut() {
            i.bitxor_assign(0x6A);
        }
        let mut h: T = T::default();
        h.write(b.as_slice());
        h.write(&self.h.digest());
        h.digest()
    }
}
