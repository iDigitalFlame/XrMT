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
        let mut b = [0u8; 21];
        self.into_str(&mut b).to_string()
    }
}
pub trait ToStrHex: ToStr {
    fn into_hex_vec(self, buf: &mut Vec<u8>);
    fn into_hex_buf(self, buf: &mut [u8]) -> usize;
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str;

    #[inline]
    fn into_hex_string(self) -> String {
        let mut b = [0u8; 20];
        self.into_hex_str(&mut b).to_string()
    }
}

impl ToStr for i8 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStr for u8 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStrHex for u8 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint8_hex_to_vec(buf, self)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint8_hex_to_str(buf, self)
    }
}

impl ToStr for i16 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStr for u16 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStrHex for u16 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_to_str(buf, self as u64)
    }
}

impl ToStr for i32 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStr for u32 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStrHex for u32 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_to_str(buf, self as u64)
    }
}

impl ToStr for i64 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStr for u64 {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self)
    }
}
impl ToStrHex for u64 {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_to_vec(buf, self)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex_to_buf(buf, self)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_to_str(buf, self)
    }
}

impl ToStr for isize {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStr for usize {
    #[inline]
    fn into_vec(self, buf: &mut Vec<u8>) {
        uint_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_buf(self, buf: &mut [u8]) -> usize {
        uint_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_to_str(buf, self as u64)
    }
}
impl ToStrHex for usize {
    #[inline]
    fn into_hex_vec(self, buf: &mut Vec<u8>) {
        uint_hex_to_vec(buf, self as u64)
    }
    #[inline]
    fn into_hex_buf(self, buf: &mut [u8]) -> usize {
        uint_hex_to_buf(buf, self as u64)
    }
    #[inline]
    fn into_hex_str<'a>(self, buf: &'a mut [u8]) -> &'a str {
        uint_hex_to_str(buf, self as u64)
    }
}

#[inline]
fn uint_to_vec(buf: &mut Vec<u8>, v: u64) {
    let n = buf.len();
    buf.resize(n + 21, 0);
    let r = uint_to_buf(&mut buf[n..], v);
    buf.truncate(n + r);
}
#[inline]
fn uint_hex_to_vec(buf: &mut Vec<u8>, v: u64) {
    let n = buf.len();
    buf.resize(n + 20, 0);
    let r = uint_hex_to_buf(&mut buf[n..], v);
    buf.truncate(n + r);
}
#[inline]
fn uint8_hex_to_vec(buf: &mut Vec<u8>, v: u8) {
    let n = buf.len();
    buf.resize(n + 2, 0);
    let r = uint8_hex_to_buf(&mut buf[n..], v);
    buf.truncate(n + r);
}
fn uint_to_buf(buf: &mut [u8], v: u64) -> usize {
    if buf.len() < 21 {
        // fix to cacul;ate the size based on the resize value
        return 0;
    }
    if v == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut b = [0u8; 21];
    let (mut i, mut v) = (20, v);
    while v >= 10 {
        let n = v / 10;
        b[i] = b'0' + (v - (n * 10)) as u8;
        i -= 1;
        v = n;
    }
    b[i] = b'0' + v as u8;
    util::copy(buf, &b[i..])
}
fn uint_hex_to_buf(buf: &mut [u8], v: u64) -> usize {
    if buf.len() < 20 {
        return 0;
    }
    if v == 0 {
        buf[0] = b'0';
        return 1;
    }
    let (mut b, mut i) = ([0u8; 20], 19);
    loop {
        let n = (v >> (4 * (19 - i))) as usize;
        b[i] = HEXTABLE[n & 0xF];
        i -= 1;
        if n <= 0xF {
            break;
        }
    }
    util::copy(buf, &b[i + 1..])
}
#[inline]
fn uint8_hex_to_buf(buf: &mut [u8], v: u8) -> usize {
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
fn uint_to_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    if buf.len() < 21 {
        return unsafe { core::str::from_utf8_unchecked(buf) };
    }
    if v == 0 {
        buf[0] = b'0';
        unsafe { core::str::from_utf8_unchecked(&buf[0..1]) };
    }
    let (mut i, mut v) = (buf.len() - 1, v);
    while v >= 10 {
        let n = v / 10;
        buf[i] = b'0' + (v - (n * 10)) as u8;
        i -= 1;
        v = n;
    }
    buf[i] = b'0' + v as u8;
    unsafe { core::str::from_utf8_unchecked(&buf[i..]) }
}
fn uint_hex_to_str<'a>(buf: &'a mut [u8], v: u64) -> &'a str {
    if buf.len() < 20 {
        return unsafe { core::str::from_utf8_unchecked(buf) };
    }
    if v == 0 {
        buf[0] = b'0';
        unsafe { core::str::from_utf8_unchecked(&buf[0..1]) };
    }
    let t = buf.len() - 1;
    let mut i = t;
    loop {
        let n = (v >> (4 * (t - i))) as usize;
        buf[i] = HEXTABLE[n & 0xF];
        i -= 1;
        if n <= 0xF {
            break;
        }
    }
    unsafe { core::str::from_utf8_unchecked(&buf[i + 1..]) }
}
#[inline]
fn uint8_hex_to_str<'a>(buf: &'a mut [u8], v: u8) -> &'a str {
    if buf.len() < 2 {
        return unsafe { core::str::from_utf8_unchecked(buf) };
    }
    let r = match v {
        0 => {
            buf[0] = b'0';
            1
        },
        1..=16 => {
            buf[0] = HEXTABLE[(v as usize) & 0x0F];
            1
        },
        _ => {
            buf[0] = HEXTABLE[(v as usize) >> 4];
            buf[1] = HEXTABLE[(v as usize) & 0x0F];
            2
        },
    };
    unsafe { core::str::from_utf8_unchecked(&buf[0..r]) }
}
