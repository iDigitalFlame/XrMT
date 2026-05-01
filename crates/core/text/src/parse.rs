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

extern crate core;

use core::option::Option::{self, None, Some};

#[inline]
pub fn parse_u8(s: &str) -> Option<u8> {
    parse(s.as_bytes(), 0xFF, 3, false).map(|v| v as u8)
}
#[inline]
pub fn parse_u16(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF, 5, false).map(|v| v as u16)
}
#[inline]
pub fn parse_u32(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF_FFFF, 10, false).map(|v| v as u16)
}
#[inline]
pub fn parse_u64(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF_FFFF_FFFF_FFFF, 20, false).map(|v| v as u16)
}

#[inline]
pub fn parse_u8_hex(s: &str) -> Option<u8> {
    parse(s.as_bytes(), 0xFF, 2, true).map(|v| v as u8)
}
#[inline]
pub fn parse_u16_hex(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF, 4, true).map(|v| v as u16)
}
#[inline]
pub fn parse_u32_hex(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF_FFFF, 8, true).map(|v| v as u16)
}
#[inline]
pub fn parse_u64_hex(s: &str) -> Option<u16> {
    parse(s.as_bytes(), 0xFFFF_FFFF_FFFF_FFFF, 16, false).map(|v| v as u16)
}

fn parse(b: &[u8], m: u64, c: usize, hex: bool) -> Option<u64> {
    let (mut n, mut v) = (0u64, 0usize);
    for i in b {
        let x = match *i {
            (b'A'..=b'F') if hex => *i - 0x37,
            (b'a'..=b'f') if hex => *i - 0x57,
            (b'A'..=b'F') | (b'a'..=b'f') => return None,
            (b'0'..=b'9') => *i - 0x30,
            _ => return None,
        };
        if (x >= 0x10 && hex) || (x >= 0xA && !hex) {
            return None;
        }
        if n >= m || v + 1 > c {
            return None;
        }
        n = (n * if hex { 0x10 } else { 0xA }) + x as u64;
        v += 1;
    }
    Some(n)
}
