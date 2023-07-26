// Copyright (C) 2023 iDigitalFlame
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
#![cfg(windows)]

use alloc::borrow::Cow;
use core::iter;

use crate::data::blob::Blob;
use crate::util::stx::ffi::{OsString, PathBuf};
use crate::util::stx::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::ansi::*;
pub use self::block::*;
pub use self::char::*;
pub use self::unicode::*;
pub use self::wchar::*;

pub trait DecodeUtf16 {
    fn decode_utf16(&self) -> String;
}
pub trait MaybeString {
    fn into_string(&self) -> Option<&str>;
}

pub type Chars = Blob<u8, 256>;
pub type WChars = Blob<u16, 128>;

impl MaybeString for &str {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for String {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for Option<&str> {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        *self
    }
}
impl MaybeString for Cow<'_, str> {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}

impl From<Char> for String {
    #[inline]
    fn from(v: Char) -> String {
        v.into_string()
    }
}
impl From<WChar> for String {
    #[inline]
    fn from(v: WChar) -> String {
        v.into_string()
    }
}
impl From<CharPtr> for String {
    #[inline]
    fn from(v: CharPtr) -> String {
        v.into_string()
    }
}
impl From<WCharPtr> for String {
    #[inline]
    fn from(v: WCharPtr) -> String {
        v.into_string()
    }
}
impl From<AnsiString> for String {
    #[inline]
    fn from(v: AnsiString) -> String {
        v.into_string()
    }
}
impl From<StringBlock> for String {
    #[inline]
    fn from(v: StringBlock) -> String {
        v.into_string()
    }
}
impl From<UnicodeString> for String {
    #[inline]
    fn from(v: UnicodeString) -> String {
        v.into_string()
    }
}

impl From<CharPtr> for PathBuf {
    #[inline]
    fn from(v: CharPtr) -> PathBuf {
        v.into_string().into()
    }
}
impl From<WCharPtr> for PathBuf {
    #[inline]
    fn from(v: WCharPtr) -> PathBuf {
        v.into_string().into()
    }
}
impl From<UnicodeString> for PathBuf {
    #[inline]
    fn from(v: UnicodeString) -> PathBuf {
        v.into_string().into()
    }
}

impl From<CharPtr> for OsString {
    #[inline]
    fn from(v: CharPtr) -> OsString {
        v.into_string().into()
    }
}
impl From<WCharPtr> for OsString {
    #[inline]
    fn from(v: WCharPtr) -> OsString {
        v.into_string().into()
    }
}
impl From<UnicodeString> for OsString {
    #[inline]
    fn from(v: UnicodeString) -> OsString {
        v.into_string().into()
    }
}

impl<const N: usize> ToString for Blob<u16, N> {
    #[inline]
    fn to_string(&self) -> String {
        utf16_to_str(self.as_slice())
    }
}
impl<const N: usize> From<&str> for Blob<u16, N> {
    #[inline]
    fn from(s: &str) -> Blob<u16, N> {
        if s.len() == 0 {
            Blob::new()
        } else {
            s.encode_utf16().chain(iter::once(0)).collect::<Blob<u16, N>>()
        }
    }
}
impl<const N: usize> From<String> for Blob<u16, N> {
    #[inline]
    fn from(s: String) -> Blob<u16, N> {
        Blob::from(s.as_str())
    }
}
impl<const N: usize> From<Option<&str>> for Blob<u16, N> {
    #[inline]
    fn from(s: Option<&str>) -> Blob<u16, N> {
        s.map_or_else(Blob::new, |v| Blob::from(v))
    }
}
impl<const N: usize> From<Option<String>> for Blob<u16, N> {
    #[inline]
    fn from(s: Option<String>) -> Blob<u16, N> {
        s.map_or_else(Blob::new, |v| Blob::from(v.as_str()))
    }
}

impl<const N: usize> From<&str> for Blob<u8, N> {
    #[inline]
    fn from(s: &str) -> Blob<u8, N> {
        // NOTE(dij): As much as using '.into()' would work, we have to remember
        //            that we need the NUL byte after for these to work.
        s.as_bytes().iter().copied().chain(iter::once(0)).collect::<Blob<u8, N>>()
    }
}
impl<const N: usize> From<String> for Blob<u8, N> {
    #[inline]
    fn from(s: String) -> Blob<u8, N> {
        s.as_bytes().iter().copied().chain(iter::once(0)).collect::<Blob<u8, N>>()
    }
}
impl<const N: usize> From<Option<&str>> for Blob<u8, N> {
    #[inline]
    fn from(s: Option<&str>) -> Blob<u8, N> {
        s.map_or_else(Blob::new, |v| v.into())
    }
}
impl<const N: usize> From<Option<String>> for Blob<u8, N> {
    #[inline]
    fn from(s: Option<String>) -> Blob<u8, N> {
        s.map_or_else(Blob::new, |v| v.as_str().into())
    }
}

impl DecodeUtf16 for &[u16] {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_str(self)
    }
}
impl<const N: usize> DecodeUtf16 for [u16; N] {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_str(self)
    }
}
impl<const N: usize> DecodeUtf16 for Blob<u16, N> {
    #[inline]
    fn decode_utf16(&self) -> String {
        utf16_to_str(self.as_slice())
    }
}

#[inline]
pub fn utf16_to_str(v: &[u16]) -> String {
    if v.len() == 0 {
        String::new()
    } else {
        char::decode_utf16(v.iter().cloned())
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    }
}
#[inline]
pub fn utf16_to_str_trim(v: &[u16]) -> String {
    if v.len() == 0 {
        String::new()
    } else {
        match v.iter().position(|v| *v == 0) {
            Some(i) => utf16_to_str(&v[0..i]),
            None => utf16_to_str(v),
        }
    }
}

mod char {
    use core::ops::Deref;
    use core::{iter, ptr, slice};

    use crate::data::blob::Blob;
    use crate::util::stx::ffi::{OsStr, OsString, Path, PathBuf};
    use crate::util::stx::prelude::*;

    pub struct Char(Vec<u8>);
    #[repr(transparent)]
    pub struct CharPtr(*const u8);

    impl Char {
        #[inline]
        pub fn new(v: impl AsRef<str>) -> Char {
            Char::from(v.as_ref())
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.0.len()
        }
        #[inline]
        pub fn is_null(&self) -> bool {
            self.0.is_empty()
        }
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u8] {
            &self.0
        }
        #[inline]
        pub fn as_ptr(&self) -> *const u8 {
            if self.0.is_empty() {
                ptr::null()
            } else {
                self.0.as_ptr()
            }
        }
        #[inline]
        pub fn resize(&mut self, n: usize) {
            self.0.resize(n, 0)
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.0.is_empty() {
                String::new()
            } else {
                unsafe { core::str::from_utf8_unchecked(&self.0) }.to_string()
            }
        }
        #[inline]
        pub fn as_char_ptr(&self) -> CharPtr {
            CharPtr(self.as_ptr())
        }
        #[inline]
        pub fn as_mut_ptr(&mut self) -> *mut u8 {
            if self.0.is_empty() {
                ptr::null_mut()
            } else {
                self.0.as_mut_ptr()
            }
        }
    }
    impl CharPtr {
        #[inline]
        pub fn null() -> CharPtr {
            CharPtr(ptr::null_mut())
        }
        #[inline]
        pub fn new(v: *const u8) -> CharPtr {
            CharPtr(v)
        }
        #[inline]
        pub fn string(v: *const u8) -> String {
            CharPtr(v).into_string()
        }

        pub fn len(&self) -> usize {
            if self.0.is_null() {
                return 0;
            }
            let mut n = 0;
            while unsafe { *self.0.add(n) } > 0 {
                n += 1
            }
            n
        }
        #[inline]
        pub fn is_null(&self) -> bool {
            self.0.is_null()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u8] {
            unsafe { slice::from_raw_parts(self.0, self.len()) }
        }
        #[inline]
        pub fn as_ptr(&self) -> *const u8 {
            self.0
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.is_null() {
                String::new()
            } else {
                unsafe { core::str::from_utf8_unchecked(self.as_slice()) }.to_string()
            }
        }
        #[inline]
        pub fn set(&mut self, v: *const u8) {
            self.0 = v
        }
        #[inline]
        pub fn to_blob(&self) -> Blob<u8, 256> {
            self.as_slice().into()
        }
        #[inline]
        pub fn as_mut_ptr(&mut self) -> *mut u8 {
            self.0 as *mut u8
        }
    }

    impl Clone for Char {
        #[inline]
        fn clone(&self) -> Char {
            Char(self.0.clone())
        }
    }
    impl Deref for Char {
        type Target = [u8];

        #[inline]
        fn deref(&self) -> &[u8] {
            &self.0
        }
    }
    impl Default for Char {
        #[inline]
        fn default() -> Char {
            Char(Vec::new())
        }
    }
    impl From<&str> for Char {
        #[inline]
        fn from(s: &str) -> Char {
            Char::from(s.as_bytes())
        }
    }
    impl From<&[u8]> for Char {
        #[inline]
        fn from(s: &[u8]) -> Char {
            if s.len() == 0 {
                Char::default()
            } else {
                Char(s.iter().copied().chain(iter::once(0)).collect::<Vec<u8>>())
            }
        }
    }
    impl From<&Path> for Char {
        #[inline]
        fn from(v: &Path) -> Char {
            Char::from(v.to_string_lossy().as_bytes())
        }
    }
    impl From<&OsStr> for Char {
        #[inline]
        fn from(v: &OsStr) -> Char {
            Char::from(v.to_string_lossy().as_bytes())
        }
    }
    impl From<String> for Char {
        #[inline]
        fn from(s: String) -> Char {
            Char::from(s.as_bytes())
        }
    }
    impl From<PathBuf> for Char {
        #[inline]
        fn from(v: PathBuf) -> Char {
            Char::from(v.to_string_lossy().as_bytes())
        }
    }
    impl From<OsString> for Char {
        #[inline]
        fn from(v: OsString) -> Char {
            Char::from(v.to_string_lossy().as_bytes())
        }
    }
    impl From<Option<&str>> for Char {
        #[inline]
        fn from(s: Option<&str>) -> Char {
            s.map_or_else(Char::default, |v| Char::from(v.as_bytes()))
        }
    }
    impl From<Option<String>> for Char {
        #[inline]
        fn from(s: Option<String>) -> Char {
            s.map_or_else(Char::default, |v| Char::from(v.as_bytes()))
        }
    }

    impl Copy for CharPtr {}
    impl Clone for CharPtr {
        #[inline]
        fn clone(&self) -> CharPtr {
            CharPtr(self.0)
        }
    }
    impl Deref for CharPtr {
        type Target = [u8];

        #[inline]
        fn deref(&self) -> &[u8] {
            self.as_slice()
        }
    }
}
mod ansi {
    use core::slice;

    use super::CharPtr;
    use crate::data::blob::Blob;
    use crate::util::stx::prelude::*;

    #[repr(C)]
    pub struct AnsiString {
        pub length:     u16,
        pub max_length: u16,
        pub buffer:     CharPtr,
    }

    impl AnsiString {
        #[inline]
        pub fn new(buf: &[u8]) -> AnsiString {
            let e = buf.last().map_or(false, |v| *v == 0);
            AnsiString {
                buffer:     CharPtr::new(buf.as_ptr()),
                length:     buf.len() as u16 - if e { 1 } else { 0 }, // Remove NUL
                max_length: buf.len() as u16,
            }
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.length as usize
        }
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.length == 0 || self.max_length == 0 || self.buffer.is_null()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u8] {
            unsafe { slice::from_raw_parts(self.buffer.as_ptr(), self.length as usize) }
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                String::new()
            } else {
                unsafe {
                    core::str::from_utf8_unchecked(slice::from_raw_parts(
                        self.buffer.as_ptr(),
                        self.length as usize,
                    ))
                }
                .to_string()
            }
        }
        #[inline]
        pub fn to_blob(&self) -> Blob<u8, 256> {
            self.as_slice().iter().collect::<Blob<u8, 256>>()
        }
    }
}
mod wchar {
    use core::ops::Deref;
    use core::{iter, ptr, slice};

    use crate::data::blob::Blob;
    use crate::util::stx::ffi::{OsStr, OsString, Path, PathBuf};
    use crate::util::stx::prelude::*;

    pub struct WChar(Vec<u16>);
    #[repr(transparent)]
    pub struct WCharPtr(*const u16);

    impl WChar {
        #[inline]
        pub fn new(v: impl AsRef<str>) -> WChar {
            WChar::from(v.as_ref())
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.0.len()
        }
        #[inline]
        pub fn is_null(&self) -> bool {
            self.0.is_empty()
        }
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u16] {
            &self.0
        }
        #[inline]
        pub fn as_ptr(&self) -> *const u16 {
            if self.0.is_empty() {
                ptr::null()
            } else {
                self.0.as_ptr()
            }
        }
        #[inline]
        pub fn resize(&mut self, n: usize) {
            self.0.resize(n, 0)
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.0.is_empty() {
                String::new()
            } else {
                super::utf16_to_str(&self.0)
            }
        }
        #[inline]
        pub fn as_wchar_ptr(&self) -> WCharPtr {
            WCharPtr(self.as_ptr())
        }
        #[inline]
        pub fn as_mut_ptr(&mut self) -> *mut u16 {
            if self.0.is_empty() {
                ptr::null_mut()
            } else {
                self.0.as_mut_ptr()
            }
        }
    }
    impl WCharPtr {
        #[inline]
        pub fn null() -> WCharPtr {
            WCharPtr(ptr::null_mut())
        }
        #[inline]
        pub fn new(v: *const u16) -> WCharPtr {
            WCharPtr(v)
        }
        #[inline]
        pub fn string(v: *const u16) -> String {
            WCharPtr(v).into_string()
        }

        pub fn len(&self) -> usize {
            if self.0.is_null() {
                return 0;
            }
            let mut n = 0;
            while unsafe { *self.0.add(n) > 0 } {
                n += 1
            }
            n
        }
        #[inline]
        pub fn is_null(&self) -> bool {
            self.0.is_null()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u16] {
            unsafe { slice::from_raw_parts(self.0, self.len()) }
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.is_null() {
                String::new()
            } else {
                super::utf16_to_str(self.as_slice())
            }
        }
        #[inline]
        pub fn as_ptr(&self) -> *const u16 {
            self.0
        }
        #[inline]
        pub fn set(&mut self, v: *const u16) {
            self.0 = v
        }
        #[inline]
        pub fn as_mut_ptr(&mut self) -> *mut u16 {
            self.0 as *mut u16
        }
        #[inline]
        pub fn to_u8_blob(&self) -> Blob<u8, 256> {
            self.as_slice().iter().map(|v| *v as u8).collect::<Blob<u8, 256>>()
        }
        #[inline]
        pub fn to_u16_blob(&self) -> Blob<u16, 256> {
            self.as_slice().iter().collect::<Blob<u16, 256>>()
        }
    }

    impl Clone for WChar {
        #[inline]
        fn clone(&self) -> WChar {
            WChar(self.0.clone())
        }
    }
    impl Deref for WChar {
        type Target = [u16];

        #[inline]
        fn deref(&self) -> &[u16] {
            &self.0
        }
    }
    impl Default for WChar {
        #[inline]
        fn default() -> WChar {
            WChar(Vec::new())
        }
    }
    impl From<&str> for WChar {
        #[inline]
        fn from(s: &str) -> WChar {
            if s.len() == 0 {
                WChar::default()
            } else {
                WChar(s.encode_utf16().chain(iter::once(0)).collect::<Vec<u16>>())
            }
        }
    }
    impl From<&Path> for WChar {
        #[inline]
        fn from(v: &Path) -> WChar {
            WChar::from(&*v.to_string_lossy())
        }
    }
    impl From<&OsStr> for WChar {
        #[inline]
        fn from(v: &OsStr) -> WChar {
            WChar::from(&*v.to_string_lossy())
        }
    }
    impl From<String> for WChar {
        #[inline]
        fn from(s: String) -> WChar {
            WChar::from(s.as_str())
        }
    }
    impl From<PathBuf> for WChar {
        #[inline]
        fn from(v: PathBuf) -> WChar {
            WChar::from(&*v.to_string_lossy())
        }
    }
    impl From<OsString> for WChar {
        #[inline]
        fn from(v: OsString) -> WChar {
            WChar::from(&*v.to_string_lossy())
        }
    }
    impl From<Option<&str>> for WChar {
        #[inline]
        fn from(s: Option<&str>) -> WChar {
            s.map_or_else(WChar::default, |v| WChar::from(v))
        }
    }
    impl From<Option<String>> for WChar {
        #[inline]
        fn from(s: Option<String>) -> WChar {
            s.map_or_else(WChar::default, |v| WChar::from(v.as_str()))
        }
    }

    impl Copy for WCharPtr {}
    impl Clone for WCharPtr {
        #[inline]
        fn clone(&self) -> WCharPtr {
            WCharPtr(self.0)
        }
    }
    impl Deref for WCharPtr {
        type Target = [u16];

        #[inline]
        fn deref(&self) -> &[u16] {
            self.as_slice()
        }
    }
}
mod block {
    use core::ops::Deref;
    use core::slice;

    use crate::data::blob::Blob;
    use crate::util::stx::ffi::OsString;
    use crate::util::stx::prelude::*;

    pub struct VariableIter<'a> {
        pos: usize,
        env: &'a StringBlock,
    }
    pub struct Variable<'a>(&'a [u16]);
    #[repr(transparent)]
    pub struct StringBlock(*const u16);

    impl StringBlock {
        pub fn len(&self) -> usize {
            if self.0.is_null() {
                return 0;
            }
            let (mut n, mut c) = (0, 0);
            loop {
                if unsafe { *self.0.add(n) } == 0 {
                    c += 1;
                    if c == 2 {
                        break;
                    }
                } else {
                    c = 0;
                }
                n += 1
            }
            n
        }
        #[inline]
        pub fn is_null(&self) -> bool {
            self.0.is_null()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u16] {
            unsafe { slice::from_raw_parts(self.0, self.len()) }
        }
        #[inline]
        pub fn into_string(self) -> String {
            self.to_string()
        }
        #[inline]
        pub fn as_ptr(&self) -> *const u16 {
            self.0
        }
        #[inline]
        pub fn iter<'a>(&'a self) -> VariableIter<'a> {
            VariableIter { pos: 0, env: self }
        }
        #[inline]
        pub fn entries(&self) -> Vec<(OsString, OsString)> {
            self.iter().map(|v| v.into()).collect::<Vec<(OsString, OsString)>>()
        }
        #[inline]
        pub fn find(&self, key: impl AsRef<str>) -> Option<String> {
            let k = key.as_ref().encode_utf16().collect::<Blob<u16, 256>>();
            self.iter().find(|v| v.is_key(&k))?.value_as_string()
        }
        #[inline]
        pub fn find_as_blob(&self, key: impl AsRef<str>) -> Option<Blob<u8, 256>> {
            let k = key.as_ref().encode_utf16().collect::<Blob<u16, 256>>();
            self.iter().find(|v| v.is_key(&k))?.value_as_blob()
        }

        unsafe fn next_entry(&self, pos: usize) -> Option<(&[u16], usize)> {
            let (mut n, mut c) = (pos, 0);
            loop {
                match *self.0.add(n) {
                    0 => {
                        c += 1;
                        if c == 2 {
                            break;
                        }
                        if n - pos > 1 {
                            return Some((slice::from_raw_parts(self.0.add(pos), n - pos), n + 1));
                        }
                    },
                    _ => c = 0,
                }
                n += 1
            }
            None
        }
    }
    impl<'a> Variable<'a> {
        #[inline]
        pub fn as_slice(&self) -> &'a [u16] {
            self.0
        }
        #[inline]
        pub fn is_key(&self, key: &[u16]) -> bool {
            match self.0[1..].iter().position(|v| *v == b'=' as u16) {
                Some(i) => i + 1 > 1 && i + 1 == key.len() && Variable::match_u16(&self.0[0..i + 1], key),
                None => false,
            }
        }
        #[inline]
        pub fn value_as_string(&self) -> Option<String> {
            Some(super::utf16_to_str(unsafe { self.value() }?))
        }
        #[inline]
        pub fn value_as_blob(&self) -> Option<Blob<u8, 256>> {
            Some(
                unsafe { self.value() }?
                    .iter()
                    .map(|v| *v as u8)
                    .collect::<Blob<u8, 256>>(),
            )
        }

        #[inline]
        pub unsafe fn key(&self) -> Option<&'a [u16]> {
            Some(&self.0[0..*(&self.0[1..].iter().position(|v| *v == b'=' as u16)?) + 1])
        }
        #[inline]
        pub unsafe fn value(&self) -> Option<&'a [u16]> {
            Some(&self.0[*(&self.0[1..].iter().position(|v| *v == b'=' as u16)?) + 2..])
        }

        fn match_u16(src: &[u16], with: &[u16]) -> bool {
            if src.len() != with.len() {
                return false;
            }
            for i in 0..src.len() {
                match i {
                    _ if src[i] < b'a' as u16 && with[i] < b'a' as u16 && src[i] == with[i] => (),
                    _ if src[i] >= b'a' as u16 && with[i] >= b'a' as u16 && src[i] == with[i] => (),
                    _ if src[i] < b'a' as u16 && with[i] >= b'a' as u16 && src[i] == with[i] - 0x20 => (),
                    _ if src[i] >= b'a' as u16 && with[i] < b'a' as u16 && src[i] - 0x20 == with[i] => (),
                    _ => return false,
                }
            }
            true
        }
    }

    impl Copy for StringBlock {}
    impl Clone for StringBlock {
        #[inline]
        fn clone(&self) -> StringBlock {
            StringBlock(self.0)
        }
    }
    impl Deref for StringBlock {
        type Target = [u16];

        #[inline]
        fn deref(&self) -> &[u16] {
            self.as_slice()
        }
    }

    impl<'a> Iterator for VariableIter<'a> {
        type Item = Variable<'a>;

        #[inline]
        fn next(&mut self) -> Option<Variable<'a>> {
            let (r, n) = unsafe { self.env.next_entry(self.pos) }?;
            self.pos = n;
            Some(Variable(r))
        }
    }
    impl Into<(OsString, OsString)> for Variable<'_> {
        #[inline]
        fn into(self) -> (OsString, OsString) {
            match (unsafe { self.key() }, unsafe { self.value() }) {
                (Some(k), Some(v)) => (
                    super::utf16_to_str_trim(k).into(),
                    super::utf16_to_str(v).into(),
                ),
                (..) => (super::utf16_to_str_trim(self.0).into(), OsString::new()),
            }
        }
    }
}
mod unicode {
    use core::mem::ManuallyDrop;
    use core::ops::Deref;
    use core::{ptr, slice};

    use super::{WChar, WCharPtr};
    use crate::data::blob::Blob;
    use crate::util::stx::ffi::{OsStr, OsString, Path, PathBuf};
    use crate::util::stx::prelude::*;

    pub struct UnicodeStr {
        pub value: UnicodeString,
        buf:       ManuallyDrop<WChar>,
    }
    #[repr(C)]
    pub struct UnicodeString {
        pub length:     u16,
        pub max_length: u16,
        pub buffer:     WCharPtr,
    }

    impl UnicodeStr {
        #[inline]
        pub fn new(v: impl AsRef<str>) -> UnicodeStr {
            UnicodeStr::from(v.as_ref())
        }

        #[inline]
        pub fn is_empty(&self) -> bool {
            self.length == 0 || self.max_length == 0 || self.buf.is_empty()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u16] {
            self.buf.as_slice()
        }
    }
    impl UnicodeString {
        #[inline]
        pub fn new(buf: &[u16]) -> UnicodeString {
            let e = buf.last().map_or(false, |v| *v == 0);
            UnicodeString {
                buffer:     WCharPtr::new(buf.as_ptr()),
                length:     (buf.len() as u16 - if e { 1 } else { 0 }) * 2, // Remove NUL
                max_length: buf.len() as u16 * 2,
            }
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.length as usize
        }
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.length == 0 || self.max_length == 0 || self.buffer.is_null()
        }
        #[inline]
        pub fn as_slice(&self) -> &[u16] {
            unsafe { slice::from_raw_parts(self.buffer.as_ptr(), (self.length / 2) as usize) }
        }
        #[inline]
        pub fn into_string(self) -> String {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                String::new()
            } else {
                super::utf16_to_str(self.as_slice())
            }
        }
        #[inline]
        pub fn to_u8_blob(&self) -> Blob<u8, 256> {
            self.as_slice().iter().map(|v| *v as u8).collect::<Blob<u8, 256>>()
        }
        #[inline]
        pub fn to_u16_blob(&self) -> Blob<u16, 256> {
            self.as_slice().iter().collect::<Blob<u16, 256>>()
        }

        #[inline]
        pub(crate) fn hash(&self) -> u32 {
            let mut h: u32 = 0x811C9DC5;
            for i in self.as_slice() {
                h = h.wrapping_mul(0x1000193);
                h ^= match *i as u8 {
                    b'A'..=b'Z' => *i + 0x20,
                    _ => *i,
                } as u32;
            }
            h
        }
    }

    impl Drop for UnicodeStr {
        #[inline]
        fn drop(&mut self) {
            if self.value.buffer.is_null() {
                return;
            }
            unsafe { ManuallyDrop::drop(&mut self.buf) }
            self.value.buffer.set(ptr::null());
            self.value.max_length = 0;
            self.value.length = 0;
        }
    }
    impl Deref for UnicodeStr {
        type Target = UnicodeString;

        #[inline]
        fn deref(&self) -> &UnicodeString {
            &self.value
        }
    }
    impl Default for UnicodeStr {
        #[inline]
        fn default() -> UnicodeStr {
            UnicodeStr {
                buf:   ManuallyDrop::new(WChar::default()),
                value: UnicodeString::default(),
            }
        }
    }
    impl From<&str> for UnicodeStr {
        #[inline]
        fn from(s: &str) -> UnicodeStr {
            let n = s.len() as u16 * 2;
            // NOTE(dij): ^ Length captured here so we don't need to worry
            //              about subtracting the NUL value from length.
            if n == 0 {
                return UnicodeStr::default();
            }
            let mut r = UnicodeStr {
                buf:   ManuallyDrop::new(WChar::from(s)),
                value: UnicodeString {
                    buffer:     WCharPtr::null(),
                    length:     n,
                    max_length: n,
                },
            };
            r.value.buffer = r.buf.as_wchar_ptr();
            r
        }
    }
    impl From<&Path> for UnicodeStr {
        #[inline]
        fn from(v: &Path) -> UnicodeStr {
            UnicodeStr::from(&*v.to_string_lossy())
        }
    }
    impl From<&OsStr> for UnicodeStr {
        #[inline]
        fn from(v: &OsStr) -> UnicodeStr {
            UnicodeStr::from(&*v.to_string_lossy())
        }
    }
    impl From<String> for UnicodeStr {
        #[inline]
        fn from(s: String) -> UnicodeStr {
            UnicodeStr::from(s.as_str())
        }
    }
    impl From<PathBuf> for UnicodeStr {
        #[inline]
        fn from(v: PathBuf) -> UnicodeStr {
            UnicodeStr::from(&*v.to_string_lossy())
        }
    }
    impl From<OsString> for UnicodeStr {
        #[inline]
        fn from(v: OsString) -> UnicodeStr {
            UnicodeStr::from(&*v.to_string_lossy())
        }
    }
    impl From<Option<&str>> for UnicodeStr {
        #[inline]
        fn from(s: Option<&str>) -> UnicodeStr {
            s.map_or_else(UnicodeStr::default, |v| UnicodeStr::from(v))
        }
    }
    impl From<Option<String>> for UnicodeStr {
        #[inline]
        fn from(s: Option<String>) -> UnicodeStr {
            s.map_or_else(UnicodeStr::default, |v| UnicodeStr::from(v.as_str()))
        }
    }

    impl Default for UnicodeString {
        #[inline]
        fn default() -> UnicodeString {
            UnicodeString {
                buffer:     WCharPtr::null(),
                length:     0,
                max_length: 0,
            }
        }
    }
}

#[cfg(feature = "implant")]
mod display {
    use core::fmt::{self, Debug, Formatter};

    use super::{AnsiString, Char, CharPtr, StringBlock, UnicodeStr, UnicodeString, Variable, WChar, WCharPtr};
    use crate::util::stx::prelude::*;

    impl Debug for Char {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for Char {
        #[inline]
        fn to_string(&self) -> String {
            if self.len() == 0 {
                String::new()
            } else {
                unsafe { core::str::from_utf8_unchecked(self.as_slice()) }.to_string()
            }
        }
    }

    impl Debug for CharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for CharPtr {
        #[inline]
        fn to_string(&self) -> String {
            if self.is_null() {
                String::new()
            } else {
                unsafe { core::str::from_utf8_unchecked(self.as_slice()) }.to_string()
            }
        }
    }

    impl Debug for AnsiString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for AnsiString {
        #[inline]
        fn to_string(&self) -> String {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                String::new()
            } else {
                unsafe { core::str::from_utf8_unchecked(self.as_slice()) }.to_string()
            }
        }
    }

    impl Debug for WChar {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for WChar {
        #[inline]
        fn to_string(&self) -> String {
            if self.len() == 0 {
                String::new()
            } else {
                super::utf16_to_str_trim(self.as_slice())
            }
        }
    }

    impl Debug for WCharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for WCharPtr {
        #[inline]
        fn to_string(&self) -> String {
            if self.is_null() {
                String::new()
            } else {
                super::utf16_to_str(self.as_slice())
            }
        }
    }

    impl Debug for StringBlock {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for StringBlock {
        #[inline]
        fn to_string(&self) -> String {
            let mut b = String::new();
            for (i, v) in self.iter().enumerate() {
                if i > 0 {
                    b.push('|');
                }
                b.push_str(&v.to_string());
            }
            b
        }
    }

    impl Debug for Variable<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for Variable<'_> {
        #[inline]
        fn to_string(&self) -> String {
            super::utf16_to_str(self.as_slice())
        }
    }

    impl Debug for UnicodeStr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.value.to_string())
        }
    }
    impl ToString for UnicodeStr {
        #[inline]
        fn to_string(&self) -> String {
            self.value.to_string()
        }
    }

    impl Debug for UnicodeString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.to_string())
        }
    }
    impl ToString for UnicodeString {
        #[inline]
        fn to_string(&self) -> String {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                String::new()
            } else {
                super::utf16_to_str(self.as_slice())
            }
        }
    }
}
#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, Write};

    use super::{AnsiString, Char, CharPtr, StringBlock, UnicodeStr, UnicodeString, Variable, WChar, WCharPtr};
    use crate::util::stx::prelude::*;

    impl Debug for Char {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Char {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.len() == 0 {
                Ok(())
            } else {
                f.write_str(unsafe { core::str::from_utf8_unchecked(self.as_slice()) })
            }
        }
    }

    impl Debug for CharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for CharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.is_null() {
                Ok(())
            } else {
                f.write_str(unsafe { core::str::from_utf8_unchecked(self.as_slice()) })
            }
        }
    }

    impl Debug for AnsiString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for AnsiString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                Ok(())
            } else {
                f.write_str(unsafe { core::str::from_utf8_unchecked(self.as_slice()) })
            }
        }
    }

    impl Debug for WChar {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for WChar {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.len() == 0 {
                Ok(())
            } else {
                f.write_str(&super::utf16_to_str(self.as_slice()))
            }
        }
    }

    impl Debug for WCharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for WCharPtr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.is_null() {
                Ok(())
            } else {
                f.write_str(&super::utf16_to_str(self.as_slice()))
            }
        }
    }

    impl Debug for StringBlock {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for StringBlock {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            for (i, v) in self.iter().enumerate() {
                if i > 0 {
                    f.write_char('|')?;
                }
                f.write_str(&v.to_string())?;
            }
            Ok(())
        }
    }

    impl Debug for Variable<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Variable<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&super::utf16_to_str(self.as_slice()))
        }
    }

    impl Debug for UnicodeStr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(&self.value, f)
        }
    }
    impl Display for UnicodeStr {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(&self.value, f)
        }
    }

    impl Debug for UnicodeString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for UnicodeString {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.length == 0 || self.max_length == 0 || self.buffer.is_null() {
                Ok(())
            } else {
                f.write_str(&super::utf16_to_str(self.as_slice()))
            }
        }
    }
}
