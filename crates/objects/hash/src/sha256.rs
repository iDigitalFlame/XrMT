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
use core::cmp::{Eq, Ord, PartialEq};
use core::convert::Into;
use core::default::Default;
use core::hash::Hasher;
use core::iter::Iterator;
use core::marker::Copy;
use core::ptr::{copy_nonoverlapping, write_bytes};
use core::result::Result::Ok;

use xrmt_io::{IoResult, Write};

use crate::{Digest, DigestHasher};

static STATES: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];
#[cfg_attr(rustfmt, rustfmt_skip)]
static ROUNDS: [u32; 64] = [
    0x428A2F98, 0x71374491, 0xB5C0FBCF, 0xE9B5DBA5, 0x3956C25B, 0x59F111F1, 0x923F82A4, 0xAB1C5ED5,
    0xD807AA98, 0x12835B01, 0x243185BE, 0x550C7DC3, 0x72BE5D74, 0x80DEB1FE, 0x9BDC06A7, 0xC19BF174,
    0xE49B69C1, 0xEFBE4786, 0x0FC19DC6, 0x240CA1CC, 0x2DE92C6F, 0x4A7484AA, 0x5CB0A9DC, 0x76F988DA,
    0x983E5152, 0xA831C66D, 0xB00327C8, 0xBF597FC7, 0xC6E00BF3, 0xD5A79147, 0x06CA6351, 0x14292967,
    0x27B70A85, 0x2E1B2138, 0x4D2C6DFC, 0x53380D13, 0x650A7354, 0x766A0ABB, 0x81C2C92E, 0x92722C85,
    0xA2BFE8A1, 0xA81A664B, 0xC24B8B70, 0xC76C51A3, 0xD192E819, 0xD6990624, 0xF40E3585, 0x106AA070,
    0x19A4C116, 0x1E376C08, 0x2748774C, 0x34B0BCB5, 0x391C0CB3, 0x4ED8AA4A, 0x5B9CCA4F, 0x682E6FF3,
    0x748F82EE, 0x78A5636F, 0x84C87814, 0x8CC70208, 0x90BEFFFA, 0xA4506CEB, 0xBEF9A3F7, 0xC67178F2,
];

pub struct Sha256 {
    v:   State,
    buf: [u8; 64],
    len: usize,
    pos: usize,
}

struct State([u32; 8]);
struct Table([u32; 16]);

impl State {
    #[inline]
    const fn new() -> State {
        State(STATES)
    }

    #[inline]
    fn sum(&self) -> [u8; 32] {
        unsafe {
            [
                self.0.get_unchecked(0).unchecked_shr(24) as u8,
                self.0.get_unchecked(0).unchecked_shr(16) as u8,
                self.0.get_unchecked(0).unchecked_shr(8) as u8,
                *self.0.get_unchecked(0) as u8,
                self.0.get_unchecked(1).unchecked_shr(24) as u8,
                self.0.get_unchecked(1).unchecked_shr(16) as u8,
                self.0.get_unchecked(1).unchecked_shr(8) as u8,
                *self.0.get_unchecked(1) as u8,
                self.0.get_unchecked(2).unchecked_shr(24) as u8,
                self.0.get_unchecked(2).unchecked_shr(16) as u8,
                self.0.get_unchecked(2).unchecked_shr(8) as u8,
                *self.0.get_unchecked(2) as u8,
                self.0.get_unchecked(3).unchecked_shr(24) as u8,
                self.0.get_unchecked(3).unchecked_shr(16) as u8,
                self.0.get_unchecked(3).unchecked_shr(8) as u8,
                *self.0.get_unchecked(3) as u8,
                self.0.get_unchecked(4).unchecked_shr(24) as u8,
                self.0.get_unchecked(4).unchecked_shr(16) as u8,
                self.0.get_unchecked(4).unchecked_shr(8) as u8,
                *self.0.get_unchecked(4) as u8,
                self.0.get_unchecked(5).unchecked_shr(24) as u8,
                self.0.get_unchecked(5).unchecked_shr(16) as u8,
                self.0.get_unchecked(5).unchecked_shr(8) as u8,
                *self.0.get_unchecked(5) as u8,
                self.0.get_unchecked(6).unchecked_shr(24) as u8,
                self.0.get_unchecked(6).unchecked_shr(16) as u8,
                self.0.get_unchecked(6).unchecked_shr(8) as u8,
                *self.0.get_unchecked(6) as u8,
                self.0.get_unchecked(7).unchecked_shr(24) as u8,
                self.0.get_unchecked(7).unchecked_shr(16) as u8,
                self.0.get_unchecked(7).unchecked_shr(8) as u8,
                *self.0.get_unchecked(7) as u8,
            ]
        }
    }
    #[inline]
    fn blocks(self, v: &[u8]) -> (State, usize) {
        let (mut c, mut t, n) = (self, self, v.len());
        let (s, r) = v.as_chunks::<64>();
        for i in s {
            exp(i, &mut c, &mut t);
        }
        if r.len() > 0 {
            exp(r, &mut c, &mut t);
        }
        (c, n & 64)
    }
}

impl Table {
    #[inline]
    fn new(v: &[u8]) -> Table {
        let mut t = [0u32; 16];
        for (i, b) in t.iter_mut().enumerate() {
            *b = unsafe { (*v.get_unchecked((i * 4) + 3) as u32) | (*v.get_unchecked((i * 4) + 2) as u32).unchecked_shl(8) | (*v.get_unchecked((i * 4) + 1) as u32).unchecked_shl(16) | (*v.get_unchecked(i * 4) as u32).unchecked_shl(24) };
        }
        Table(t)
    }

    #[inline]
    fn x(&mut self) {
        self.m(0x0);
        self.m(0x1);
        self.m(0x2);
        self.m(0x3);
        self.m(0x4);
        self.m(0x5);
        self.m(0x6);
        self.m(0x7);
        self.m(0x8);
        self.m(0x9);
        self.m(0xA);
        self.m(0xB);
        self.m(0xC);
        self.m(0xD);
        self.m(0xE);
        self.m(0xF);
    }
    #[inline]
    fn m(&mut self, i: usize) {
        unsafe {
            let (a, b, c, d) = (
                *self.0.get_unchecked(i),
                *self.0.get_unchecked((i + 0xE) & 0xF),
                *self.0.get_unchecked((i + 0x9) & 0xF),
                *self.0.get_unchecked((i + 0x1) & 0xF),
            );
            *self.0.get_unchecked_mut(i) = a
                .wrapping_add(b.rotate_right(0x11) ^ b.rotate_right(0x13) ^ b.unchecked_shr(0xA))
                .wrapping_add(c)
                .wrapping_add(d.rotate_right(0x07) ^ d.rotate_right(0x12) ^ d.unchecked_shr(0x3));
        }
    }
    #[inline]
    fn g(&mut self, r: &mut [u32; 8], s: usize) {
        self.f(r, 0x0, s);
        self.f(r, 0x1, s);
        self.f(r, 0x2, s);
        self.f(r, 0x3, s);
        self.f(r, 0x4, s);
        self.f(r, 0x5, s);
        self.f(r, 0x6, s);
        self.f(r, 0x7, s);
        self.f(r, 0x8, s);
        self.f(r, 0x9, s);
        self.f(r, 0xA, s);
        self.f(r, 0xB, s);
        self.f(r, 0xC, s);
        self.f(r, 0xD, s);
        self.f(r, 0xE, s);
        self.f(r, 0xF, s);
    }
    #[inline]
    fn f(&mut self, v: &mut [u32; 8], i: usize, k: usize) {
        unsafe {
            *v.get_unchecked_mut((0x10 - i + 7) & 7) = v
                .get_unchecked((0x10 - i + 7) & 7)
                .wrapping_add(f(*v.get_unchecked((0x10 - i + 4) & 7)))
                .wrapping_add(c(
                    *v.get_unchecked((0x10 - i + 4) & 7),
                    *v.get_unchecked((0x10 - i + 5) & 7),
                    *v.get_unchecked((0x10 - i + 6) & 7),
                ))
                .wrapping_add(*ROUNDS.get_unchecked((k * 0x10) + i))
                .wrapping_add(*self.0.get_unchecked(i));
            *v.get_unchecked_mut((0x10 - i + 3) & 7) = v
                .get_unchecked((0x10 - i + 3) & 7)
                .wrapping_add(*v.get_unchecked((0x10 - i + 7) & 7));
            *v.get_unchecked_mut((0x10 - i + 7) & 7) = v
                .get_unchecked((0x10 - i + 7) & 7)
                .wrapping_add(s(*v.get_unchecked((0x10 - i) & 7)))
                .wrapping_add(m(
                    *v.get_unchecked((0x10 - i) & 7),
                    *v.get_unchecked((0x10 - i + 1) & 7),
                    *v.get_unchecked((0x10 - i + 2) & 7),
                ))
        }
    }
}
impl Sha256 {
    #[inline]
    pub const fn new() -> Sha256 {
        Sha256 {
            v:   State::new(),
            buf: [0u8; 64],
            len: 0usize,
            pos: 0usize,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        unsafe { write_bytes(self.buf.as_mut_ptr(), 0, 64) };
        (self.len, self.pos, self.v) = (0, 0, State::new());
    }
    #[inline]
    pub fn sum(&self) -> [u8; 32] {
        let mut b = [0u8; 128];
        unsafe {
            copy_nonoverlapping(self.buf.as_ptr(), b.as_mut_ptr(), self.pos);
            *b.get_unchecked_mut(self.pos) = 0x80;
        }
        let v = if self.pos < 0x38 { 64 } else { 128 };
        unsafe {
            *b.get_unchecked_mut(v - 8) = (((self.len * 8) as u64).unchecked_shr(0x38)) as u8; // 0
            *b.get_unchecked_mut(v - 7) = (((self.len * 8) as u64).unchecked_shr(0x30)) as u8; // 1
            *b.get_unchecked_mut(v - 6) = (((self.len * 8) as u64).unchecked_shr(0x28)) as u8; // 2
            *b.get_unchecked_mut(v - 5) = (((self.len * 8) as u64).unchecked_shr(0x20)) as u8; // 3
            *b.get_unchecked_mut(v - 4) = (((self.len * 8) as u64).unchecked_shr(0x18)) as u8; // 4
            *b.get_unchecked_mut(v - 3) = (((self.len * 8) as u64).unchecked_shr(0x10)) as u8; // 5
            *b.get_unchecked_mut(v - 2) = (((self.len * 8) as u64).unchecked_shr(0x08)) as u8; // 6
            *b.get_unchecked_mut(v - 1) = (self.len * 8) as u8; // 7
        }
        let (s, _) = self.v.blocks(unsafe { b.get_unchecked(0..v) });
        s.sum()
    }
    #[inline]
    pub fn update(&mut self, b: &[u8]) {
        let (n, v) = (b.len(), b.len().min(64 - self.pos));
        self.len += n;
        unsafe { copy_nonoverlapping(b.as_ptr(), self.buf.as_mut_ptr().add(self.pos), v) };
        self.pos += v;
        let r = n - v;
        if self.pos == 64 {
            let (s, _) = self.v.blocks(&mut self.buf);
            (self.v, self.pos) = (s, 0);
        }
        if self.pos > 0 || r == 0 {
            return;
        }
        let (s, i) = unsafe { self.v.blocks(b.get_unchecked(v..)) };
        if i > 0 {
            unsafe { copy_nonoverlapping(b.as_ptr().add(v + r - i), self.buf.as_mut_ptr(), i) };
            self.pos = i;
        }
        self.v = s;
    }
}

impl Eq for Sha256 {}
impl Clone for Sha256 {
    #[inline]
    fn clone(&self) -> Sha256 {
        Sha256 {
            v:   self.v,
            buf: self.buf.clone(),
            len: self.len,
            pos: self.pos,
        }
    }
}
impl Write for Sha256 {
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
impl Hasher for Sha256 {
    #[inline]
    fn finish(&self) -> u64 {
        let mut h = 0xCBF29CE484222325u64;
        for i in self.sum() {
            h = h.wrapping_mul(0x100000001B3);
            h ^= i as u64;
        }
        h
    }
    #[inline]
    fn write(&mut self, b: &[u8]) {
        self.update(b);
    }
}
impl Default for Sha256 {
    #[inline]
    fn default() -> Sha256 {
        Sha256::new()
    }
}
impl PartialEq for Sha256 {
    #[inline]
    fn eq(&self, other: &Sha256) -> bool {
        self.len.eq(&other.len) && self.pos.eq(&other.pos) && self.buf.eq(&other.buf) && self.v.0.eq(&other.v.0)
    }
}
impl DigestHasher<32> for Sha256 {
    #[inline]
    fn digest(&self) -> Digest<32> {
        self.sum().into()
    }
}

impl Copy for State {}
impl Clone for State {
    #[inline]
    fn clone(&self) -> State {
        State(self.0.clone())
    }
}

#[inline]
fn s(x: u32) -> u32 {
    x.rotate_right(0x2) ^ x.rotate_right(0xD) ^ x.rotate_right(0x16)
}
#[inline]
fn f(x: u32) -> u32 {
    x.rotate_right(0x6) ^ x.rotate_right(0xB) ^ x.rotate_right(0x19)
}
#[inline]
fn c(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}
#[inline]
fn m(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}
#[inline]
fn exp(v: &[u8], s: &mut State, x: &mut State) {
    let mut t = Table::new(v);
    t.g(&mut x.0, 0);
    t.x();
    t.g(&mut x.0, 1);
    t.x();
    t.g(&mut x.0, 2);
    t.x();
    t.g(&mut x.0, 3);
    unsafe {
        *s.0.get_unchecked_mut(0) = x.0.get_unchecked(0).wrapping_add(*s.0.get_unchecked(0));
        *s.0.get_unchecked_mut(1) = x.0.get_unchecked(1).wrapping_add(*s.0.get_unchecked(1));
        *s.0.get_unchecked_mut(2) = x.0.get_unchecked(2).wrapping_add(*s.0.get_unchecked(2));
        *s.0.get_unchecked_mut(3) = x.0.get_unchecked(3).wrapping_add(*s.0.get_unchecked(3));
        *s.0.get_unchecked_mut(4) = x.0.get_unchecked(4).wrapping_add(*s.0.get_unchecked(4));
        *s.0.get_unchecked_mut(5) = x.0.get_unchecked(5).wrapping_add(*s.0.get_unchecked(5));
        *s.0.get_unchecked_mut(6) = x.0.get_unchecked(6).wrapping_add(*s.0.get_unchecked(6));
        *s.0.get_unchecked_mut(7) = x.0.get_unchecked(7).wrapping_add(*s.0.get_unchecked(7));
    }
}
