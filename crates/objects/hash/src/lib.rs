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

use core::cmp::Ord;
use core::convert::From;
use core::fmt::{Debug, Formatter, Result};
use core::hash::Hasher;
use core::iter::{ExactSizeIterator, FusedIterator, Iterator};
use core::ops::Deref;
use core::option::Option::{self, None, Some};
use core::ptr::copy_nonoverlapping;
use core::result::Result::Ok;

mod fnv;
mod hmac;
mod sha256;

use xrmt_io::{IoResult, Read};

pub use self::fnv::*;
pub use self::hmac::*;
pub use self::sha256::*;

pub trait DigestHasher<const N: usize>: Hasher {
    fn digest(&self) -> Digest<N>;
}

pub struct Digest<const N: usize> {
    b: [u8; N],
    n: usize,
}

impl<const N: usize> Digest<N> {
    #[inline]
    pub fn reset(&mut self) {
        self.n = 0;
    }
    #[inline]
    pub fn size(&self) -> usize {
        N
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { self.b.get_unchecked(self.n..) }
    }
    #[inline]
    pub fn remaining(&self) -> usize {
        N.saturating_sub(self.n)
    }
    #[inline]
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        if self.remaining() == 0 {
            return 0;
        }
        let n = buf.len().min(self.remaining());
        unsafe { copy_nonoverlapping(self.b.as_ptr().add(self.n), buf.as_mut_ptr(), n) };
        self.n += n;
        n
    }

    #[inline]
    pub unsafe fn set_pos(&mut self, n: usize) {
        self.n = n.min(N);
    }
}

impl<const N: usize> Read for Digest<N> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        Ok(self.read(buf))
    }
}
impl<const N: usize> Debug for Digest<N> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(&self.b, f)
    }
}
impl<const N: usize> Deref for Digest<N> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl<const N: usize> Iterator for Digest<N> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.remaining() == 0 {
            return None;
        }
        let r = unsafe { *self.b.get_unchecked(self.n) };
        self.n += 1;
        Some(r)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining(), Some(self.remaining()))
    }
}
impl<const N: usize> From<[u8; N]> for Digest<N> {
    #[inline]
    fn from(v: [u8; N]) -> Digest<N> {
        Digest { b: v, n: 0usize }
    }
}
impl<const N: usize> FusedIterator for Digest<N> {}
impl<const N: usize> ExactSizeIterator for Digest<N> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining()
    }
}
