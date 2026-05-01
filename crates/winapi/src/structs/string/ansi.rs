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

use crate::structs::{Char, CharLike, CharPtr, StringLike, StringLikeU8};

#[repr(C)]
pub struct AnsiString<'a> {
    pub length:     u16,
    pub max_length: u16,
    pub buffer:     CharPtr<'a>,
}

impl<'a> AnsiString<'a> {
    #[inline]
    pub const fn empty() -> AnsiString<'a> {
        AnsiString {
            buffer:     CharPtr::null(),
            length:     0u16,
            max_length: 0u16,
        }
    }

    #[inline]
    pub fn new(v: &'a CharLike<'a>) -> AnsiString<'a> {
        if v.is_null() {
            AnsiString::empty()
        } else {
            let n = v.len_without_null() as u16;
            AnsiString {
                buffer:     unsafe { v.as_char_ptr() },
                length:     n,
                max_length: n,
            }
        }
    }

    #[inline]
    pub unsafe fn leaked(v: impl Into<Char>) -> AnsiString<'a> {
        let mut n = AnsiString::empty();
        unsafe { n.set_and_leak(v) };
        n
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0 || self.buffer.is_null()
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
    pub unsafe fn set_and_leak(&mut self, v: impl Into<Char>) {
        let p = v.into();
        self.max_length = p.len() as u16;
        self.length = self.max_length
            - if self.max_length > 0 && p.is_null_padded() {
                1
            } else {
                0
            };
        self.buffer = CharPtr::new(unsafe { p.leak() });
    }
}

impl<'a> StringLikeU8 for AnsiString<'a> {
    #[inline]
    fn len_without_null(&self) -> usize {
        self.len()
    }
}
impl<'a> StringLike<u8> for AnsiString<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.length as usize
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
    fn as_slice(&self) -> &[u8] {
        if self.is_empty() {
            &[]
        } else {
            unsafe { from_raw_parts(self.buffer.as_ptr(), self.length as usize) }
        }
    }
    #[inline]
    fn as_ptr(&self) -> *const u8 {
        if self.is_empty() {
            null()
        } else {
            self.buffer.as_ptr()
        }
    }
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        if self.is_empty() {
            null_mut()
        } else {
            self.buffer.as_mut_ptr()
        }
    }
}

impl Deref for AnsiString<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl<'a> Default for AnsiString<'a> {
    #[inline]
    fn default() -> AnsiString<'a> {
        AnsiString::empty()
    }
}
impl PartialEq<[u8]> for AnsiString<'_> {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice().eq(other)
    }
}
impl<'a> From<&'a CharLike<'a>> for AnsiString<'a> {
    #[inline]
    fn from(v: &'a CharLike<'a>) -> AnsiString<'a> {
        AnsiString::new(v)
    }
}

impl From<AnsiString<'_>> for Fiber {
    #[inline]
    fn from(v: AnsiString<'_>) -> Fiber {
        v.into_fiber()
    }
}
impl From<&AnsiString<'_>> for Fiber {
    #[inline]
    fn from(v: &AnsiString<'_>) -> Fiber {
        v.to_fiber()
    }
}

impl From<AnsiString<'_>> for String {
    #[inline]
    fn from(v: AnsiString<'_>) -> String {
        v.into_string()
    }
}
impl From<&AnsiString<'_>> for String {
    #[inline]
    fn from(v: &AnsiString<'_>) -> String {
        v.to_string()
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    extern crate xrmt_data;

    use core::fmt::{Debug, Display, Formatter, Result};

    use crate::structs::{AnsiString, StringLikeU8};

    impl Debug for AnsiString<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_debug(f)
        }
    }
    impl Display for AnsiString<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_display(f)
        }
    }
}
