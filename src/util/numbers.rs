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

use core::cmp;

use crate::util;
use crate::util::stx::prelude::*;

pub(crate) const HEXTABLE: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D', b'E', b'F',
];

pub trait ToStr: Sized {
    fn into_vec(self, buf: &mut Vec<u8>);
    fn into_buf(self, buf: &mut [u8]) -> usize;
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str;

    #[inline]
    fn into_string(self) -> String {
        let mut b = [0u8; 20];
        self.into_str(&mut b).to_string()
    }
}
pub trait ToStrHex: ToStr {
    fn into_hex_vec(self, buf: &mut Vec<u8>);
    fn into_hex_buf(self, buf: &mut [u8]) -> usize;
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str;

    #[inline]
    fn into_hex_string(self) -> String {
        let mut b = [0u8; 16];
        self.into_hex_str(&mut b).to_string()
    }
}

impl ToStr for i8 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStr for u8 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStrHex for u8 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        u8_hex_vec(buf, self)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        u8_hex_str(buf, self)
    }
}

impl ToStr for i16 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStr for u16 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStrHex for u16 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
}

impl ToStr for i32 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStr for u32 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStrHex for u32 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
}

impl ToStr for i64 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStr for u64 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self)
    }
}
impl ToStrHex for u64 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_vec(buf, self)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self)
    }
}

impl ToStr for isize {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStr for usize {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_str(buf, self as u64)
    }
}
impl ToStrHex for usize {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_str(buf, self as u64)
    }
}

#[inline]
fn uint_vec(buf: &mut Vec<u8>, v: u64) {
    let n = buf.len();
    buf.resize(n + 20, 0);
    let mut t = [0u8; 20];
    let r = _uint(&mut t, v);
    let i = util::copy(&mut buf[n..], &t[r..]);
    buf.truncate(n + i);
}
#[inline]
fn u8_hex_vec(buf: &mut Vec<u8>, v: u8) {
    let n = buf.len();
    buf.resize(n + 2, 0);
    let r = u8_hex(&mut buf[n..], v);
    buf.truncate(n + r);
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
        255..=65535 if t < 5 => return 0,
        65535..=4294967295 if t < 10 => return 0,
        4294967295..=18446744073709551615 if t < 20 => return 0,
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
    loop {
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
#[inline]
fn uint_hex_vec(buf: &mut Vec<u8>, v: u64) {
    let n = buf.len();
    buf.resize(n + 16, 0);
    let mut t = [0u8; 16];
    let r = _uint(&mut t, v);
    let i = util::copy(&mut buf[n..], &t[r..]);
    buf.truncate(n + i);
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
        0xFF..=0xFFFF if t < 4 => return 0,
        0xFFFF..=0xFFFFFFFF if t < 8 => return 0,
        0xFFFFFFFF..=0xFFFFFFFFFFFFFFFF if t < 16 => return 0,
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
    let (mut n, mut i) = (v as usize, 16);
    while n > 0xF {
        n = (v >> (4 * (16 - i))) as usize;
        i -= 1;
        buf[i] = HEXTABLE[n & 0xF];
    }
    i
}
#[inline]
fn uint_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    // Copy into the buf buffer provided. Order is still at the end of the slice.
    // Return the a str pointer that starts at the char start index.
    // Saves from an additional memcpy.
    let r = _uint(buf, v);
    unsafe { core::str::from_utf8_unchecked(&buf[r..cmp::min(20, buf.len())]) }
}
#[inline]
fn u8_hex_str<'a>(buf: &'a mut [u8], v: u8) -> &'a str {
    // u8 to hex chars is a very simple conversion and is seperated for speed.
    let r = u8_hex(buf, v);
    unsafe { core::str::from_utf8_unchecked(&buf[0..r]) }
}
#[inline]
fn uint_hex_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    // Copy into the buf buffer provided. Order is still at the end of the slice.
    // Return the a str pointer that starts at the char start index.
    // Saves from an additional memcpy.
    let r = _uint_hex(buf, v);
    unsafe { core::str::from_utf8_unchecked(&buf[r..cmp::min(16, buf.len())]) }
}
