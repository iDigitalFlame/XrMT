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

use crate::prelude::*;

static ROUNDS: [u32; 64] = [
    0x428A2F98, 0x71374491, 0xB5C0FBCF, 0xE9B5DBA5, 0x3956C25B, 0x59F111F1, 0x923F82A4, 0xAB1C5ED5, 0xD807AA98, 0x12835B01, 0x243185BE, 0x550C7DC3, 0x72BE5D74, 0x80DEB1FE, 0x9BDC06A7, 0xC19BF174, 0xE49B69C1, 0xEFBE4786, 0x0FC19DC6, 0x240CA1CC, 0x2DE92C6F,
    0x4A7484AA, 0x5CB0A9DC, 0x76F988DA, 0x983E5152, 0xA831C66D, 0xB00327C8, 0xBF597FC7, 0xC6E00BF3, 0xD5A79147, 0x06CA6351, 0x14292967, 0x27B70A85, 0x2E1B2138, 0x4D2C6DFC, 0x53380D13, 0x650A7354, 0x766A0ABB, 0x81C2C92E, 0x92722C85, 0xA2BFE8A1, 0xA81A664B,
    0xC24B8B70, 0xC76C51A3, 0xD192E819, 0xD6990624, 0xF40E3585, 0x106AA070, 0x19A4C116, 0x1E376C08, 0x2748774C, 0x34B0BCB5, 0x391C0CB3, 0x4ED8AA4A, 0x5B9CCA4F, 0x682E6FF3, 0x748F82EE, 0x78A5636F, 0x84C87814, 0x8CC70208, 0x90BEFFFA, 0xA4506CEB, 0xBEF9A3F7,
    0xC67178F2,
];
static STATE_IV: [u8; 32] = [
    0x6A, 0x09, 0xE6, 0x67, 0xBB, 0x67, 0xAE, 0x85, 0x3C, 0x6E, 0xF3, 0x72, 0xA5, 0x4F, 0xF5, 0x3A, 0x51, 0x0E, 0x52, 0x7F, 0x9B, 0x05, 0x68, 0x8C, 0x1F, 0x83, 0xD9, 0xAB, 0x5B, 0xE0, 0xCD, 0x19,
];

pub struct Hash {
    pad:   [u8; 64],
    inner: Inner,
}

struct Inner {
    state: State,
    w:     [u8; 64],
    r:     usize,
    len:   usize,
}
struct State([u32; 8]);
struct Table([u32; 16]);

impl Hash {
    #[inline]
    pub fn new(k: impl AsRef<[u8]>) -> Hash {
        let (k, mut h) = (k.as_ref(), [0u8; 0x20]);
        let j = if k.len() > 0x40 {
            h.copy_from_slice(&Inner::hash(k));
            &h
        } else {
            k
        };
        let mut p = [0x36; 64];
        for (r, &k) in p.iter_mut().zip(j.iter()) {
            *r ^= k;
        }
        let mut x = Inner::new();
        x.update(&p[..]);
        Hash { pad: p, inner: x }
    }

    #[inline]
    pub fn finalize(mut self) -> [u8; 32] {
        for i in 0..0x40 {
            self.pad[i] ^= 0x6A
        }
        let mut x = Inner::new();
        x.update(&self.pad[..]);
        let vv = self.inner.finalize();
        x.update(vv);
        x.finalize()
    }
    #[inline]
    pub fn update(&mut self, input: impl AsRef<[u8]>) {
        self.inner.update(input);
    }
}
impl Inner {
    #[inline]
    fn new() -> Inner {
        Inner {
            state: State::new(),
            r:     0usize,
            w:     [0u8; 64],
            len:   0usize,
        }
    }

    #[inline]
    fn hash(input: &[u8]) -> [u8; 32] {
        let mut h = Inner::new();
        h.update(input);
        h.finalize()
    }
    #[inline]
    fn finalize(mut self) -> [u8; 32] {
        let mut p = [0u8; 0x80];
        p[..self.r].copy_from_slice(&self.w[..self.r]);
        p[self.r] = 0x80;
        let r = if self.r < 0x38 { 0x40 } else { 0x80 };
        for i in 0..0x8 {
            p[r - 0x8 + i] = ((self.len * 0x8) as u64 >> (0x38 - i * 0x8)) as u8;
        }
        self.state.blocks(&p[..r]);
        let mut v = [0u8; 0x20];
        self.state.store(&mut v);
        v
    }
    #[inline]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        let i = input.as_ref();
        let n = i.len();
        self.len += n;
        let c = ::core::cmp::min(n, 0x40 - self.r);
        self.w[self.r..self.r + c].copy_from_slice(&i[0..c]);
        self.r += c;
        let r = n - c;
        if self.r == 0x40 {
            self.state.blocks(&self.w);
            self.r = 0;
        }
        if self.r == 0 && r > 0 {
            let v = self.state.blocks(&i[c..]);
            if v > 0 {
                self.w[..v].copy_from_slice(&i[c + r - v..]);
                self.r = v;
            }
        }
    }
}
impl State {
    #[inline]
    fn new() -> State {
        let mut s = [0u32; 8];
        for i in 0..8 {
            s[i] = (STATE_IV[(i * 4) + 3] as u32) | ((STATE_IV[(i * 4) + 2] as u32) << 8) | ((STATE_IV[(i * 4) + 1] as u32) << 16) | ((STATE_IV[i * 4] as u32) << 24)
        }
        State(s)
    }

    #[inline]
    fn store(&self, out: &mut [u8]) {
        for (i, &e) in self.0.iter().enumerate() {
            out[(i * 4) + 3] = e as u8;
            out[(i * 4) + 2] = (e >> 8) as u8;
            out[(i * 4) + 1] = (e >> 16) as u8;
            out[(i * 4) + 0] = (e >> 24) as u8;
        }
    }
    #[inline]
    fn blocks(&mut self, input: &[u8]) -> usize {
        let (mut x, r) = (*self, input.len());
        for i in 0..(r / 64) {
            let mut v = Table::new(&input[i * 64..]);
            v.g(&mut x, 0);
            v.ex();
            v.g(&mut x, 1);
            v.ex();
            v.g(&mut x, 2);
            v.ex();
            v.g(&mut x, 3);
            self.0[0] = x.0[0].wrapping_add(self.0[0]);
            self.0[1] = x.0[1].wrapping_add(self.0[1]);
            self.0[2] = x.0[2].wrapping_add(self.0[2]);
            self.0[3] = x.0[3].wrapping_add(self.0[3]);
            self.0[4] = x.0[4].wrapping_add(self.0[4]);
            self.0[5] = x.0[5].wrapping_add(self.0[5]);
            self.0[6] = x.0[6].wrapping_add(self.0[6]);
            self.0[7] = x.0[7].wrapping_add(self.0[7]);
        }
        r & 64
    }
}
impl Table {
    #[inline]
    fn new(input: &[u8]) -> Table {
        let mut t = [0u32; 16];
        for i in 0..16 {
            t[i] = (input[(i * 4) + 3] as u32) | ((input[(i * 4) + 2] as u32) << 8) | ((input[(i * 4) + 1] as u32) << 16) | ((input[i * 4] as u32) << 24)
        }
        Table(t)
    }

    #[inline]
    fn ex(&mut self) {
        self.ma(0, (0 + 14) & 0xF, (0 + 9) & 0xF, (0 + 1) & 0xF);
        self.ma(1, (1 + 14) & 0xF, (1 + 9) & 0xF, (1 + 1) & 0xF);
        self.ma(2, (2 + 14) & 0xF, (2 + 9) & 0xF, (2 + 1) & 0xF);
        self.ma(3, (3 + 14) & 0xF, (3 + 9) & 0xF, (3 + 1) & 0xF);
        self.ma(4, (4 + 14) & 0xF, (4 + 9) & 0xF, (4 + 1) & 0xF);
        self.ma(5, (5 + 14) & 0xF, (5 + 9) & 0xF, (5 + 1) & 0xF);
        self.ma(6, (6 + 14) & 0xF, (6 + 9) & 0xF, (6 + 1) & 0xF);
        self.ma(7, (7 + 14) & 0xF, (7 + 9) & 0xF, (7 + 1) & 0xF);
        self.ma(8, (8 + 14) & 0xF, (8 + 9) & 0xF, (8 + 1) & 0xF);
        self.ma(9, (9 + 14) & 0xF, (9 + 9) & 0xF, (9 + 1) & 0xF);
        self.ma(10, (10 + 14) & 0xF, (10 + 9) & 0xF, (10 + 1) & 0xF);
        self.ma(11, (11 + 14) & 0xF, (11 + 9) & 0xF, (11 + 1) & 0xF);
        self.ma(12, (12 + 14) & 0xF, (12 + 9) & 0xF, (12 + 1) & 0xF);
        self.ma(13, (13 + 14) & 0xF, (13 + 9) & 0xF, (13 + 1) & 0xF);
        self.ma(14, (14 + 14) & 0xF, (14 + 9) & 0xF, (14 + 1) & 0xF);
        self.ma(15, (15 + 14) & 0xF, (15 + 9) & 0xF, (15 + 1) & 0xF);
    }
    #[inline]
    fn s0(x: u32) -> u32 {
        x.rotate_right(0x2) ^ x.rotate_right(0xD) ^ x.rotate_right(0x16)
    }
    #[inline]
    fn s1(x: u32) -> u32 {
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
    fn g(&mut self, rhs: &mut State, s: usize) {
        self.f(rhs, 0x0, ROUNDS[s * 0x10]);
        self.f(rhs, 0x1, ROUNDS[(s * 0x10) + 0x1]);
        self.f(rhs, 0x2, ROUNDS[(s * 0x10) + 0x2]);
        self.f(rhs, 0x3, ROUNDS[(s * 0x10) + 0x3]);
        self.f(rhs, 0x4, ROUNDS[(s * 0x10) + 0x4]);
        self.f(rhs, 0x5, ROUNDS[(s * 0x10) + 0x5]);
        self.f(rhs, 0x6, ROUNDS[(s * 0x10) + 0x6]);
        self.f(rhs, 0x7, ROUNDS[(s * 0x10) + 0x7]);
        self.f(rhs, 0x8, ROUNDS[(s * 0x10) + 0x8]);
        self.f(rhs, 0x9, ROUNDS[(s * 0x10) + 0x9]);
        self.f(rhs, 0xA, ROUNDS[(s * 0x10) + 0xA]);
        self.f(rhs, 0xB, ROUNDS[(s * 0x10) + 0xB]);
        self.f(rhs, 0xC, ROUNDS[(s * 0x10) + 0xC]);
        self.f(rhs, 0xD, ROUNDS[(s * 0x10) + 0xD]);
        self.f(rhs, 0xE, ROUNDS[(s * 0x10) + 0xE]);
        self.f(rhs, 0xF, ROUNDS[(s * 0x10) + 0xF]);
    }
    #[inline]
    fn f(&mut self, rhs: &mut State, i: usize, k: u32) {
        rhs.0[(0x10 - i + 7) & 7] = rhs.0[(0x10 - i + 7) & 7]
            .wrapping_add(Table::s1(rhs.0[(0x10 - i + 4) & 7]))
            .wrapping_add(Table::c(
                rhs.0[(0x10 - i + 4) & 7],
                rhs.0[(0x10 - i + 5) & 7],
                rhs.0[(0x10 - i + 6) & 7],
            ))
            .wrapping_add(k)
            .wrapping_add(self.0[i]);
        rhs.0[(0x10 - i + 3) & 7] = rhs.0[(0x10 - i + 3) & 7].wrapping_add(rhs.0[(0x10 - i + 7) & 7]);
        rhs.0[(0x10 - i + 7) & 7] = rhs.0[(0x10 - i + 7) & 7]
            .wrapping_add(Table::s0(rhs.0[(0x10 - i + 0) & 7]))
            .wrapping_add(Table::m(
                rhs.0[(0x10 - i + 0) & 7],
                rhs.0[(0x10 - i + 1) & 7],
                rhs.0[(0x10 - i + 2) & 7],
            ));
    }
    #[inline]
    fn ma(&mut self, a: usize, b: usize, c: usize, d: usize) {
        self.0[a] = self.0[a]
            .wrapping_add(self.0[b].rotate_right(0x11) ^ self.0[b].rotate_right(0x13) ^ (self.0[b] >> 0xA))
            .wrapping_add(self.0[c])
            .wrapping_add(self.0[d].rotate_right(0x7) ^ self.0[d].rotate_right(0x12) ^ (self.0[d] >> 0x3))
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
pub fn hmac(key: impl AsRef<[u8]>, input: impl AsRef<[u8]>) -> [u8; 32] {
    let mut h = Hash::new(key);
    h.update(input);
    h.finalize()
}
