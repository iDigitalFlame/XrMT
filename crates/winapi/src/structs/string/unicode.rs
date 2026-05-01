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

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::PartialEq;
use core::convert::{From, Into};
use core::default::Default;
use core::mem::drop;
use core::ops::Deref;
use core::ptr::{null, null_mut};
use core::slice::from_raw_parts;

use xrmt_data::Fiber;

use crate::structs::{StringLike, StringLikeU16, WChar, WCharLike, WCharPtr};

#[repr(C)]
pub struct UnicodeString<'a> {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     WCharPtr<'a>,
}

impl<'a> UnicodeString<'a> {
    #[inline]
    pub const fn empty() -> UnicodeString<'a> {
        UnicodeString {
            buffer:     WCharPtr::null(),
            length:     0u16,
            max_length: 0u16,
        }
    }

    #[inline]
    pub fn new_wchar(v: &'a WChar) -> UnicodeString<'a> {
        if v.is_null() {
            UnicodeString::empty()
        } else {
            let n = 2 * v.len_without_null() as u16;
            UnicodeString {
                buffer:     v.as_char_ptr(),
                length:     n,
                max_length: n,
            }
        }
    }
    #[inline]
    pub fn new(v: &'a WCharLike<'a>) -> UnicodeString<'a> {
        if v.is_null() {
            UnicodeString::empty()
        } else {
            let n = 2 * v.len_without_null() as u16;
            UnicodeString {
                buffer:     unsafe { v.as_char_ptr() },
                length:     n,
                max_length: n,
            }
        }
    }

    #[inline]
    pub unsafe fn leaked(v: impl Into<WChar>) -> UnicodeString<'a> {
        let mut n = UnicodeString::empty();
        unsafe { n.set_and_leak(v) };
        n
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.buffer.is_null()
    }
    #[inline]
    pub fn hash_fnv32(&self) -> u32 {
        let mut h = 0x811C9DC5u32;
        for i in self.as_slice() {
            if *i == 0 {
                break;
            }
            h = h.wrapping_mul(0x1000193);
            h ^= match *i as u8 {
                b'A'..=b'Z' => *i + 0x20,
                _ => *i,
            } as u32;
        }
        h
    }
    #[inline]
    pub fn hash_x65599(&self) -> u32 {
        let mut h = 0u32;
        for i in self.as_slice() {
            if *i == 0 {
                break;
            }
            h = h.wrapping_mul(0x1003F).saturating_add(match *i as u8 {
                b'A'..=b'Z' => *i + 0x20,
                _ => *i,
            } as u32);
        }
        h
    }

    #[inline]
    pub unsafe fn free(&mut self) {
        let mut b = unsafe {
            Vec::from_raw_parts(
                self.buffer.as_mut_ptr(),
                self.length as usize,
                self.max_length as usize,
            )
        };
        b.clear();
        drop(b);
    }
    #[inline]
    pub unsafe fn set_and_leak(&mut self, v: impl Into<WChar>) {
        let p = v.into();
        let i = 2 * p.len_without_null() as u16;
        (self.length, self.max_length) = (i, i);
        self.buffer = WCharPtr::new(unsafe { p.leak() });
    }
}

impl<'a> StringLikeU16 for UnicodeString<'a> {
    #[inline]
    fn len_without_null(&self) -> usize {
        self.len()
    }
}
impl<'a> StringLike<u16> for UnicodeString<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.length as usize / 2
    }
    #[inline]
    fn is_null(&self) -> bool {
        self.buffer.is_null()
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.buffer.is_null() || self.length == 0
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        if self.is_empty() {
            &[]
        } else {
            unsafe { from_raw_parts(self.buffer.as_ptr(), (self.length / 2) as usize) }
        }
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        if self.is_empty() {
            null()
        } else {
            self.buffer.as_ptr()
        }
    }
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u16 {
        if self.is_empty() {
            null_mut()
        } else {
            self.buffer.as_mut_ptr()
        }
    }
}

impl Deref for UnicodeString<'_> {
    type Target = [u16];

    #[inline]
    fn deref(&self) -> &[u16] {
        self.as_slice()
    }
}
impl<'a> Default for UnicodeString<'a> {
    #[inline]
    fn default() -> UnicodeString<'a> {
        UnicodeString::empty()
    }
}
impl PartialEq<[u16]> for UnicodeString<'_> {
    #[inline]
    fn eq(&self, other: &[u16]) -> bool {
        self.as_slice().eq(other)
    }
}
impl<'a> From<&'a WCharLike<'a>> for UnicodeString<'a> {
    #[inline]
    fn from(v: &'a WCharLike<'a>) -> UnicodeString<'a> {
        UnicodeString::new(v)
    }
}

impl From<UnicodeString<'_>> for Fiber {
    #[inline]
    fn from(v: UnicodeString<'_>) -> Fiber {
        v.into_fiber()
    }
}
impl From<&UnicodeString<'_>> for Fiber {
    #[inline]
    fn from(v: &UnicodeString<'_>) -> Fiber {
        v.to_fiber()
    }
}

impl From<UnicodeString<'_>> for String {
    #[inline]
    fn from(v: UnicodeString<'_>) -> String {
        v.into_string()
    }
}
impl From<&UnicodeString<'_>> for String {
    #[inline]
    fn from(v: &UnicodeString<'_>) -> String {
        v.to_string()
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, Result};

    use crate::structs::{StringLikeU16, UnicodeString};

    impl Debug for UnicodeString<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_debug(f)
        }
    }
    impl Display for UnicodeString<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_display(f)
        }
    }
}
