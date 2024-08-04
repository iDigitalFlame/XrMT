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
use core::fmt::Formatter;
use core::str::from_utf8_unchecked;
use core::{cmp, fmt};

use crate::prelude::*;
use crate::util;

pub(crate) const HEXTABLE: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D', b'E', b'F',
];

pub struct BinaryIter {
    cur: usize,
    num: usize,
    max: usize,
}

pub trait ToStr: Sized {
    fn into_buf(self, buf: &mut [u8]) -> usize;
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str;
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>);

    #[inline]
    fn into_string(self) -> String {
        let mut b = [0u8; 20];
        self.into_str(&mut b).to_string()
    }
    #[inline]
    fn into_formatter(self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut b = [0u8; 20];
        f.write_str(self.into_str(&mut b))
    }
}
pub trait ToStrHex: ToStr {
    fn into_hex_buf(self, buf: &mut [u8]) -> usize;
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str;
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>);

    #[inline]
    fn into_hex_string(self) -> String {
        let mut b = [0u8; 16];
        self.into_hex_str(&mut b).to_string()
    }
    #[inline]
    fn into_hex_formatter(self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut b = [0u8; 0xA];
        f.write_str(self.into_hex_str(&mut b))
    }
}

impl BinaryIter {
    #[inline]
    pub const fn u8(val: u8) -> BinaryIter {
        BinaryIter {
            cur: 0usize,
            num: val as usize,
            max: 7usize,
        }
    }
    #[inline]
    pub const fn u16(val: u16) -> BinaryIter {
        BinaryIter {
            cur: 0usize,
            num: val as usize,
            max: 15usize,
        }
    }
    #[inline]
    pub const fn u32(val: u32) -> BinaryIter {
        BinaryIter {
            cur: 0usize,
            num: val as usize,
            max: 31usize,
        }
    }
    #[inline]
    pub const fn new(val: usize, iters: usize) -> BinaryIter {
        BinaryIter {
            cur: 0usize,
            num: val,
            max: iters,
        }
    }
}

impl Iterator for BinaryIter {
    type Item = (usize, bool);

    fn next(&mut self) -> Option<(usize, bool)> {
        if self.cur > self.max {
            return None;
        }
        let (v, i) = (self.num & (1 << self.cur), self.cur);
        self.cur += 1;
        Some((i, v != 0))
    }
}

impl ToStr for i8 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStr for u8 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStrHex for u8 {
    #[inline(always)]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        u8_hex_str(buf, self)
    }
    #[inline(always)]
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        u8_hex_vec(buf, self)
    }
}

impl ToStr for i16 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStr for u16 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStrHex for u16 {
    #[inline(always)]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_hex_vec(buf, self as u64)
    }
}

impl ToStr for i32 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStr for u32 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStrHex for u32 {
    #[inline(always)]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_hex_vec(buf, self as u64)
    }
}

impl ToStr for i64 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStr for u64 {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self)
    }
}
impl ToStrHex for u64 {
    #[inline(always)]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self)
    }
    #[inline(always)]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self)
    }
    #[inline(always)]
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_hex_vec(buf, self)
    }
}

impl ToStr for isize {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStr for usize {
    #[inline(always)]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline(always)]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_vec(buf, self as u64)
    }
}
impl ToStrHex for usize {
    #[inline(always)]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
    #[inline(always)]
    fn into_hex_vec<A: Allocator>(self, buf: &mut Vec<u8, A>) {
        uint_hex_vec(buf, self as u64)
    }
}

fn uint(out: &mut [u8], v: u64) -> usize {
    // Copy into the out buffer, but order them so the chars are at the beginning
    // of the slice.
    // Return the amount of chars placed.
    let t = out.len();
    match v {
        0 if t < 1 => return 0,
        0 => {
            out[0] = b'0';
            return 1;
        },
        0..=255 if t < 3 => return 0,
        256..=65535 if t < 5 => return 0,
        65536..=4294967295 if t < 10 => return 0,
        4294967296..=18446744073709551615 if t < 20 => return 0,
        _ => (),
    }
    let mut t = [0u8; 20];
    let r = _uint(&mut t, v);
    util::copy(out, &t[r..])
}
fn _uint(buf: &mut [u8], v: u64) -> usize {
    // Let buf be our backing store for chars.
    // Chars are ordered at the end of the 20bytes length.
    // Return the first char position in the slice.
    if buf.len() < 20 {
        return 0;
    }
    if v == 0 {
        buf[19] = b'0';
        return 19;
    }
    let (mut x, mut i) = (v, 19);
    while x >= 10 {
        let y = x / 10;
        buf[i] = b'0' + (x - (y * 10)) as u8;
        i -= 1;
        x = y;
        if y < 10 {
            break;
        }
    }
    buf[i] = b'0' + x as u8;
    i
}
#[inline]
fn u8_hex(buf: &mut [u8], v: u8) -> usize {
    // u8 to hex chars is a very simple conversion and is seperated for speed.
    if buf.len() < 2 {
        return 0;
    }
    match v {
        0 => buf[0] = b'0',
        1..=16 => buf[0] = HEXTABLE[(v as usize) & 0x0F],
        _ => {
            buf[0] = HEXTABLE[(v as usize) >> 4];
            buf[1] = HEXTABLE[(v as usize) & 0x0F];
            return 2;
        },
    }
    1
}
fn uint_hex(out: &mut [u8], v: u64) -> usize {
    // Copy into the out buffer, but order them so the chars are at the beginning
    // of the slice.
    // Return the amount of chars placed.
    let t = out.len();
    match v {
        0 if t < 1 => return 0,
        0 => {
            out[0] = b'0';
            return 1;
        },
        0..=0xFF if t < 2 => return 0,
        0x100..=0xFFFF if t < 4 => return 0,
        0x10000..=0xFFFFFFFF if t < 8 => return 0,
        0x100000000..=0xFFFFFFFFFFFFFFFF if t < 16 => return 0,
        _ => (),
    }
    let mut t = [0u8; 16];
    let r = _uint_hex(&mut t, v);
    util::copy(out, &t[r..])
}
fn _uint_hex(buf: &mut [u8], v: u64) -> usize {
    // Let buf be our backing store for chars.
    // Chars are ordered at the end of the 16bytes length.
    // Return the first char position in the slice.
    if buf.len() < 16 {
        return 0;
    }
    if v == 0 {
        buf[15] = b'0';
        return 15;
    }
    let mut i = 16;
    loop {
        let n = (v >> (4 * (16 - i))) as usize;
        i -= 1;
        buf[i] = HEXTABLE[n & 0xF];
        if n <= 0xF {
            break;
        }
    }
    i
}
#[inline]
fn uint_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    // Copy into the buf buffer provided. Order is still at the end of the slice.
    // Return the a str pointer that starts at the char start index.
    // Saves from an additional memcpy.
    let r = _uint(buf, v);
    // Shift to remove leading zero.
    let p = (&buf[r..]).iter().position(|v| *v > b'0').map_or(r, |x| r + x);
    unsafe { from_utf8_unchecked(&buf[p..cmp::min(20, buf.len())]) }
}
#[inline]
fn u8_hex_str<'a>(buf: &'a mut [u8], v: u8) -> &'a str {
    // u8 to hex chars is a very simple conversion and is seperated for speed.
    let r = u8_hex(buf, v);
    unsafe { from_utf8_unchecked(&buf[0..r]) }
}
#[inline]
fn uint_vec<A: Allocator>(buf: &mut Vec<u8, A>, v: u64) {
    let n = buf.len();
    buf.resize(n + 20, 0);
    let mut t = [0u8; 20];
    let r = _uint(&mut t, v);
    // Shift to remove leading zero.
    let p = (&t[r..]).iter().position(|v| *v > b'0').map_or(r, |x| r + x);
    let i = util::copy(&mut buf[n..], &t[p..]);
    buf.truncate(n + i);
}
#[inline]
fn u8_hex_vec<A: Allocator>(buf: &mut Vec<u8, A>, v: u8) {
    let n = buf.len();
    buf.resize(n + 2, 0);
    let r = u8_hex(&mut buf[n..], v);
    buf.truncate(n + r);
}
#[inline]
fn uint_hex_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    // Copy into the buf buffer provided. Order is still at the end of the slice.
    // Return the a str pointer that starts at the char start index.
    // Saves from an additional memcpy.
    let r = _uint_hex(buf, v);
    unsafe { from_utf8_unchecked(&buf[r..cmp::min(16, buf.len())]) }
}
#[inline]
fn uint_hex_vec<A: Allocator>(buf: &mut Vec<u8, A>, v: u64) {
    let n = buf.len();
    buf.resize(n + 16, 0);
    let mut t = [0u8; 16];
    let r = _uint(&mut t, v);
    let i = util::copy(&mut buf[n..], &t[r..]);
    buf.truncate(n + i);
}

#[macro_export]
macro_rules! number_like {
    ($Me:ident, $Num:ty) => {
        impl core::ops::Add for $Me {
            type Output = $Me;

            #[inline]
            fn add(self, rhs: $Me) -> $Me {
                $Me(self.0 + rhs.0)
            }
        }
        impl core::ops::Sub for $Me {
            type Output = $Me;

            #[inline]
            fn sub(self, rhs: $Me) -> $Me {
                $Me(self.0 - rhs.0)
            }
        }
        impl core::ops::Deref for $Me {
            type Target = $Num;

            #[inline]
            fn deref(&self) -> &$Num {
                &self.0
            }
        }
        impl core::ops::BitOr for $Me {
            type Output = $Me;

            #[inline]
            fn bitor(self, rhs: $Me) -> $Me {
                $Me(self.0 | rhs.0)
            }
        }
        impl core::ops::BitXor for $Me {
            type Output = $Me;

            #[inline]
            fn bitxor(self, rhs: $Me) -> $Me {
                $Me(self.0 ^ rhs.0)
            }
        }
        impl core::ops::BitAnd for $Me {
            type Output = $Me;

            #[inline]
            fn bitand(self, rhs: $Me) -> $Me {
                $Me(self.0 & rhs.0)
            }
        }
        impl core::clone::Clone for $Me {
            #[inline]
            fn clone(&self) -> $Me {
                $Me(self.0)
            }
        }
        impl core::marker::Copy for $Me {}
        impl core::ops::AddAssign for $Me {
            #[inline]
            fn add_assign(&mut self, rhs: $Me) {
                self.0 += rhs.0
            }
        }
        impl core::ops::SubAssign for $Me {
            #[inline]
            fn sub_assign(&mut self, rhs: $Me) {
                self.0 -= rhs.0
            }
        }
        impl core::ops::BitOrAssign for $Me {
            #[inline]
            fn bitor_assign(&mut self, rhs: $Me) {
                self.0 |= rhs.0
            }
        }
        impl core::ops::BitXorAssign for $Me {
            #[inline]
            fn bitxor_assign(&mut self, rhs: $Me) {
                self.0 ^= rhs.0
            }
        }
        impl core::ops::BitAndAssign for $Me {
            #[inline]
            fn bitand_assign(&mut self, rhs: $Me) {
                self.0 &= rhs.0
            }
        }
        impl core::convert::From<$Num> for $Me {
            #[inline]
            fn from(v: $Num) -> $Me {
                $Me(v)
            }
        }
        impl core::cmp::PartialEq<$Me> for $Me {
            #[inline]
            fn eq(&self, other: &$Me) -> bool {
                self.0.eq(&other.0)
            }
        }
        impl core::cmp::PartialEq<$Num> for $Me {
            #[inline]
            fn eq(&self, other: &$Num) -> bool {
                self.0.eq(&other)
            }
        }
        impl core::cmp::PartialOrd<$Num> for $Me {
            fn partial_cmp(&self, other: &$Num) -> core::option::Option<core::cmp::Ordering> {
                self.0.partial_cmp(&other)
            }
        }
    };
}
