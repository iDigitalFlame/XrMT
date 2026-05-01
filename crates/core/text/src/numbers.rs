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
#![cfg(feature = "alloc")]

extern crate alloc;
extern crate core;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::convert::Into;
use core::fmt::{Formatter, Result};
use core::iter::{FusedIterator, Iterator};
use core::marker::Sized;
use core::mem::transmute;
use core::option::Option::{self, None, Some};

use crate::hex;

pub struct BinaryIter {
    cur: usize,
    num: usize,
    max: usize,
}

pub trait ToStr<const N: usize>: Into<u64> + Sized {
    #[inline]
    fn into_string(self) -> String {
        let mut r = [0u8; N];
        self.into_str(&mut r).to_string()
    }
    #[inline]
    fn into_buf(self, b: &mut [u8]) -> usize {
        write(b, self.into())
    }
    #[inline]
    fn into_str<'a>(self, b: &'a mut [u8]) -> &'a str {
        let r = self.into_buf(b);
        unsafe { transmute(b.get_unchecked(0..r)) }
    }
    #[inline]
    fn into_fmt(self, f: &mut Formatter<'_>) -> Result {
        let mut r = [0u8; N];
        f.write_str(self.into_str(&mut r))
    }
    #[inline]
    fn into_vec<A: Allocator>(self, b: &mut Vec<u8, A>) {
        let p = b.len();
        b.resize(p + N, 0);
        let n = self.into_buf(unsafe { b.get_unchecked_mut(p..) });
        b.truncate(p + n)
    }
}
pub trait ToStrHex<const N: usize>: Into<u64> + Sized {
    #[inline]
    fn into_hex_string(self) -> String {
        let mut b = [0u8; N];
        self.into_hex_str(&mut b).to_string()
    }
    #[inline]
    fn into_hex_buf(self, b: &mut [u8]) -> usize {
        write_hex(b, self.into())
    }
    #[inline]
    fn into_hex_str<'a>(self, b: &'a mut [u8]) -> &'a str {
        let r = self.into_hex_buf(b);
        unsafe { transmute(b.get_unchecked(0..r)) }
    }
    #[inline]
    fn into_hex_fmt(self, f: &mut Formatter<'_>) -> Result {
        let mut b = [0u8; N];
        f.write_str(self.into_hex_str(&mut b))
    }
    #[inline]
    fn into_hex_vec<A: Allocator>(self, b: &mut Vec<u8, A>) {
        let p = b.len();
        b.resize(p + N, 0);
        let n = self.into_hex_buf(unsafe { b.get_unchecked_mut(p..) });
        b.truncate(p + n)
    }
}
pub trait ToStrSigned<const N: usize>: Into<i64> + Sized {
    #[inline]
    fn into_string(self) -> String {
        let mut r = [0u8; N];
        self.into_str(&mut r).to_string()
    }
    #[inline]
    fn into_buf(self, b: &mut [u8]) -> usize {
        let v = self.into();
        if (b.len() < 1 && v < 0) || b.is_empty() {
            return 0;
        }
        if v < 0 {
            // 'b' can't be empty here.
            unsafe {
                *b.get_unchecked_mut(0) = 0x2D; // -
                write(&mut b.get_unchecked_mut(1..), v.unchecked_neg() as u64) + 1
            }
        } else {
            write(b, v as u64)
        }
    }
    #[inline]
    fn into_str<'a>(self, b: &'a mut [u8]) -> &'a str {
        let r = self.into_buf(b);
        unsafe { transmute(b.get_unchecked(0..r)) }
    }
    #[inline]
    fn into_fmt(self, f: &mut Formatter<'_>) -> Result {
        let mut r = [0u8; N];
        f.write_str(self.into_str(&mut r))
    }
    #[inline]
    fn into_vec<A: Allocator>(self, b: &mut Vec<u8, A>) {
        let p = b.len();
        b.resize(p + N, 0);
        let n = self.into_buf(unsafe { b.get_unchecked_mut(p..) });
        b.truncate(p + n)
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

impl ToStr<3> for u8 {}
impl ToStr<5> for u16 {}
impl ToStr<10> for u32 {}
impl ToStr<20> for u64 {}

impl ToStrHex<2> for u8 {
    #[inline]
    fn into_hex_buf(self, b: &mut [u8]) -> usize {
        hex_u8(b, self)
    }
}
impl ToStrHex<4> for u16 {}
impl ToStrHex<8> for u32 {}
impl ToStrHex<16> for u64 {}

impl ToStrSigned<4> for i8 {}
impl ToStrSigned<6> for i16 {}
impl ToStrSigned<11> for i32 {}
impl ToStrSigned<21> for i64 {}

impl Iterator for BinaryIter {
    type Item = (usize, bool);

    #[inline]
    fn next(&mut self) -> Option<(usize, bool)> {
        if self.cur > self.max {
            return None;
        }
        let (v, i) = (
            self.num & unsafe { 1usize.unchecked_shl(self.cur as u32) },
            self.cur,
        );
        self.cur += 1;
        Some((i, v != 0))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.max, Some(self.max))
    }
}
impl FusedIterator for BinaryIter {}

#[inline]
fn hex_u8(b: &mut [u8], v: u8) -> usize {
    // u8 to hex chars is a very simple conversion and is seperated for speed.
    if b.len() < 2 {
        return 0;
    }
    match v {
        0 => unsafe { *b.get_unchecked_mut(0) = 0x30 }, // 0
        0x1..0x10 => unsafe { *b.get_unchecked_mut(0) = hex(v) },
        _ => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4));
            *b.get_unchecked_mut(1) = hex(v);
            return 2;
        },
    }
    1
}
fn write(b: &mut [u8], s: u64) -> usize {
    if b.is_empty() {
        return 0;
    }
    let n = match s {
        0 => {
            unsafe { *b.get_unchecked_mut(0) = 0x30 }; // 0
            return 1;
        },
        1________________________..=9________________________ => 1,
        1_______________________0..=9_______________________9 => 2,
        1______________________00..=9______________________99 => 3,
        1_____________________000..=9_____________________999 => 4,
        1___________________0_000..=9___________________9_999 => 5,
        1__________________00_000..=9__________________99_999 => 6,
        1_________________000_000..=9_________________999_999 => 7,
        1_______________0_000_000..=9_______________9_999_999 => 8,
        1______________00_000_000..=9______________99_999_999 => 9,
        1_____________000_000_000..=9_____________999_999_999 => 10,
        1___________0_000_000_000..=9___________9_999_999_999 => 11,
        1__________00_000_000_000..=9__________99_999_999_999 => 12,
        1_________000_000_000_000..=9_________999_999_999_999 => 13,
        1_______0_000_000_000_000..=9_______9_999_999_999_999 => 14,
        1______00_000_000_000_000..=9______99_999_999_999_999 => 15,
        1_____000_000_000_000_000..=9_____999_999_999_999_999 => 16,
        1___0_000_000_000_000_000..=9___9_999_999_999_999_999 => 17,
        1__00_000_000_000_000_000..=9__99_999_999_999_999_999 => 18,
        1_000_000_000_000_000_000..=9_999_999_999_999_999_999 => 19,
        _ => 20,
    };
    if n > b.len() {
        return 0;
    }
    let mut v = s;
    for i in (1..n).rev() {
        let t = v.saturating_div(0xA);
        unsafe { *b.get_unchecked_mut(i) = 0x30 + (v - (t * 0xA)) as u8 };
        v = t;
        if v < 0xA {
            break;
        }
    }
    unsafe { *b.get_unchecked_mut(0) = 0x30 + (v as u8) };
    n
}
fn write_hex(b: &mut [u8], s: u64) -> usize {
    if b.is_empty() {
        return 0;
    }
    let n = match s {
        0 => {
            unsafe { *b.get_unchecked_mut(0) = 0x30 }; // 0
            return 1;
        },
        0x__________________1..=0x__________________F => 1,
        0x_________________10..=0x_________________FF => 2,
        0x________________100..=0x________________FFF => 3,
        0x______________1_000..=0x______________F_FFF => 4,
        0x_____________10_000..=0x_____________FF_FFF => 5,
        0x____________100_000..=0x____________FFF_FFF => 6,
        0x__________1_000_000..=0x__________F_FFF_FFF => 7,
        0x_________10_000_000..=0x_________FF_FFF_FFF => 8,
        0x________100_000_000..=0x________FFF_FFF_FFF => 9,
        0x______1_000_000_000..=0x______F_FFF_FFF_FFF => 10,
        0x_____10_000_000_000..=0x_____FF_FFF_FFF_FFF => 11,
        0x____100_000_000_000..=0x____FFF_FFF_FFF_FFF => 12,
        0x__1_000_000_000_000..=0x__F_FFF_FFF_FFF_FFF => 13,
        0x_10_000_000_000_000..=0x_FF_FFF_FFF_FFF_FFF => 14,
        0x100_000_000_000_000..=0xFFF_FFF_FFF_FFF_FFF => 15,
        _ => 16,
    };
    if n > b.len() {
        return 0;
    }
    let (l, mut p) = (16usize.saturating_sub(n), 0);
    for i in l..n + l {
        unsafe { *b.get_unchecked_mut(p) = hex(s.unchecked_shr((4 * (15 - i)) as u32) as u8) };
        p += 1;
    }
    n
}
