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

use alloc::alloc::Global;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::From;
use core::marker::PhantomData;

use xrmt_data::text::{utf16_to_string, utf8_to_lossy_u16};
use xrmt_data::Blob;

use crate::char_like;
use crate::structs::StringLikeU16;

pub enum WCharLike<'a> {
    Null,
    Owned(WChar),
    Slice(WCharSlice<'a>),
    Pointer(WCharPtr<'a>),
}

/// Should have an ending NULL
#[repr(transparent)]
pub struct WCharPtr<'a> {
    ptr: *const u16,
    _p:  PhantomData<&'a [u16]>,
}
/// Guaranteed to have an ending NULL
pub struct WChar(Vec<u16>);
/// Might not have an ending NULL
#[repr(transparent)]
pub struct WCharSlice<'a>(&'a [u16]);

pub type WChars<A = Global> = Blob<u16, 128, A>;

impl WChar {
    #[inline]
    pub unsafe fn from_utf8_unchecked(v: &[u8]) -> WChar {
        let mut b = WChar(Vec::with_capacity(v.len()));
        for i in v {
            b.0.push(*i as u16);
        }
        b.0.push(0); // Add NULL
        b
    }
}
impl WCharPtr<'_> {
    #[inline]
    pub fn str(v: *const u16) -> String {
        utf16_to_string(&WCharPtr::new(v))
    }
}

impl From<&[u8]> for WChar {
    #[inline]
    fn from(v: &[u8]) -> WChar {
        let mut x = WChar::null();
        let _ = utf8_to_lossy_u16(&mut x.0, v);
        x.0.push(0); // Add NULL
        x
    }
}
impl From<Vec<u8>> for WChar {
    #[inline]
    fn from(v: Vec<u8>) -> WChar {
        WChar::from(v.as_slice())
    }
}
impl<'a> From<&[u8]> for WCharLike<'a> {
    #[inline]
    fn from(v: &[u8]) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v))
    }
}
impl<'a> From<Vec<u8>> for WCharLike<'a> {
    #[inline]
    fn from(v: Vec<u8>) -> WCharLike<'a> {
        WCharLike::Owned(WChar::from(v.as_slice()))
    }
}

char_like!(u16, WChar, WCharPtr, WCharSlice, WCharLike, StringLikeU16);
