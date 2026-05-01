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
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

extern crate xrmt_data;

use core::iter::{FusedIterator, Iterator};
use core::option::Option;
use core::slice::Iter;

use xrmt_data::text::{utf16_to_vec, U16Encoder};

use crate::ffi::{OsStr, OsString};

pub struct EncodeWide<'a>(U16Encoder<'a, Iter<'a, u8>>);

/// Windows-specific extensions to [`OsStr`].
///
/// This trait is sealed: it cannot be implemented outside the standard library.
/// This is so that future additional methods are not breaking changes.
pub trait OsStrExt {
    /// Re-encodes an `OsStr` as a wide character sequence, i.e., potentially
    /// ill-formed UTF-16.
    ///
    /// This is lossless: calling [`OsStringExt::from_wide`] and then
    /// `encode_wide` on the result will yield the original code units.
    /// Note that the encoding does not add a final null terminator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// // UTF-16 encoding for "Unicode".
    /// let source = [0x0055, 0x006E, 0x0069, 0x0063, 0x006F, 0x0064, 0x0065];
    ///
    /// let string = OsString::from_wide(&source[..]);
    ///
    /// let result: Vec<u16> = string.encode_wide().collect();
    /// assert_eq!(&source[..], &result[..]);
    /// ```
    fn encode_wide(&self) -> EncodeWide<'_>;
}
/// Windows-specific extensions to [`OsString`].
///
/// This trait is sealed: it cannot be implemented outside the standard library.
/// This is so that future additional methods are not breaking changes.
pub trait OsStringExt {
    /// Creates an `OsString` from a potentially ill-formed UTF-16 slice of
    /// 16-bit code units.
    ///
    /// This is lossless: calling [`OsStrExt::encode_wide`] on the resulting
    /// string will always return the original code units.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::ffi::OsString;
    /// use xrmt_stx::os::windows::prelude::*;
    ///
    /// // UTF-16 encoding for "Unicode".
    /// let source = [0x0055, 0x006E, 0x0069, 0x0063, 0x006F, 0x0064, 0x0065];
    ///
    /// let string = OsString::from_wide(&source[..]);
    /// ```
    fn from_wide(wide: &[u16]) -> Self;
}

impl OsStrExt for OsStr {
    #[inline]
    fn encode_wide(&self) -> EncodeWide<'_> {
        EncodeWide(U16Encoder::new(self.as_bytes()))
    }
}
impl OsStringExt for OsString {
    #[inline]
    fn from_wide(wide: &[u16]) -> OsString {
        let mut v = OsString::new();
        utf16_to_vec(v.as_mut_vec(), wide);
        v
    }
}

impl Iterator for EncodeWide<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        self.0.next()
    }
}
impl FusedIterator for EncodeWide<'_> {}
