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
use core::convert::{From, Into};
use core::marker::PhantomData;

use xrmt_data::text::utf16_to_buf;
use xrmt_data::Blob;

use crate::char_like;
use crate::structs::{StringLike, StringLikeU8};

pub enum CharLike<'a> {
    Null,
    Owned(Char),
    Slice(CharSlice<'a>),
    Pointer(CharPtr<'a>),
}

/// Should have an ending NULL
#[repr(transparent)]
pub struct CharPtr<'a> {
    ptr: *const u8,
    _p:  PhantomData<&'a [u8]>,
}
/// Guaranteed to have an ending NULL
pub struct Char(Vec<u8>);
/// Might not have an ending NULL
#[repr(transparent)]
pub struct CharSlice<'a>(&'a [u8]);

pub type Chars<A = Global> = Blob<u8, 128, A>;

impl CharPtr<'_> {
    #[inline]
    pub fn str(v: *const u8) -> String {
        String::from_utf8_lossy(&CharPtr::new(v)).into()
    }
}

impl From<&[u16]> for Char {
    #[inline]
    fn from(v: &[u16]) -> Char {
        let mut x = Char::null();
        let _ = utf16_to_buf(&mut x.0, v);
        x.0.push(0); // Add NULL
        x
    }
}
impl From<Vec<u16>> for Char {
    #[inline]
    fn from(v: Vec<u16>) -> Char {
        Char::from(v.as_slice())
    }
}
impl<'a> From<&[u16]> for CharLike<'a> {
    #[inline]
    fn from(v: &[u16]) -> CharLike<'a> {
        CharLike::Owned(Char::from(v))
    }
}
impl<'a> From<Vec<u16>> for CharLike<'a> {
    #[inline]
    fn from(v: Vec<u16>) -> CharLike<'a> {
        CharLike::Owned(Char::from(v.as_slice()))
    }
}

char_like!(u8, Char, CharPtr, CharSlice, CharLike, StringLikeU8);
