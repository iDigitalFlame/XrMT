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

extern crate alloc;
extern crate core;

extern crate xrmt_text;

use alloc::alloc::Allocator;
use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::iter::Iterator;
use core::mem::transmute;
use core::option::Option::{None, Some};

use crate::fiber::Fiber;
use crate::VecLike;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use xrmt_text::*;

#[inline]
pub fn str_to_utf16_vec(b: &mut impl VecLike<u16>, v: &str) -> usize {
    let t = v.as_bytes();
    if t.len() > 0 {
        b.reserve((t.len() + 3).saturating_div(4));
        str_iter_to_utf16_vec(b, t.iter())
    } else {
        0
    }
}
pub fn str_iter_to_utf16_vec<'a>(b: &mut impl VecLike<u16>, v: impl Iterator<Item = &'a u8>) -> usize {
    let mut n = 0;
    for i in U16Encoder::new_iter(v) {
        if n >= b.capacity() {
            b.reserve(n + 16);
        }
        b.push(i);
        n += 1;
    }
    b.shrink_to_fit();
    n
}

#[inline]
pub fn utf8_to_lossy_owned(v: Vec<u8>) -> String {
    if v.is_empty() {
        return unsafe { String::from_utf8_unchecked(v) };
    }
    match utf8_to_lossy(&v) {
        Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(v) },
        Cow::Owned(s) => s,
    }
}
pub fn utf8_to_lossy<'a>(v: &'a [u8]) -> Cow<'a, str> {
    let mut i = U8Splitter::new(v);
    let n = match i.next() {
        Some((c, u)) if u.is_empty() => return Cow::Borrowed(c),
        Some((c, _)) => c,
        None => return Cow::Borrowed(""),
    };
    let mut s = String::with_capacity(v.len());
    let b = unsafe { s.as_mut_vec() };
    b.extend_from_slice(n.as_bytes());
    b.push(0xFF);
    b.push(0xFD); // Replacement Char.
    for (c, u) in i {
        b.extend_from_slice(c.as_bytes()); // No need to check UTF8 since we did above
        if !u.is_empty() {
            b.push(0xFF);
            b.push(0xFD); // Replacement Char.
        }
    }
    b.shrink_to_fit();
    Cow::Owned(s)
}
pub fn utf8_to_lossy_u16(b: &mut impl VecLike<u16>, v: &[u8]) -> usize {
    if v.is_empty() {
        return 0;
    }
    let mut n = b.len();
    utf8_to_lossy_func(v, |s| n += str_to_utf16_vec(b, s));
    b.truncate(n);
    b.shrink_to_fit();
    n
}
pub fn utf8_to_lossy_rewrite<A: Allocator>(b: &mut Vec<u8, A>) -> bool {
    let (mut z, mut u) = (0usize, false);
    while z < b.len() {
        let (_, i, p) = utf8_next_valid_pair(b, z);
        if p == i {
            return u;
        }
        let r = p.saturating_sub(i);
        // 'i' will always be in bounds.
        unsafe { (*b.get_unchecked_mut(i), z, u) = (0xFF, p, true) };
        if r == 1 {
            b.insert(i + 1, 0xFD);
            z += 1;
        } else {
            // i + 1 must be at-least 1 in bounds.
            unsafe { *b.get_unchecked_mut(i + 1) = 0xFD };
            if r > 2 {
                // Copy then scroll-back the next values.
                b.copy_within(z.., i + 2);
                b.truncate(b.len() - (r - 2));
                z = z - (r - 2)
            }
        }
    }
    u
}
#[inline]
pub fn utf8_to_lossy_insert(b: &mut impl VecLike<u8>, v: &[u8]) {
    utf8_to_lossy_func(v, |s| b.extend_from_slice(s.as_bytes()));
    b.shrink_to_fit();
}

#[inline]
pub fn utf16_to_fiber(v: &[u16]) -> Fiber {
    let mut b = Fiber::with_capacity(v.len());
    if v.len() > 0 {
        utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v.iter());
    }
    b
}
#[inline]
pub fn utf16_to_string(v: &[u16]) -> String {
    let mut b = String::with_capacity(v.len());
    if v.len() > 0 {
        utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v.iter());
    }
    b
}
#[inline]
pub fn utf16_to_str<'a>(b: &'a mut [u8], v: &'a [u16]) -> &'a str {
    utf16_iter_to_str(b, v.iter())
}
#[inline]
pub fn utf16_to_fiber_in<A: Allocator>(v: &[u16], alloc: A) -> Fiber<A> {
    let mut b = Fiber::with_capacity_in(v.len(), alloc);
    if v.len() > 0 {
        utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v.iter());
    }
    b
}
#[inline]
pub fn utf16_to_vec<'a>(b: &'a mut impl VecLike<u8>, v: &'a [u16]) -> &'a str {
    b.reserve(v.len());
    utf16_iter_to_vec(b, v.iter())
}

#[inline]
pub fn utf16_iter_to_fiber<'a>(v: impl Iterator<Item = &'a u16>) -> Fiber {
    let mut b = Fiber::new();
    utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v);
    b
}
#[inline]
pub fn utf16_iter_to_string<'a>(v: impl Iterator<Item = &'a u16>) -> String {
    let mut b = String::new();
    utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v);
    b
}
pub fn utf16_iter_to_vec<'a>(b: &mut impl VecLike<u8>, v: impl Iterator<Item = &'a u16>) -> &'a str {
    let (mut n, o) = (0usize, b.len());
    for i in U16Decoder::new_replacer_iter(v).map(CharSize::new_u32) {
        n += match i {
            CharSize::One(v) if v == 0 => break,
            v => {
                let k = v.as_slice();
                b.reserve(k.len());
                b.extend_from_slice(k);
                k.len()
            },
        };
    }
    b.truncate(o + n);
    b.shrink_to_fit();
    unsafe { transmute(b.get_unchecked(0..n)) }
}
#[inline]
pub fn utf16_iter_to_fiber_in<'a, A: Allocator>(v: impl Iterator<Item = &'a u16>, alloc: A) -> Fiber<A> {
    let mut b = Fiber::new_in(alloc);
    utf16_iter_to_vec(unsafe { b.as_mut_vec() }, v);
    b
}

#[inline]
pub unsafe fn str_to_u16_unchecked(b: &mut impl VecLike<u16>, v: &str) {
    b.reserve(v.len());
    for i in v.as_bytes() {
        if *i == 0 {
            break;
        }
        b.push(*i as u16);
    }
}
