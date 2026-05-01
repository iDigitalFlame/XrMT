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

extern crate xrmt_data;

use alloc::vec::Vec;
use core::iter::Iterator;
use core::marker::Copy;
use core::matches;
use core::option::Option::{None, Some};

use xrmt_data::text::utf16_to_vec;

use crate::structs::EnvironmentBlock;

pub trait PathByte: Copy {
    fn from_u8(v: u8) -> Self;
    fn from_u16(v: u16) -> Self;

    fn as_u8(self) -> u8;
    fn is(self, v: u8) -> bool;
}
pub trait Environment<T: PathByte> {
    fn var(&self, var: &[T], out: &mut Vec<T>) -> bool;
}

impl PathByte for u8 {
    #[inline]
    fn from_u8(v: u8) -> u8 {
        v
    }
    #[inline]
    fn from_u16(v: u16) -> u8 {
        v as u8
    }

    #[inline]
    fn as_u8(self) -> u8 {
        self
    }
    #[inline]
    fn is(self, v: u8) -> bool {
        self == v
    }
}
impl PathByte for u16 {
    #[inline]
    fn from_u8(v: u8) -> u16 {
        v as u16
    }
    #[inline]
    fn from_u16(v: u16) -> u16 {
        v
    }

    #[inline]
    fn as_u8(self) -> u8 {
        self as u8
    }
    #[inline]
    fn is(self, v: u8) -> bool {
        self as u8 == v
    }
}

impl Environment<u8> for EnvironmentBlock<'_> {
    fn var(&self, var: &[u8], out: &mut Vec<u8>) -> bool {
        match self.iter().find(|v| v.is_key_u8(var)).and_then(|v| v.value()) {
            Some(v) => {
                let _ = utf16_to_vec(out, &v);
            },
            None => return false,
        }
        true
    }
}
impl Environment<u8> for &EnvironmentBlock<'_> {
    #[inline]
    fn var(&self, var: &[u8], out: &mut Vec<u8>) -> bool {
        (**self).var(var, out)
    }
}
impl Environment<u16> for EnvironmentBlock<'_> {
    fn var(&self, var: &[u16], out: &mut Vec<u16>) -> bool {
        match self.iter().find(|v| v.is_key(var)).and_then(|v| v.value()) {
            Some(v) => out.extend_from_slice(&v),
            None => return false,
        }
        true
    }
}
impl Environment<u16> for &EnvironmentBlock<'_> {
    #[inline]
    fn var(&self, var: &[u16], out: &mut Vec<u16>) -> bool {
        (**self).var(var, out)
    }
}

#[inline]
pub fn expand<T: PathByte, E: Environment<T>>(src: Vec<T>, env: &E) -> Vec<T> {
    match src.iter().position(|v| matches!(v.as_u8(), b'$' | b'%')) {
        Some(_) => expand_slice(&src, env),
        None => src,
    }
}
pub fn expand_slice<T: PathByte, E: Environment<T>>(src: &[T], env: &E) -> Vec<T> {
    let mut b = Vec::with_capacity(src.len());
    let (mut i, mut c, mut n) = (0, 0, 0);
    while i < src.len() {
        // This will always be in bounds.
        let v = unsafe { src.get_unchecked(i) };
        match (v.as_u8(), c) {
            (0, _) => break,
            // Windows Style?
            //   BCE: If empty or bigger, clear it.
            (b'%', b'%') if n + 1 >= i => (n, c) = (0, 0),
            //   This can only be hit if it's '%' and '%' has to be set earlier.
            (b'%', b'%') if env.var(unsafe { src.get_unchecked(n + 1..i) }, &mut b) => (n, c) = (0, 0),
            // Unix Style?
            //   BCE: If empty or bigger, clear it.
            (b'}', b'$') if n + 2 >= i => (n, c) = (0, 0),
            //   Validate that the other end has a matching '{'.
            (b'}', b'$') if unsafe { src.get_unchecked(n + 1) }.as_u8() == b'{' && env.var(unsafe { src.get_unchecked(n + 2..i) }, &mut b) => (n, c) = (0, 0),
            //   If we have a Unix Style var, ignore the '{'.
            (b'{', b'$') => (),
            // Validation
            //   Ignore these items that are only valid on Windows.
            #[cfg(target_family = "windows")]
            (b'=' | b':' | b'(' | b')', b'%') => (),
            //   Ignore these valid name items.
            (b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_', 1..) => (),
            // New Var Starting?
            //   New var starting with '$' (Unix Style)
            (b'$', 0) => (n, c) = (i, b'$'),
            //   New var starting with '%' (Windows Style)
            (b'%', 0) => (n, c) = (i, b'%'),
            //   BCE: If empty or bigger, clear it.
            (_, b'$') if n + 1 >= i => (n, c) = (0, 0),
            //   End of Unix Style var? Check and fallback.
            //   If the 'var' check fails. we couldn't find it, so fall down.
            (_, b'$') if env.var(unsafe { src.get_unchecked(n + 1..i) }, &mut b) => {
                match v.as_u8() {
                    b'$' => (n, c) = (i, b'$'),
                    b'%' => (n, c) = (i, b'%'),
                    _ => {
                        b.push(*v); // Add char that triggered this.
                        (n, c) = (0, 0);
                    },
                }
            },
            //   BCE: If empty or bigger, clear it.
            (_, 1..) if n >= i => (n, c) = (0, 0),
            // Invalid or un-ended path value.
            // Just write it to the buf.
            (_, 1..) => {
                b.extend_from_slice(unsafe { src.get_unchecked(n..=i) });
                c = 0;
            },
            _ => b.push(*v),
        }
        i += 1;
    }
    if c > 0 && n > 0 && n < src.len() {
        b.extend_from_slice(unsafe { src.get_unchecked(n - 1..) });
    }
    b
}
