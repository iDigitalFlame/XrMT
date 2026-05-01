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
use core::alloc::Allocator;
use core::convert::From;
use core::fmt::{Formatter, Result};
use core::iter::Iterator;
use core::marker::Sized;
use core::option::Option::{None, Some};
use core::ptr::null_mut;

use xrmt_data::text::{utf16_debug, utf16_display, utf16_to_fiber, utf16_to_fiber_in, utf16_to_string, utf8_debug, utf8_display, utf8_to_lossy};
use xrmt_data::{Blob, Fiber, Slice, VecLike};

use crate::utils::copy;

mod ansi;
mod char;
mod env;
mod unicode;
mod wchar;

pub use self::ansi::*;
pub use self::char::*;
pub use self::env::*;
pub use self::unicode::*;
pub use self::wchar::*;

pub trait StringLike<T> {
    fn as_slice(&self) -> &[T];

    #[inline]
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    #[inline]
    fn is_null(&self) -> bool {
        self.is_empty()
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    fn as_ptr(&self) -> *const T {
        self.as_slice().as_ptr()
    }
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut T {
        null_mut()
    }
}
pub trait DecodeUtf16<T> {
    fn decode_utf16(&self) -> T;
}
pub trait StringWritable<T> {
    fn into_buf(&self, buf: &mut [T]) -> usize;
    fn into_vec(&self, buf: &mut impl VecLike<T>) -> usize;
}
pub trait StringLikeU8: StringLike<u8> + Sized {
    #[inline]
    fn to_u8(&self) -> Chars {
        self.to_u8_in(Global)
    }
    #[inline]
    fn to_fiber(&self) -> Fiber {
        self.to_fiber_in(Global)
    }
    #[inline]
    fn into_fiber(self) -> Fiber {
        self.into_fiber_in(Global)
    }
    #[inline]
    fn to_string(&self) -> String {
        utf8_to_lossy(self.as_slice()).into_owned()
    }
    #[inline]
    fn into_string(self) -> String {
        self.to_string()
    }
    #[inline]
    fn is_null_padded(&self) -> bool {
        self.as_slice().last().map_or(false, |v| *v == 0)
    }
    #[inline]
    fn len_without_null(&self) -> usize {
        if self.is_null_padded() {
            self.len().saturating_sub(1)
        } else {
            self.len()
        }
    }
    #[inline]
    fn to_slice(&self, b: &mut [u8]) -> usize {
        copy(self.as_slice(), b)
    }
    #[inline]
    fn as_char_slice<'a>(&'a self) -> CharSlice<'a> {
        CharSlice::from(self.as_slice())
    }
    #[inline]
    fn to_u8_slice<const N: usize>(&self) -> Slice<u8, N> {
        Slice::from_cstr(self.as_slice())
    }
    #[inline]
    fn to_u8_in<A: Allocator>(&self, alloc: A) -> Chars<A> {
        Blob::with_values_in(self.as_slice(), alloc)
    }
    #[inline]
    fn to_fiber_in<A: Allocator>(&self, alloc: A) -> Fiber<A> {
        let v = self.as_slice();
        if v.is_empty() {
            Fiber::new_in(alloc)
        } else {
            Fiber::from_utf8_lossy_in(v, alloc)
        }
    }
    #[inline]
    fn into_fiber_in<A: Allocator>(self, alloc: A) -> Fiber<A> {
        self.to_fiber_in(alloc)
    }

    #[doc(hidden)]
    #[inline]
    fn fmt_debug(&self, f: &mut Formatter<'_>) -> Result {
        utf8_debug(self.as_slice(), f)
    }
    #[doc(hidden)]
    #[inline]
    fn fmt_display(&self, f: &mut Formatter<'_>) -> Result {
        utf8_display(self.as_slice(), f)
    }
}
pub trait StringLikeU16: StringLike<u16> + Sized {
    #[inline]
    fn to_u8(&self) -> Chars {
        self.to_u8_in(Global)
    }
    #[inline]
    fn to_u16(&self) -> WChars {
        self.to_u16_in(Global)
    }
    #[inline]
    fn to_fiber(&self) -> Fiber {
        self.to_fiber_in(Global)
    }
    #[inline]
    fn into_fiber(self) -> Fiber {
        self.into_fiber_in(Global)
    }
    #[inline]
    fn to_string(&self) -> String {
        let v = self.as_slice();
        if v.is_empty() {
            String::new()
        } else {
            utf16_to_string(v)
        }
    }
    #[inline]
    fn into_string(self) -> String {
        self.to_string()
    }
    #[inline]
    fn is_null_padded(&self) -> bool {
        self.as_slice().last().map_or(false, |v| *v == 0)
    }
    #[inline]
    fn len_without_null(&self) -> usize {
        if self.is_null_padded() {
            self.len().saturating_sub(1)
        } else {
            self.len()
        }
    }
    #[inline]
    fn to_slice(&self, b: &mut [u16]) -> usize {
        copy(self.as_slice(), b)
    }
    #[inline]
    fn as_wchar_slice<'a>(&'a self) -> WCharSlice<'a> {
        WCharSlice::from(self.as_slice())
    }
    #[inline]
    fn to_u8_slice<const N: usize>(&self) -> Slice<u8, N> {
        Slice::from_utf16(self.as_slice())
    }
    #[inline]
    fn to_u8_in<A: Allocator>(&self, alloc: A) -> Chars<A> {
        Blob::from_utf16_in(self.as_slice(), alloc)
    }
    #[inline]
    fn to_u16_slice<const N: usize>(&self) -> Slice<u16, N> {
        let mut n = Slice::from(self.as_slice());
        if let Some(i) = n.iter().position(|v| *v == 0) {
            n.truncate(i);
        }
        n
    }
    #[inline]
    fn to_u16_in<A: Allocator>(&self, alloc: A) -> WChars<A> {
        let d = self.as_slice();
        match d.iter().position(|v| *v == 0) {
            Some(i) => Blob::with_values_in(unsafe { d.get_unchecked(0..i) }, alloc),
            None => Blob::with_values_in(d, alloc),
        }
    }
    #[inline]
    fn to_fiber_in<A: Allocator>(&self, alloc: A) -> Fiber<A> {
        let v = self.as_slice();
        if v.is_empty() {
            Fiber::new_in(alloc)
        } else {
            utf16_to_fiber_in(v, alloc)
        }
    }
    #[inline]
    fn into_fiber_in<A: Allocator>(self, alloc: A) -> Fiber<A> {
        self.to_fiber_in(alloc)
    }

    #[doc(hidden)]
    #[inline]
    fn fmt_debug(&self, f: &mut Formatter<'_>) -> Result {
        utf16_debug(self.as_slice(), f)
    }
    #[doc(hidden)]
    #[inline]
    fn fmt_display(&self, f: &mut Formatter<'_>) -> Result {
        utf16_display(self.as_slice(), f)
    }
}

impl DecodeUtf16<Fiber> for &[u16] {
    #[inline]
    fn decode_utf16(&self) -> Fiber {
        utf16_to_fiber(self)
    }
}
impl DecodeUtf16<String> for &[u16] {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_string(self)
    }
}

impl DecodeUtf16<Fiber> for Vec<u16> {
    #[inline]
    fn decode_utf16(&self) -> Fiber {
        utf16_to_fiber(&self)
    }
}
impl DecodeUtf16<String> for Vec<u16> {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_string(&self)
    }
}

impl<const N: usize> DecodeUtf16<Fiber> for [u16; N] {
    #[inline]
    fn decode_utf16(&self) -> Fiber {
        utf16_to_fiber(self)
    }
}
impl<const N: usize> DecodeUtf16<String> for [u16; N] {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_string(self)
    }
}

impl<const N: usize> DecodeUtf16<Fiber> for &[u16; N] {
    #[inline]
    fn decode_utf16(&self) -> Fiber {
        utf16_to_fiber(self.as_slice())
    }
}
impl<const N: usize> DecodeUtf16<String> for &[u16; N] {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_string(self.as_slice())
    }
}

impl<const N: usize> DecodeUtf16<Fiber> for Blob<u16, N> {
    #[inline]
    fn decode_utf16(&self) -> Fiber {
        utf16_to_fiber(self.as_slice())
    }
}
impl<const N: usize> DecodeUtf16<String> for Blob<u16, N> {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_string(self.as_slice())
    }
}

#[macro_export]
macro_rules! char_like {
    ($chr:ty, $owned:ident, $ptr:ident, $slice:ident, $like:ident, $trait:ident) => {
        impl $owned {
            /// This instance is "null", but it's internal buffer is empty
            /// and is considered as "null".
            ///
            /// Unless you plan to use "null" or add things to this,
            #[inline]
            pub const fn null() -> $owned {
                $owned(alloc::vec::Vec::new())
            }

            /// This is considered an "empty" value and contains a single
            /// NULL byte.
            #[inline]
            pub fn new_with_null() -> $owned {
                let mut v = $owned::null();
                v.0.push(0); // Add NULL
                v
            }
            #[inline]
            pub fn new(v: impl core::convert::AsRef<str>) -> $owned {
                $owned::from(v.as_ref())
            }
            #[inline]
            pub fn maybe<'a>(v: core::option::Option<impl core::convert::Into<$like<'a>>>) -> $like<'a> {
                match v {
                    core::option::Option::Some(v) => v.into(),
                    core::option::Option::None => $like::Null,
                }
            }

            #[inline]
            pub fn clear(&mut self) {
                self.0.clear()
            }
            #[inline]
            pub fn push(&mut self, v: $chr) {
                self.0.push(v);
            }
            #[inline]
            pub fn add_null(&mut self) -> bool {
                if self.0.last().map_or(false, |v| *v == 0) {
                    true
                } else {
                    self.0.push(0);
                    false
                }
            }
            #[inline]
            pub fn as_char_ptr<'a>(&'a self) -> $ptr<'a> {
                $ptr::new(self.as_ptr())
            }
            #[inline]
            pub fn into_vec(self) -> alloc::vec::Vec<$chr> {
                self.0
            }
            #[inline]
            pub fn extend_from_slice(&mut self, v: &[$chr]) {
                self.0.extend_from_slice(v);
            }

            #[inline]
            pub unsafe fn leak(self) -> *const $chr {
                self.0.leak().as_ptr()
            }
            #[inline]
            pub unsafe fn as_mut_vec(&mut self) -> &mut alloc::vec::Vec<$chr> {
                &mut self.0
            }
        }
        impl<'a> $ptr<'a> {
            #[inline]
            pub const fn null() -> $ptr<'a> {
                $ptr { ptr: core::ptr::null(), _p: core::marker::PhantomData }
            }
            #[inline]
            pub const fn new(v: *const $chr) -> $ptr<'a> {
                $ptr { ptr: v, _p: core::marker::PhantomData }
            }
            #[inline]
            pub const fn slice(v: &'a [$chr]) -> $ptr<'a> {
                $ptr {
                    ptr: v.as_ptr(),
                    _p:  core::marker::PhantomData,
                }
            }

            #[inline]
            pub fn fiber(v: *const $chr) -> xrmt_data::Fiber {
                $ptr::fiber_in(v, alloc::alloc::Global)
            }
            #[inline]
            pub fn string(v: *const $chr) -> alloc::string::String {
                $ptr::new(v).into_string()
            }
            #[inline]
            pub fn fiber_in<A: core::alloc::Allocator>(v: *const $chr, alloc: A) -> xrmt_data::Fiber<A> {
                $ptr::new(v).into_fiber_in(alloc)
            }

            #[inline]
            pub fn to_owned(&self) -> $owned {
                let mut v = $owned::null();
                let b = $crate::structs::StringLike::as_slice(self);
                if !b.is_empty() {
                    v.0.reserve(b.len());
                    v.0.extend_from_slice(b);
                }
                if b.last().map_or(true, |v| *v != 0) {
                    v.0.push(0); // Add NULL
                }
                v
            }
            #[inline]
            pub fn set(&mut self, v: *const $chr) {
                self.ptr = v
            }
        }
        impl<'a> $like<'a> {
            #[inline]
            pub fn into_owned(self) -> $owned {
                match self {
                    $like::Null => $owned::null(),
                    $like::Owned(v) if v.is_null_padded() => v,
                    $like::Owned(mut v) => {
                        v.0.push(0); // Add NULL
                        v
                    },
                    $like::Slice(v) => $owned::from(v.0), // Adds NULL
                    $like::Pointer(v) => v.to_owned(),    // Adds NULL
                }
            }
            /// Adds a NULL if there is no NULL at the end.
            /// Re-uses Owned structs andjust adds the NULL.
            ///
            /// NULL or NULL padded objects will remain unchanged
            #[inline]
            pub fn with_null(self) -> $like<'a> {
                if $crate::structs::StringLike::is_null(&self) || self.is_null_padded() {
                    self
                } else {
                    $like::Owned(self.into_owned())
                }
            }
            /// Maps [`None`] to [`$like::Null`]
            #[inline]
            pub fn map(&self) -> core::option::Option<&$like<'a>> {
                match self {
                    $like::Null => core::option::Option::None,
                    _ => core::option::Option::Some(self),
                }
            }

            /// Safety: This does not check if a NULL entry follows. Mainly used for
            /// structs.
            #[inline]
            pub unsafe fn as_char_ptr(&'a self) -> $ptr<'a> {
                match self {
                    $like::Null => $ptr::null(),
                    $like::Owned(v) => v.as_char_ptr(),
                    $like::Slice(v) => v.as_char_ptr(),
                    $like::Pointer(v) => *v,
                }
            }
        }
        impl<'a> $slice<'a> {
            #[inline]
            pub fn as_char_ptr(&'a self) -> $ptr<'a> {
                $ptr::new(self.0.as_ptr())
            }
        }

        impl core::ops::Deref for $owned {
            type Target = [$chr];

            #[inline]
            fn deref(&self) -> &[$chr] {
                &self.0
            }
        }
        impl core::clone::Clone for $owned {
            #[inline]
            fn clone(&self) -> $owned {
                $owned(self.0.clone())
            }
        }
        impl core::str::FromStr for $owned {
            type Err = core::convert::Infallible;

            #[inline]
            fn from_str(v: &str) -> core::result::Result<$owned, core::convert::Infallible> {
                core::result::Result::Ok($owned::from(v.as_bytes()))
            }
        }
        impl core::default::Default for $owned {
            #[inline]
            fn default() -> $owned {
                $owned::null()
            }
        }
        impl core::convert::From<&str> for $owned {
            #[inline]
            fn from(v: &str) -> $owned {
                $owned::from(v.as_bytes())
            }
        }
        impl core::cmp::PartialEq<[$chr]> for $owned {
            #[inline]
            fn eq(&self, other: &[$chr]) -> bool {
                self.0.eq(other)
            }
        }
        impl core::convert::From<&[$chr]> for $owned {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: &[$chr]) -> $owned {
                $owned(v.to_vec())
            }
        }
        impl core::convert::From<$slice<'_>> for $owned {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: $slice) -> $owned {
                $owned(v.0.to_vec())
            }
        }
        impl core::convert::From<&$slice<'_>> for $owned {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: &$slice) -> $owned {
                $owned(v.0.to_vec())
            }
        }
        impl core::convert::From<alloc::vec::Vec<$chr>> for $owned {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: alloc::vec::Vec<$chr>) -> $owned {
                $owned(v)
            }
        }
        impl core::convert::From<alloc::string::String> for $owned {
            #[inline]
            fn from(v: alloc::string::String) -> $owned {
                $owned::from(v.as_bytes())
            }
        }
        impl core::convert::From<&alloc::string::String> for $owned {
            #[inline]
            fn from(v: &alloc::string::String) -> $owned {
                $owned::from(v.as_bytes())
            }
        }
        impl<const N: usize> core::convert::From<&[$chr; N]> for $owned {
            #[inline]
            fn from(v: &[$chr; N]) -> $owned {
                $owned(v.to_vec())
            }
        }
        impl core::convert::From<core::option::Option<&str>> for $owned {
            #[inline]
            fn from(v: core::option::Option<&str>) -> $owned {
                v.map_or_else($owned::null, |x| $owned::from(x.as_bytes()))
            }
        }
        impl core::convert::From<alloc::borrow::Cow<'_, str>> for $owned {
            #[inline]
            fn from(v: alloc::borrow::Cow<'_, str>) -> $owned {
                $owned::from(v.as_bytes())
            }
        }
        impl<const N: usize> core::convert::From<xrmt_data::Blob<$chr, N>> for $owned {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: xrmt_data::Blob<$chr, N>) -> $owned {
                $owned(v.into_vec())
            }
        }
        impl core::convert::From<core::option::Option<alloc::string::String>> for $owned {
            #[inline]
            fn from(v: core::option::Option<alloc::string::String>) -> $owned {
                v.map_or_else($owned::null, |x| $owned::from(x.as_bytes()))
            }
        }
        impl core::convert::From<core::option::Option<&alloc::string::String>> for $owned {
            #[inline]
            fn from(v: core::option::Option<&alloc::string::String>) -> $owned {
                v.map_or_else($owned::null, |x| $owned::from(x.as_bytes()))
            }
        }
        impl<A: core::alloc::Allocator, const N: usize> core::convert::From<&xrmt_data::Blob<$chr, N, A>> for $owned {
            #[inline]
            fn from(v: &xrmt_data::Blob<$chr, N, A>) -> $owned {
                $owned(v.to_vec())
            }
        }

        impl core::ops::Deref for $ptr<'_> {
            type Target = [$chr];

            #[inline]
            fn deref(&self) -> &[$chr] {
                $crate::structs::StringLike::as_slice(self)
            }
        }
        impl core::marker::Copy for $ptr<'_> {}
        impl<'a> core::clone::Clone for $ptr<'a> {
            #[inline]
            fn clone(&self) -> $ptr<'a> {
                $ptr { ptr: self.ptr, _p: core::marker::PhantomData }
            }
        }
        impl<'a> core::default::Default for $ptr<'a> {
            #[inline]
            fn default() -> $ptr<'a> {
                $ptr::null()
            }
        }
        impl core::cmp::PartialEq<[$chr]> for $ptr<'_> {
            #[inline]
            fn eq(&self, other: &[$chr]) -> bool {
                $crate::structs::StringLike::as_slice(self).eq(other)
            }
        }
        impl<'a> core::convert::From<*mut $chr> for $ptr<'a> {
            #[inline]
            fn from(v: *mut $chr) -> $ptr<'a> {
                $ptr::new(v)
            }
        }
        impl<'a> core::convert::From<&'a [$chr]> for $ptr<'a> {
            #[inline]
            fn from(v: &'a [$chr]) -> $ptr<'a> {
                $ptr::slice(v)
            }
        }
        impl<'a> core::convert::From<*const $chr> for $ptr<'a> {
            #[inline]
            fn from(v: *const $chr) -> $ptr<'a> {
                $ptr::new(v)
            }
        }
        impl<'a> core::convert::From<&'a alloc::vec::Vec<$chr>> for $ptr<'a> {
            #[inline]
            fn from(v: &'a alloc::vec::Vec<$chr>) -> $ptr<'a> {
                $ptr::new(v.as_ptr())
            }
        }
        impl<'a, const N: usize> core::convert::From<&'a [$chr; N]> for $ptr<'a> {
            #[inline]
            fn from(v: &'a [$chr; N]) -> $ptr<'a> {
                $ptr::new(v.as_ptr())
            }
        }
        impl<'a, A: core::alloc::Allocator, const N: usize> core::convert::From<&'a xrmt_data::Blob<$chr, N, A>> for $ptr<'a> {
            #[inline]
            fn from(v: &'a xrmt_data::Blob<$chr, N, A>) -> $ptr<'a> {
                $ptr::new(v.as_ptr())
            }
        }

        impl core::ops::Deref for $slice<'_> {
            type Target = [$chr];

            #[inline]
            fn deref(&self) -> &[$chr] {
                self.0
            }
        }
        impl<'a> core::default::Default for $slice<'a> {
            #[inline]
            fn default() -> $slice<'a> {
                $slice(&[])
            }
        }
        impl core::cmp::PartialEq<[$chr]> for $slice<'_> {
            #[inline]
            fn eq(&self, other: &[$chr]) -> bool {
                self.0.eq(other)
            }
        }
        impl<'a> core::convert::From<&'a [$chr]> for $slice<'a> {
            #[inline]
            fn from(v: &'a [$chr]) -> $slice<'a> {
                $slice(v)
            }
        }
        impl<'a> core::convert::From<&'a alloc::vec::Vec<$chr>> for $slice<'a> {
            #[inline]
            fn from(v: &'a alloc::vec::Vec<$chr>) -> $slice<'a> {
                $slice(v.as_slice())
            }
        }
        impl<'a, const N: usize> core::convert::From<&'a [$chr; N]> for $slice<'a> {
            #[inline]
            fn from(v: &'a [$chr; N]) -> $slice<'a> {
                $slice(v.as_slice())
            }
        }
        impl<'a, A: core::alloc::Allocator, const N: usize> core::convert::From<&'a xrmt_data::Blob<$chr, N, A>> for $slice<'a> {
            #[inline]
            fn from(v: &'a xrmt_data::Blob<$chr, N, A>) -> $slice<'a> {
                $slice(&v)
            }
        }

        impl core::ops::Deref for $like<'_> {
            type Target = [$chr];

            #[inline]
            fn deref(&self) -> &[$chr] {
                $crate::structs::StringLike::as_slice(self)
            }
        }
        // Makes this an Owned object
        impl<'a> core::str::FromStr for $like<'a> {
            type Err = core::convert::Infallible;

            #[inline]
            fn from_str(v: &str) -> core::result::Result<$like<'a>, core::convert::Infallible> {
                core::result::Result::Ok($like::Owned($owned::from(v.as_bytes())))
            }
        }
        impl<'a> core::default::Default for $like<'a> {
            #[inline]
            fn default() -> $like<'a> {
                $like::Null
            }
        }
        impl core::cmp::PartialEq<[$chr]> for $like<'_> {
            #[inline]
            fn eq(&self, other: &[$chr]) -> bool {
                $crate::structs::StringLike::as_slice(self).eq(other)
            }
        }
        // Makes this an Owned object
        impl<'a> core::convert::From<&str> for $like<'a> {
            #[inline]
            fn from(v: &str) -> $like<'a> {
                $like::Owned($owned::from(v.as_bytes()))
            }
        }
        impl<'a> core::convert::From<$owned> for $like<'a> {
            #[inline]
            fn from(v: $owned) -> $like<'a> {
                $like::Owned(v)
            }
        }
        impl<'a> core::convert::From<$ptr<'a>> for $like<'a> {
            #[inline]
            fn from(v: $ptr<'a>) -> $like<'a> {
                $like::Pointer(v)
            }
        }
        impl<'a> core::convert::From<*mut $chr> for $like<'a> {
            #[inline]
            fn from(v: *mut $chr) -> $like<'a> {
                $like::Pointer($ptr::new(v))
            }
        }
        impl<'a> core::convert::From<&'a $owned> for $like<'a> {
            #[inline]
            fn from(v: &'a $owned) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a> core::convert::From<&'a [$chr]> for $like<'a> {
            #[inline]
            fn from(v: &'a [$chr]) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a> core::convert::From<$slice<'a>> for $like<'a> {
            #[inline]
            fn from(v: $slice<'a>) -> $like<'a> {
                $like::Slice(v)
            }
        }
        impl<'a> core::convert::From<*const $chr> for $like<'a> {
            #[inline]
            fn from(v: *const $chr) -> $like<'a> {
                $like::Pointer($ptr::new(v))
            }
        }
        impl<'a> core::convert::From<&'a $like<'a>> for $like<'a> {
            #[inline]
            fn from(v: &'a $like<'a>) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a> core::convert::From<&'a mut [$chr]> for $like<'a> {
            #[inline]
            fn from(v: &'a mut [$chr]) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a> core::convert::From<&'a mut $owned> for $like<'a> {
            #[inline]
            fn from(v: &'a mut $owned) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a> core::convert::From<alloc::vec::Vec<$chr>> for $like<'a> {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: alloc::vec::Vec<$chr>) -> $like<'a> {
                $like::Owned($owned(v))
            }
        }
        // Makes this an Owned object
        impl<'a> core::convert::From<alloc::string::String> for $like<'a> {
            #[inline]
            fn from(v: alloc::string::String) -> $like<'a> {
                $like::from(v.into_bytes())
            }
        }
        // Makes this an Owned object
        impl<'a> core::convert::From<&'a alloc::string::String> for $like<'a> {
            #[inline]
            fn from(v: &'a alloc::string::String) -> $like<'a> {
                $like::from(v.as_bytes())
            }
        }
        impl<'a, const N: usize> core::convert::From<[$chr; N]> for $like<'a> {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: [$chr; N]) -> $like<'a> {
                $like::Owned($owned(v.to_vec()))
            }
        }
        impl<'a> core::convert::From<&'a alloc::vec::Vec<$chr>> for $like<'a> {
            #[inline]
            fn from(v: &'a alloc::vec::Vec<$chr>) -> $like<'a> {
                $like::Slice($slice(v.as_slice()))
            }
        }
        // Makes this an Owned object
        impl<'a> core::convert::From<core::option::Option<&str>> for $like<'a> {
            #[inline]
            fn from(v: core::option::Option<&str>) -> $like<'a> {
                v.map_or($like::Null, |x| $like::Owned($owned::from(x.as_bytes())))
            }
        }
        // Makes this an Owned object
        impl<'a> core::convert::From<alloc::borrow::Cow<'_, str>> for $like<'a> {
            #[inline]
            fn from(v: alloc::borrow::Cow<'_, str>) -> $like<'a> {
                $like::Owned($owned::from(v.as_bytes()))
            }
        }
        impl<'a> core::convert::From<&'a mut alloc::vec::Vec<$chr>> for $like<'a> {
            #[inline]
            fn from(v: &'a mut alloc::vec::Vec<$chr>) -> $like<'a> {
                $like::Slice($slice(v))
            }
        }
        impl<'a, const N: usize> core::convert::From<&'a [$chr; N]> for $like<'a> {
            #[inline]
            fn from(v: &'a [$chr; N]) -> $like<'a> {
                $like::Slice($slice(v.as_slice()))
            }
        }
        impl<'a, const N: usize> core::convert::From<xrmt_data::Blob<$chr, N>> for $like<'a> {
            /// Does NOT enforce NULL end
            #[inline]
            fn from(v: xrmt_data::Blob<$chr, N>) -> $like<'a> {
                $like::Owned($owned(v.into_vec()))
            }
        }
        impl<'a, A: core::alloc::Allocator, const N: usize> core::convert::From<&'a xrmt_data::Blob<$chr, N, A>> for $like<'a> {
            #[inline]
            fn from(v: &'a xrmt_data::Blob<$chr, N, A>) -> $like<'a> {
                $like::Slice($slice(&v))
            }
        }

        impl core::convert::From<$owned> for xrmt_data::Fiber {
            #[inline]
            fn from(v: $owned) -> xrmt_data::Fiber {
                v.into_fiber()
            }
        }
        impl core::convert::From<&$owned> for xrmt_data::Fiber {
            #[inline]
            fn from(v: &$owned) -> xrmt_data::Fiber {
                v.to_fiber()
            }
        }
        impl core::convert::From<$ptr<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: $ptr<'_>) -> xrmt_data::Fiber {
                v.into_fiber()
            }
        }
        impl core::convert::From<&$ptr<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: &$ptr<'_>) -> xrmt_data::Fiber {
                v.to_fiber()
            }
        }
        impl core::convert::From<$like<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: $like<'_>) -> xrmt_data::Fiber {
                v.into_fiber()
            }
        }
        impl core::convert::From<&$like<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: &$like<'_>) -> xrmt_data::Fiber {
                v.to_fiber()
            }
        }
        impl core::convert::From<$slice<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: $slice<'_>) -> xrmt_data::Fiber {
                v.into_fiber()
            }
        }
        impl core::convert::From<&$slice<'_>> for xrmt_data::Fiber {
            #[inline]
            fn from(v: &$slice<'_>) -> xrmt_data::Fiber {
                v.to_fiber()
            }
        }

        impl core::convert::From<$owned> for alloc::string::String {
            #[inline]
            fn from(v: $owned) -> alloc::string::String {
                v.into_string()
            }
        }
        impl core::convert::From<&$owned> for alloc::string::String {
            #[inline]
            fn from(v: &$owned) -> alloc::string::String {
                v.to_string()
            }
        }
        impl core::convert::From<$ptr<'_>> for alloc::string::String {
            #[inline]
            fn from(v: $ptr<'_>) -> alloc::string::String {
                v.into_string()
            }
        }
        impl core::convert::From<&$ptr<'_>> for alloc::string::String {
            #[inline]
            fn from(v: &$ptr<'_>) -> alloc::string::String {
                v.to_string()
            }
        }
        impl core::convert::From<$like<'_>> for alloc::string::String {
            #[inline]
            fn from(v: $like<'_>) -> alloc::string::String {
                v.into_string()
            }
        }
        impl core::convert::From<&$like<'_>> for alloc::string::String {
            #[inline]
            fn from(v: &$like<'_>) -> alloc::string::String {
                v.to_string()
            }
        }
        impl core::convert::From<$slice<'_>> for alloc::string::String {
            #[inline]
            fn from(v: $slice<'_>) -> alloc::string::String {
                v.into_string()
            }
        }
        impl core::convert::From<&$slice<'_>> for alloc::string::String {
            #[inline]
            fn from(v: &$slice<'_>) -> alloc::string::String {
                v.to_string()
            }
        }

        impl $crate::structs::StringLike<$chr> for $owned {
            #[inline]
            fn len(&self) -> usize {
                self.0.len() // NULL is included in len.
            }
            #[inline]
            fn is_null(&self) -> bool {
                self.0.is_empty()
            }
            #[inline]
            fn is_empty(&self) -> bool {
                // Empty if only item is a NULL
                self.0.is_empty() || (self.0.len() == 1 && unsafe { *self.0.get_unchecked(0) == 0 })
            }
            #[inline]
            fn as_slice(&self) -> &[$chr] {
                &self.0
            }
            #[inline]
            fn as_ptr(&self) -> *const $chr {
                if self.0.is_empty() {
                    core::ptr::null()
                } else {
                    self.0.as_ptr()
                }
            }
            #[inline]
            fn as_mut_ptr(&mut self) -> *mut $chr {
                if self.0.is_empty() {
                    core::ptr::null_mut()
                } else {
                    self.0.as_mut_ptr()
                }
            }
        }
        impl<'a> $crate::structs::StringLike<$chr> for $ptr<'a> {
            fn len(&self) -> usize {
                if self.ptr.is_null() {
                    return 0;
                }
                let mut n = 0usize;
                while unsafe { *self.ptr.add(n) > 0 } {
                    n += 1
                }
                // Include NULL in len
                n + 1
            }
            #[inline]
            fn is_null(&self) -> bool {
                self.ptr.is_null()
            }
            #[inline]
            fn is_empty(&self) -> bool {
                self.ptr.is_null() || self.len() == 0
            }
            #[inline]
            fn as_slice(&self) -> &[$chr] {
                let n = self.len();
                if n == 0 {
                    &[]
                } else {
                    // NULL is included in this
                    unsafe { core::slice::from_raw_parts(self.ptr, n) }
                }
            }
            #[inline]
            fn as_ptr(&self) -> *const $chr {
                if self.is_null() {
                   core::ptr::null()
                } else {
                    self.ptr
                }
            }
            #[inline]
            fn as_mut_ptr(&mut self) -> *mut $chr {
                if self.is_null() {
                    core::ptr::null_mut()
                } else {
                    self.ptr as *mut $chr
                }
            }
        }
        impl<'a> $crate::structs::StringLike<$chr> for $like<'a> {
            #[inline]
            fn len(&self) -> usize {
                match self {
                    $like::Null => 0,
                    $like::Owned(v) => v.len(),
                    $like::Slice(v) => v.len(),
                    $like::Pointer(v) => v.len(),
                }
            }
            #[inline]
            fn is_null(&self) -> bool {
                match self {
                    $like::Null => true,
                    $like::Owned(v) => v.is_null(),
                    $like::Slice(v) => v.is_null(),
                    $like::Pointer(v) => v.is_null(),
                }
            }
            #[inline]
            fn is_empty(&self) -> bool {
                match self {
                    $like::Null => true,
                    $like::Owned(v) => v.is_empty(),
                    $like::Slice(v) => v.is_empty(),
                    $like::Pointer(v) => v.is_empty(),
                }
            }
            #[inline]
            fn as_slice(&self) -> &[$chr] {
                match self {
                    $like::Null => &[],
                    $like::Owned(v) => v.as_slice(),
                    $like::Slice(v) => v.0,
                    $like::Pointer(v) => v.as_slice(),
                }
            }
            #[inline]
            fn as_ptr(&self) -> *const $chr {
                match self {
                    $like::Null => core::ptr::null(),
                    $like::Owned(v) => v.as_ptr(),
                    $like::Slice(v) => v.as_ptr(),
                    $like::Pointer(v) => v.as_ptr(),
                }
            }
            #[inline]
            fn as_mut_ptr(&mut self) -> *mut $chr {
                match self {
                    $like::Owned(v) => v.as_mut_ptr(),
                    $like::Pointer(v) => v.as_mut_ptr(),
                    _ => core::ptr::null_mut(),
                }
            }
        }
        impl<'a> $crate::structs::StringLike<$chr> for $slice<'a> {
            #[inline]
            fn len(&self) -> usize {
                self.0.len()
            }
            #[inline]
            fn is_null(&self) -> bool {
                self.0.is_empty()
            }
            #[inline]
            fn is_empty(&self) -> bool {
                self.0.is_empty() || (self.0.len() == 1 && unsafe { *self.0.get_unchecked(0) == 0 })
            }
            #[inline]
            fn as_slice(&self) -> &[$chr] {
                &self.0
            }
            #[inline]
            fn as_ptr(&self) -> *const $chr {
                if self.0.is_empty() {
                    core::ptr::null()
                } else {
                    self.0.as_ptr()
                }
            }
        }

        impl $trait for $owned {}
        impl<'a> $trait for $ptr<'a> {}
        impl<'a> $trait for $like<'a> {
            #[inline]
            fn is_null_padded(&self) -> bool {
                match self {
                    $like::Null => false,
                    $like::Owned(v) => v.is_null_padded(),
                    $like::Slice(v) => v.is_null_padded(),
                    $like::Pointer(v) => v.is_null_padded(),
                }
            }
            #[inline]
            fn len_without_null(&self) -> usize {
                match self {
                    $like::Null => 0,
                    $like::Owned(v) => v.len_without_null(),
                    $like::Slice(v) => v.len_without_null(),
                    $like::Pointer(v) => v.len_without_null(),
                }
            }
        }
        impl<'a> $trait for $slice<'a> {}

        #[cfg(not(feature = "strip"))]
        impl core::fmt::Debug for $owned {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_debug(f)
            }
        }
        #[cfg(not(feature = "strip"))]
        impl core::fmt::Display for $owned {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_display(f)
            }
        }

        #[cfg(not(feature = "strip"))]
        impl core::fmt::Debug for $ptr<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_debug(f)
            }
        }
        #[cfg(not(feature = "strip"))]
        impl core::fmt::Display for $ptr<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_display(f)
            }
        }

        #[cfg(not(feature = "strip"))]
        impl core::fmt::Debug for $slice<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_debug(f)
            }
        }
        #[cfg(not(feature = "strip"))]
        impl core::fmt::Display for $slice<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_display(f)
            }
        }

        #[cfg(not(feature = "strip"))]
        impl core::fmt::Debug for $like<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_debug(f)
            }
        }
        #[cfg(not(feature = "strip"))]
        impl core::fmt::Display for $like<'_> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.fmt_display(f)
            }
        }
    };
}

#[macro_export]
macro_rules! str_const {
    ($crypt:expr, $str:expr, $len:expr, $n:ident) => {
        let mut $n = [0u16; $len];
        unsafe { xrmt_data::text::utf8_to_utf16_unchecked(&mut $n, xrmt_crypt::crypt!($crypt, $str)) };
    };
}

#[macro_export]
macro_rules! unicode_string {
    ($name:expr, $n:ident) => {
        let __wchar_name = core::convert::Into::<$crate::structs::WCharLike>::into($name);
        core::debug_assert!($crate::structs::StringLike::is_null(&__wchar_name) || $crate::structs::StringLikeU16::is_null_padded(&__wchar_name));
        let $n = $crate::structs::UnicodeString::new(&__wchar_name);
    };
    (wchars $name:expr, $n:ident) => {
        let __wchar_val = core::convert::Into::<$crate::structs::WChars>::into($name);
        let __wchar_name = $crate::structs::WCharLike::Slice(&__wchar_val);
        core::debug_assert!($crate::structs::StringLike::is_null(&__wchar_name) || $crate::structs::StringLikeU16::is_null_padded(&__wchar_name));
        let $n = $crate::structs::UnicodeString::new(&__wchar_name);
    };
}
#[macro_export]
macro_rules! unicode_display {
    ($($n:ident),+) => {
        $(
            $crate::unicode_display!(_ $n);
        )+
    };
    (_ $n:ident) => {
        #[cfg(not(feature = "strip"))]
        impl core::fmt::Display for $n {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let v = self.as_slice();
                if !v.is_empty() {
                    xrmt_data::text::utf16_to_func(v, |c| {
                        let _ = f.write_str(c.as_str());
                    });
                }
                core::fmt::Result::Ok(())
            }
        }
    };
}
