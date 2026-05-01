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
#![feature(allocator_api, unchecked_neg, unchecked_shifts)]

extern crate core;

use core::fmt::{self, Debug, Display, Formatter};
use core::hint::unreachable_unchecked;
use core::iter::{FusedIterator, Iterator};
use core::matches;
use core::mem::{replace, transmute};
use core::ops::FnMut;
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Err, Ok};
use core::slice::{from_raw_parts, Iter};

#[cfg(feature = "alloc")]
mod numbers;
mod parse;

#[cfg(feature = "alloc")]
pub use self::numbers::*;
pub use self::parse::*;

pub const SPACE: &str = &" ";
pub const SPLITTER: &str = &";";
pub const REPLACEMENT: &str = "\u{FFFD}";

pub enum CharSize {
    One(u8),
    Two(u8, u8),
    Three(u8, u8, u8),
    Four(u8, u8, u8, u8),
}

pub struct U8Splitter<'a> {
    v:   &'a [u8],
    pos: usize,
}
pub struct U16DecodeError(pub u16);
pub struct U16Encoder<'a, I: Iterator<Item = &'a u8>> {
    v:   u16,
    buf: U8Decoder<'a, I>,
}
pub struct U8Decoder<'a, I: Iterator<Item = &'a u8>>(I);
pub struct U16Decoder<'a, I: Iterator<Item = &'a u16>> {
    r:   Option<u16>,
    buf: I,
}
pub struct U16ReplaceDecoder<'a, I: Iterator<Item = &'a u16>>(U16Decoder<'a, I>);

impl CharSize {
    #[inline]
    pub fn new(c: char) -> CharSize {
        CharSize::new_u32(c as u32)
    }
    #[inline]
    pub fn new_u32(v: u32) -> CharSize {
        match v as u32 {
            ..0x80 => CharSize::One(v as u8),
            ..0x800 => CharSize::Two(
                unsafe { v.unchecked_shl(6) & 0x3F } as u8 | 0xC0,
                (v & 0x3F) as u8 | 0x80,
            ),
            ..0x10000 => CharSize::Three(
                unsafe { v.unchecked_shr(12) & 0x3F } as u8 | 0xE0,
                unsafe { v.unchecked_shr(6) & 0x3F } as u8 | 0x80,
                (v & 0x3F) as u8 | 0x80,
            ),
            _ => CharSize::Four(
                unsafe { v.unchecked_shr(18) & 0x3F } as u8 | 0xF0,
                unsafe { v.unchecked_shr(12) & 0x3F } as u8 | 0x80,
                unsafe { v.unchecked_shr(6) & 0x3F } as u8 | 0x80,
                (v & 0x3F) as u8 | 0x80,
            ),
        }
    }

    #[inline]
    pub fn as_str<'a>(&'a self) -> &'a str {
        unsafe { transmute(self.as_slice()) }
    }
    #[inline]
    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        match self {
            CharSize::One(v) if *v == 0 => &[],
            CharSize::One(v) => unsafe { from_raw_parts(v as *const u8, 1) },
            CharSize::Two(v, _) => unsafe { from_raw_parts(v as *const u8, 2) },
            CharSize::Three(v, ..) => unsafe { from_raw_parts(v as *const u8, 3) },
            CharSize::Four(v, ..) => unsafe { from_raw_parts(v as *const u8, 4) },
        }
    }
    #[inline]
    pub fn write_hex(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CharSize::One(a) => write_hex(*a, f),
            CharSize::Two(a, b) => {
                write_hex(*a, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*b, f)
            },
            CharSize::Three(a, b, c) => {
                write_hex(*a, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*b, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*c, f)
            },
            CharSize::Four(a, b, c, d) => {
                write_hex(*a, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*b, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*c, f)?;
                f.write_str(SPLITTER)?;
                write_hex(*d, f)
            },
        }
    }
}
impl<'a> U8Splitter<'a> {
    #[inline]
    pub fn new(v: &'a [u8]) -> U8Splitter<'a> {
        U8Splitter { v, pos: 0usize }
    }
}
impl<'a> U8Decoder<'a, Iter<'a, u8>> {
    #[inline]
    pub fn new(v: &'a [u8]) -> U8Decoder<'a, Iter<'a, u8>> {
        U8Decoder(v.iter())
    }
}
impl<'a> U16Encoder<'a, Iter<'a, u8>> {
    #[inline]
    pub fn new(v: &'a [u8]) -> U16Encoder<'a, Iter<'a, u8>> {
        U16Encoder {
            v:   0u16,
            buf: U8Decoder::new(v),
        }
    }
}
impl<'a> U16Decoder<'a, Iter<'a, u16>> {
    #[inline]
    pub fn new(v: &'a [u16]) -> U16Decoder<'a, Iter<'a, u16>> {
        U16Decoder { r: None, buf: v.iter() }
    }
    #[inline]
    pub fn new_replacer(v: &'a [u16]) -> U16ReplaceDecoder<'a, Iter<'a, u16>> {
        U16ReplaceDecoder(U16Decoder::new(v))
    }
}
impl<'a, I: Iterator<Item = &'a u8>> U8Decoder<'a, I> {
    #[inline]
    pub fn new_iter(v: I) -> U8Decoder<'a, I> {
        U8Decoder(v)
    }
}
impl<'a, I: Iterator<Item = &'a u8>> U16Encoder<'a, I> {
    #[inline]
    pub fn new_iter(v: I) -> U16Encoder<'a, I> {
        U16Encoder {
            v:   0u16,
            buf: U8Decoder::new_iter(v),
        }
    }
}
impl<'a, I: Iterator<Item = &'a u16>> U16Decoder<'a, I> {
    #[inline]
    pub fn new_iter(i: I) -> U16Decoder<'a, I> {
        U16Decoder { r: None, buf: i }
    }
    #[inline]
    pub fn new_replacer_iter(i: I) -> U16ReplaceDecoder<'a, I> {
        U16ReplaceDecoder(U16Decoder::new_iter(i))
    }
}

impl Debug for CharSize {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}
impl Display for CharSize {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> Iterator for U8Splitter<'a> {
    type Item = (&'a str, &'a [u8]);

    #[inline]
    fn next(&mut self) -> Option<(&'a str, &'a [u8])> {
        if self.pos >= self.v.len() {
            return None;
        }
        let (s, i, p) = utf8_next_valid_pair(self.v, self.pos);
        self.pos = p;
        //
        // str  = s..i
        // [u8] = i..p
        //
        Some(unsafe {
            (
                transmute(from_raw_parts(self.v.as_ptr().add(s), i - s)),
                from_raw_parts(self.v.as_ptr().add(i), p - i),
            )
        })
    }
}
impl<'a> FusedIterator for U8Splitter<'a> {}

impl<'a, I: Iterator<Item = &'a u8>> Iterator for U8Decoder<'a, I> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let v = *self.0.next()?;
        if v < 0x80 {
            return Some(v as u32);
        }
        let i = (v & 0x1F) as u32;
        let n = unsafe { *self.0.next().unwrap_unchecked() } as u32;
        let u = unsafe { i.unchecked_shl(6) | (n & 0x3F) } as u32;
        if v < 0xE0 {
            return Some(u);
        }
        let y = unsafe { (n & 0x3F).unchecked_shl(6) | (*self.0.next().unwrap_unchecked() as u32 & 0x3F) } as u32;
        if v >= 0xF0 {
            Some(unsafe { (i & 0x7).unchecked_shr(18) | (y.unchecked_shl(6) | (*self.0.next().unwrap_unchecked() as u32 & 0x3F)) } as u32)
        } else {
            Some(unsafe { i.unchecked_shl(12) | y })
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (n, _) = self.0.size_hint();
        (n.div_ceil(4), Some(n))
    }
}
impl<'a, I: Iterator<Item = &'a u8>> FusedIterator for U8Decoder<'a, I> {}

impl<'a, I: Iterator<Item = &'a u8>> Iterator for U16Encoder<'a, I> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        if self.v != 0 {
            return Some(replace(&mut self.v, 0));
        }
        match self.buf.next() {
            Some(v) if (v & 0xFFFF) == v => Some(v as u16),
            Some(v) => {
                let i = v.saturating_sub(0x1_0000);
                self.v = (i & 0x3FF) as u16 | 0xDC00;
                Some(unsafe { i.unchecked_shr(10) } as u16 | 0xDC00)
            },
            None => None,
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (n, _) = self.buf.0.size_hint();
        if self.v == 0 {
            (n.div_ceil(3), Some(n))
        } else {
            (n.div_ceil(3) + 1, Some(n + 1))
        }
    }
}
impl<'a, I: Iterator<Item = &'a u8>> FusedIterator for U16Encoder<'a, I> {}

impl<'a, I: Iterator<Item = &'a u16>> Iterator for U16Decoder<'a, I> {
    type Item = Result<u32, U16DecodeError>;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a, b) = self.buf.size_hint();
        let (c, d) = self
            .r
            .map(|v| match v {
                0xD800..=0xDFFF => (0, 1),
                _ => (1, 1),
            })
            .unwrap_or((0, 0));
        (a.div_ceil(2) + c, b.and_then(|v| v.checked_add(d)))
    }
    fn next(&mut self) -> Option<Result<u32, U16DecodeError>> {
        let n = match self.r.take() {
            Some(v) => v,
            None => *self.buf.next()?,
        };
        if !matches!(n, 0xD800..=0xDFFF) {
            Some(Ok(n as u32))
        } else if n >= 0xDC00 {
            Some(Err(U16DecodeError(n)))
        } else {
            let v = match self.buf.next() {
                None => return Some(Err(U16DecodeError(n))),
                Some(v) => *v,
            };
            if v < 0xDC00 || v > 0xDFFF {
                self.r = Some(v);
                Some(Err(U16DecodeError(n)))
            } else {
                Some(Ok(unsafe {
                    (((n & 0x3FF) as u32).unchecked_shl(10) | ((v & 0x3FF) as u32)) + 0x10000
                }))
            }
        }
    }
}
impl<'a, I: Iterator<Item = &'a u16>> FusedIterator for U16Decoder<'a, I> {}

impl<'a, I: Iterator<Item = &'a u16>> Iterator for U16ReplaceDecoder<'a, I> {
    type Item = u32;

    #[inline]
    fn next(&mut self) -> Option<u32> {
        self.0.next().map(|r| r.unwrap_or(0xFFFD))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (n, _) = self.0.buf.size_hint();
        (n.div_ceil(4), Some(n))
    }
}
impl<'a, I: Iterator<Item = &'a u16>> FusedIterator for U16ReplaceDecoder<'a, I> {}

pub fn utf8_match(a: &[u8], b: &[u8], case: bool) -> bool {
    if a.is_empty() && b.is_empty() {
        return true;
    }
    // Match without NULL ends.
    // Check for NULL end.
    let (e, f) = (
        a.last().map_or(false, |v| *v == 0),
        b.last().map_or(false, |v| *v == 0),
    );
    let n = match (e, f) {
        (true, true) if a.len() == b.len() => a.len(),
        (false, false) if a.len() == b.len() => a.len(),
        (true, false) if a.len().saturating_sub(1) == b.len() => b.len(),
        (false, true) if a.len() == b.len().saturating_sub(1) => a.len(),
        _ => return false,
    };
    for i in 0..n {
        // Bounds already checked above.
        let (x, y) = unsafe { (*a.get_unchecked(i), *b.get_unchecked(i)) };
        match (case, x, y) {
            (false, 0x41..=0x5A, 0x61..=0x7A) if x + 0x20 == y => (),
            (false, 0x61..=0x7A, 0x41..=0x5A) if x == y + 0x20 => (),
            _ if x == y => (),
            _ => return false,
        }
    }
    true
}
pub fn utf16_match(a: &[u16], b: &[u16], case: bool) -> bool {
    if a.is_empty() && b.is_empty() {
        return true;
    }
    // Match without NULL ends.
    // Check for NULL end.
    let (e, f) = (
        a.last().map_or(false, |v| *v == 0),
        b.last().map_or(false, |v| *v == 0),
    );
    let n = match (e, f) {
        (true, true) if a.len() == b.len() => a.len(),
        (false, false) if a.len() == b.len() => a.len(),
        (true, false) if a.len().saturating_sub(1) == b.len() => b.len(),
        (false, true) if a.len() == b.len().saturating_sub(1) => a.len(),
        _ => return false,
    };
    for i in 0..n {
        // Bounds already checked above.
        let (x, y) = unsafe { (*a.get_unchecked(i), *b.get_unchecked(i)) };
        match (case, x, y) {
            (false, 0x41..=0x5A, 0x61..=0x7A) if x + 0x20 == y => (),
            (false, 0x61..=0x7A, 0x41..=0x5A) if x == y + 0x20 => (),
            _ if x == y => (),
            _ => return false,
        }
    }
    true
}

#[inline(always)]
pub fn hex(v: u8) -> u8 {
    match v & 0xF {
        0x0..=0x9 => (v & 0xF) + 0x30,
        0xA..=0xF => (v & 0xF) + 0x37,
        _ => unsafe { unreachable_unchecked() }, // Not possible to reach in bounds
    }
}
#[inline]
pub fn write_hex(v: u8, f: &mut Formatter<'_>) -> fmt::Result {
    let mut b = [0x30u8, 0x30u8];
    match v {
        ..=0xF => unsafe { *b.get_unchecked_mut(1) = hex(v) },
        _ => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4));
            *b.get_unchecked_mut(1) = hex(v);
        },
    }
    f.write_str(unsafe { transmute(b.as_slice()) })
}
pub fn write_hex_u32(v: u32, f: &mut Formatter<'_>) -> fmt::Result {
    let mut b = [0x30u8, 0x30u8];
    // Unsafe is used here as the compiler might not 100% know what we're doing.
    // All the access is in bounds always as it's manual.
    //
    // None of the shifts can overflow as the values are checked and are always
    // u32's
    match v {
        0 => f.write_str(unsafe { transmute(b.get_unchecked(0..1)) }),
        ..=0xF => unsafe {
            *b.get_unchecked_mut(0) = hex(v as u8);
            f.write_str(transmute(b.get_unchecked(0..1)))
        },
        ..=0xFF => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(transmute(b.as_slice()))
        },
        ..=0xFFFF => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(12) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(8) as u8);
            f.write_str(transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(transmute(b.as_slice()))
        },
        _ => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(28) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(24) as u8);
            f.write_str(transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(20) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(16) as u8);
            f.write_str(transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(12) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(8) as u8);
            f.write_str(transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(transmute(b.as_slice()))
        },
    }
}

#[inline]
pub fn utf8_to_utf16(b: &mut [u16], v: &[u8]) -> usize {
    if v.len() > 0 {
        str_iter_to_utf16(b, v.iter())
    } else {
        0
    }
}
pub fn utf8_to_lossy_func(v: &[u8], mut f: impl FnMut(&str)) {
    if v.is_empty() {
        return;
    }
    let mut i = U8Splitter::new(v);
    match i.next() {
        Some((c, u)) => {
            f(c);
            if u.is_empty() {
                return;
            }
        },
        None => return,
    };
    f(REPLACEMENT);
    for (c, u) in i {
        f(c);
        if !u.is_empty() {
            f(REPLACEMENT);
        }
    }
}
#[inline]
pub fn utf8_debug(v: &[u8], f: &mut Formatter<'_>) -> fmt::Result {
    if !v.is_empty() {
        for (i, c) in v.iter().enumerate() {
            if i > 0 {
                f.write_str(SPACE)?;
            }
            write_hex(*c, f)?;
        }
    }
    Ok(())
}
#[inline]
pub fn utf8_display(v: &[u8], f: &mut Formatter<'_>) -> fmt::Result {
    if !v.is_empty() {
        utf8_to_lossy_func(v, |v| {
            let _ = f.write_str(v);
        });
    }
    Ok(())
}
pub fn utf8_next_valid_pair(b: &[u8], pos: usize) -> (usize, usize, usize) {
    // 's' - strarting pos
    // 'i' - "v" or last valid index
    // 'p' - pos after
    let (s, mut i, mut p) = (pos, pos, pos);
    while p < b.len() {
        // Bounds already checked.
        let v = *unsafe { b.get_unchecked(p) };
        p += 1;
        match v {
            0..=0x7F => (),
            0xC2..=0xDF if next(&mut p, b) => break,
            // ^ This increases 'p' if false
            0xC2..=0xDF => break,
            0xE0..=0xEF => {
                match (v, b.get(p).copied().unwrap_or(0)) {
                    (0xE0, 0xA0..=0xBF) | (0xED, 0x80..=0x9F) => (),
                    (0xE1..=0xEC, 0x80..=0xBF) | (0xEE..=0xEF, 0x80..=0xBF) => (),
                    _ => break,
                }
                p += 1;
                if next(&mut p, b) {
                    break;
                }
                // ^ 'p' has already been increased if the above
                // fails.
            },
            0xF0..=0xF4 => {
                match (v, b.get(p).copied().unwrap_or(0)) {
                    (0xF0, 0x90..=0xBF) | (0xF4, 0x80..=0x8F) => (),
                    (0xF1..=0xF3, 0x80..=0xBF) => (),
                    _ => break,
                }
                p += 1;
                if next(&mut p, b) || next(&mut p, b) {
                    break;
                }
                // ^ 'p' has already been increased twice if the above
                // fails.
            },
            _ => break,
        }
        i = p
    }
    (s, i, p)
}

#[inline]
pub fn str_to_utf16(b: &mut [u16], v: &str) -> usize {
    if v.len() > 0 {
        str_iter_to_utf16(b, v.as_bytes().iter())
    } else {
        0
    }
}
pub fn str_iter_to_utf16<'a>(b: &mut [u16], v: impl Iterator<Item = &'a u8>) -> usize {
    let mut n = 0usize;
    for i in U16Encoder::new_iter(v) {
        if n >= b.len() {
            break;
        }
        // Bounds checked above.
        unsafe { *b.get_unchecked_mut(n) = i };
        n += 1;
    }
    n
}

#[inline]
pub fn utf16_to_buf(b: &mut [u8], v: &[u16]) -> usize {
    if v.len() > 0 {
        utf16_iter_to_buf(b, v.iter())
    } else {
        0
    }
}
#[inline]
pub fn utf16_to_func(v: &[u16], f: impl FnMut(CharSize)) {
    if v.len() > 0 {
        utf16_iter_to_func(v.iter(), f)
    }
}
#[inline]
pub fn utf16_to_str<'a>(b: &mut [u8], v: &'a [u16]) -> &'a str {
    utf16_iter_to_str(b, v.iter())
}
#[inline]
pub fn utf16_debug(v: &[u16], f: &mut Formatter<'_>) -> fmt::Result {
    if !v.is_empty() {
        for (i, c) in U16Decoder::new_replacer(v).map(CharSize::new_u32).enumerate() {
            if i > 0 {
                f.write_str(SPACE)?;
            }
            c.write_hex(f)?;
        }
    }
    Ok(())
}
#[inline]
pub fn utf16_display(v: &[u16], f: &mut Formatter<'_>) -> fmt::Result {
    if !v.is_empty() {
        utf16_to_func(v, |c| {
            let _ = f.write_str(c.as_str());
        })
    }
    Ok(())
}

pub fn utf16_iter_to_buf<'a>(b: &mut [u8], v: impl Iterator<Item = &'a u16>) -> usize {
    let mut n = 0usize;
    for i in U16Decoder::new_replacer_iter(v).map(CharSize::new_u32) {
        // All bounds are checked below.
        n += match i {
            CharSize::One(v) if v == 0 => break,
            CharSize::One(_) if n + 1 >= b.len() => break,
            CharSize::Two(..) if n + 2 >= b.len() => break,
            CharSize::Three(..) if n + 3 >= b.len() => break,
            CharSize::Four(..) if n + 4 >= b.len() => break,
            CharSize::One(v) if v == 0 => break,
            CharSize::One(v) => {
                // Already enforced by the above line check.
                unsafe { *b.get_unchecked_mut(n) = v };
                1
            },
            CharSize::Two(v, x) => {
                unsafe {
                    *b.get_unchecked_mut(n) = v;
                    *b.get_unchecked_mut(n + 1) = x;
                }
                2
            },
            CharSize::Three(v, x, y) => {
                unsafe {
                    *b.get_unchecked_mut(n) = v;
                    *b.get_unchecked_mut(n + 1) = x;
                    *b.get_unchecked_mut(n + 2) = y;
                }
                3
            },
            CharSize::Four(v, x, y, z) => {
                unsafe {
                    *b.get_unchecked_mut(n) = v;
                    *b.get_unchecked_mut(n + 1) = x;
                    *b.get_unchecked_mut(n + 2) = y;
                    *b.get_unchecked_mut(n + 3) = z;
                }
                4
            },
        };
    }
    n
}
#[inline]
pub fn utf16_iter_to_str<'a>(b: &mut [u8], v: impl Iterator<Item = &'a u16>) -> &'a str {
    let r = utf16_iter_to_buf(b, v);
    unsafe { transmute(b.get_unchecked(0..r)) }
}
#[inline]
pub fn utf16_iter_to_func<'a>(v: impl Iterator<Item = &'a u16>, mut f: impl FnMut(CharSize)) {
    for i in U16Decoder::new_replacer_iter(v).map(CharSize::new_u32) {
        f(i)
    }
}

#[inline]
pub unsafe fn utf8_to_utf16_unchecked(b: &mut [u16], v: &str) {
    for (i, e) in v.as_bytes().iter().enumerate() {
        if i >= b.len() || *e == 0 {
            break;
        }
        // 'i' is always in bounds of 'b' due to the above check.
        unsafe { *b.get_unchecked_mut(i) = *e as u16 };
    }
}

#[inline]
fn next(i: &mut usize, b: &[u8]) -> bool {
    match b.get(*i) {
        None => true,
        Some(v) if *v & 0xC0 != 0x80 => true,
        Some(_) => {
            *i += 1;
            false
        },
    }
}
