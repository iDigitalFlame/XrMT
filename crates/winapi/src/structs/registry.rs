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
#![cfg(target_family = "windows")]

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Into};
use core::default::Default;
use core::hint::unreachable_unchecked;
use core::iter::Iterator;
use core::marker::Copy;
use core::mem::forget;
use core::ops::{Deref, Drop};
use core::option::Option::{self, Some};
use core::result::Result::{Err, Ok};
use core::slice::from_raw_parts;

use xrmt_data::text::{utf16_to_string, U16Encoder};
use xrmt_data::Blob;

use crate::env::expand_slice;
use crate::functions::{close_handle, len_to_u32, DuplicateHandleEx, GetEnvironment, RegCloseKey, RegDeleteKey, RegDeleteKeyValue, RegFlushKey, RegGetKeys, RegGetValues, RegQueryInfoKey, RegQueryValueEx, RegSetValueEx};
use crate::registry::{create_key, delete_key_tree, open_key};
use crate::structs::{FileTime, Handle, RegKeyIter, RegValueIter, StringLike, StringLikeU16, WCharLike};
use crate::{unicode_display, Win32Error, Win32Result, CURRENT_PROCESS};

pub const VALUE_STRING: u8 = 1u8;
pub const VALUE_EXPAND_STRING: u8 = 2u8;
pub const VALUE_BINARY: u8 = 3u8;
pub const VALUE_DWORD: u8 = 4u8;
pub const VALUE_STRING_LIST: u8 = 7u8;
pub const VALUE_QWORD: u8 = 11u8;

pub const HKEY_ROOT: Key = Key(Handle::new(0usize));
pub const HKEY_USERS: Key = Key(Handle::new(0x80000003usize));
pub const HKEY_CLASSES_ROOT: Key = Key(Handle::new(0x80000000usize));
pub const HKEY_CURRENT_USER: Key = Key(Handle::new(0x80000001usize));
pub const HKEY_LOCAL_MACHINE: Key = Key(Handle::new(0x80000002usize));
pub const HKEY_CURRENT_CONFIG: Key = Key(Handle::new(0x80000005usize));

pub enum RegValue {
    Dword(u32),
    Qword(u64),
    String(String),
    Binary(Vec<u8>),
    ExpandString(String),
    StringList(Vec<String>),
}

#[repr(transparent)]
pub struct Key(Handle);
pub struct RegKeyInfo {
    pub subkey_count:       u32,
    pub max_subkey_len:     u32,
    pub max_class_len:      u32,
    pub value_count:        u32,
    pub max_value_name_len: u32,
    pub max_value_len:      u32,
    pub last_write:         FileTime,
}
#[repr(transparent)]
pub struct OwnedKey(Key);
#[repr(C)]
pub struct RegKeyFullInfo {
    pub last_write:         FileTime,
    pub index:              u32,
    pub class_offset:       u32,
    pub class_length:       u32,
    pub subkeys:            u32,
    pub max_name_len:       u32,
    pub max_class_len:      u32,
    pub values:             u32,
    pub max_value_name_len: u32,
    pub max_value_len:      u32,
    pub class_name:         [u16; 128],
    // KEY_FULL_INFORMATION
}
#[repr(C)]
pub struct RegKeyBasicInfo {
    pub last_write:  FileTime,
    pub index:       u32,
    pub name_length: u32,
    pub name:        [u16; 128],
    // KEY_BASIC_INFORMATION
}
#[repr(C)]
pub struct RegValueFullInfo {
    pub index:       u32,
    pub value_type:  u32,
    pub data_offset: u32,
    pub data_length: u32,
    pub name_length: u32,
    pub name:        [u16; 128],
    // KEY_VALUE_FULL_INFORMATION
}

impl Key {
    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn close(self) -> Win32Result<()> {
        if self.is_invalid() || self.is_predefined() {
            return Ok(());
        }
        RegCloseKey(self)
    }
    #[inline]
    pub fn flush(&self) -> Win32Result<()> {
        RegFlushKey(*self)
    }
    #[inline]
    pub fn stat(&self) -> Win32Result<(String, RegKeyInfo)> {
        RegQueryInfoKey(*self)
    }
    #[inline]
    pub fn subkeys<'a>(&self) -> Win32Result<RegKeyIter<'a>> {
        RegGetKeys(*self)
    }
    #[inline]
    pub fn values<'a>(&self) -> Win32Result<RegValueIter<'a>> {
        RegGetValues(*self)
    }
    #[inline]
    pub fn delete<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<()> {
        RegDeleteKeyValue(*self, value)
    }
    #[inline]
    pub fn value<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<RegValue> {
        let mut b: Blob<u8, 64> = Blob::new();
        data(RegQueryValueEx(*self, value, &mut b, 64)?, &b)
    }
    #[inline]
    pub fn delete_tree<'a>(&self, subkey: impl Into<WCharLike<'a>>) -> Win32Result<()> {
        delete_key_tree(*self, subkey)
    }
    #[inline]
    pub fn delete_subkey<'a>(&self, subkey: impl Into<WCharLike<'a>>) -> Win32Result<()> {
        RegDeleteKey(*self, subkey, 0)
    }
    #[inline]
    pub fn value_as_str<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<String> {
        match self.value_checked(VALUE_STRING, value)? {
            RegValue::String(v) => Ok(v),
            RegValue::ExpandString(v) => Ok(v),
            // Tell the compiler that it can only be a value we want.
            // 'value_checked' can't return anything that's NOT what we wanted.
            _ => unsafe { unreachable_unchecked() },
        }
    }
    #[inline]
    pub fn value_as_number<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<u64> {
        match self.value_checked(VALUE_QWORD, value)? {
            RegValue::Dword(v) => Ok(v as u64),
            RegValue::Qword(v) => Ok(v),
            // Tell the compiler that it can only be a value we want.
            // 'value_checked' can't return anything that's NOT what we wanted.
            _ => unsafe { unreachable_unchecked() },
        }
    }
    #[inline]
    pub fn value_as_binary<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<Vec<u8>> {
        match self.value_checked(VALUE_BINARY, value)? {
            RegValue::Binary(v) => Ok(v),
            // Tell the compiler that it can only be a value we want.
            // 'value_checked' can't return anything that's NOT what we wanted.
            _ => unsafe { unreachable_unchecked() },
        }
    }
    #[inline]
    pub fn open<'a>(&self, subkey: impl Into<WCharLike<'a>>, access: u32) -> Win32Result<OwnedKey> {
        open_key(*self, subkey, access)
    }
    #[inline]
    pub fn set_value<'a>(&self, value: impl Into<WCharLike<'a>>, data: RegValue) -> Win32Result<()> {
        let mut b: Blob<u8, 64> = Blob::new();
        let t = data.type_value();
        match data {
            RegValue::Dword(v) => b.extend_from_slice(&v.to_le_bytes()),
            RegValue::Qword(v) => b.extend_from_slice(&v.to_le_bytes()),
            RegValue::Binary(v) => {
                b.extend_from_slice(&v);
                b.push(0);
            },
            RegValue::StringList(v) => {
                for i in v {
                    b.reserve(i.len() * 2);
                    // Convert string to UTF16, then write it as raw u8's.
                    for c in U16Encoder::new(i.as_bytes()) {
                        b.extend_from_slice(&c.to_le_bytes());
                    }
                    b.push(0); // Add NULL
                }
                b.push(0); // Add NULL
            },
            RegValue::String(v) | RegValue::ExpandString(v) => {
                b.reserve(v.len() * 2);
                // Convert string to UTF16, then write it as raw u8's.
                for c in U16Encoder::new(v.as_bytes()) {
                    b.extend_from_slice(&c.to_le_bytes());
                }
                b.push(0); // Add NULL
            },
        }
        RegSetValueEx(*self, value, t as u32, Some(&b), len_to_u32(b.len()))
    }
    #[inline]
    pub fn value_as_str_list<'a>(&self, value: impl Into<WCharLike<'a>>) -> Win32Result<Vec<String>> {
        match self.value_checked(VALUE_STRING_LIST, value)? {
            RegValue::StringList(v) => Ok(v),
            // Tell the compiler that it can only be a value we want.
            // 'value_checked' can't return anything that's NOT what we wanted.
            _ => unsafe { unreachable_unchecked() },
        }
    }
    #[inline]
    pub fn value_checked<'a>(&self, ty: u8, value: impl Into<WCharLike<'a>>) -> Win32Result<RegValue> {
        let mut b: Blob<u8, 64> = Blob::new();
        let t = RegQueryValueEx(*self, value, &mut b, 64)?;
        // Support calling for String and u64 that could have either value.
        if t == ty || (ty == VALUE_STRING && t == VALUE_EXPAND_STRING) || (ty == VALUE_QWORD && t == VALUE_DWORD) {
            data(t, &b)
        } else {
            Err(Win32Error::InvalidType)
        }
    }
    #[inline]
    pub fn create<'a>(&self, subkey: impl Into<WCharLike<'a>>, access: u32) -> Win32Result<(OwnedKey, bool)> {
        create_key(*self, subkey, access)
    }
    #[inline]
    pub fn set_value_raw<'a>(&self, value: impl Into<WCharLike<'a>>, ty: u8, data: Option<impl AsRef<[u8]>>) -> Win32Result<()> {
        let n = data.as_ref().map_or(0, |v| len_to_u32(v.as_ref().len()));
        RegSetValueEx(*self, value, ty as u32, data, n)
    }
    #[inline]
    pub fn value_raw<'a, A: Allocator, const N: usize>(&self, value: impl Into<WCharLike<'a>>, buf: &mut Blob<u8, N, A>, expected: u32) -> Win32Result<u8> {
        RegQueryValueEx(*self, value, buf, expected)
    }

    #[inline]
    pub unsafe fn take(v: OwnedKey) -> Key {
        let h = v.0;
        forget(v); // Prevent double close
        h
    }

    #[inline]
    pub(crate) fn is_root(&self) -> bool {
        *self.0 & 0xFFFFFFF == 0
    }
    #[inline]
    pub(crate) fn is_predefined(&self) -> bool {
        *self.0 & 0xF0000000 == 0x80000000
    }
}
impl OwnedKey {
    #[inline]
    pub fn duplicate(&self) -> Win32Result<OwnedKey> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        Ok(Key(DuplicateHandleEx(
            self,
            CURRENT_PROCESS,
            CURRENT_PROCESS,
            0,
            false,
            0x2,
        )?)
        .into())
    }
}
impl RegValue {
    #[inline]
    pub fn type_value(&self) -> u8 {
        match self {
            RegValue::Dword(_) => VALUE_DWORD,
            RegValue::Qword(_) => VALUE_QWORD,
            RegValue::String(_) => VALUE_STRING,
            RegValue::Binary(_) => VALUE_BINARY,
            RegValue::ExpandString(_) => VALUE_EXPAND_STRING,
            RegValue::StringList(_) => VALUE_STRING_LIST,
        }
    }
}
impl RegValueFullInfo {
    #[inline]
    pub fn value_raw(&self) -> &[u8] {
        unsafe {
            from_raw_parts(
                (self as *const RegValueFullInfo as *const u8).add(self.data_offset as usize),
                self.data_length as usize,
            )
        }
    }
    #[inline]
    pub fn value(&self) -> Win32Result<RegValue> {
        data(self.value_type as u8, self.value_raw())
    }
}

impl Eq for Key {}
impl Copy for Key {}
impl Clone for Key {
    #[inline]
    fn clone(&self) -> Key {
        Key(self.0)
    }
}
impl Deref for Key {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        &self.0
    }
}
impl Default for Key {
    #[inline]
    fn default() -> Key {
        Key(Handle::default())
    }
}
impl PartialEq for Key {
    #[inline]
    fn eq(&self, other: &Key) -> bool {
        self.0 == other.0
    }
}
impl AsRef<Handle> for Key {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

impl Eq for OwnedKey {}
impl Drop for OwnedKey {
    #[inline]
    fn drop(&mut self) {
        if self.is_invalid() || self.is_predefined() {
            return;
        }
        unsafe { close_handle(self.0 .0) }
    }
}
impl Deref for OwnedKey {
    type Target = Key;

    #[inline]
    fn deref(&self) -> &Key {
        &self.0
    }
}
impl PartialEq for OwnedKey {
    #[inline]
    fn eq(&self, other: &OwnedKey) -> bool {
        self.0 .0 == other.0 .0
    }
}
impl From<Key> for OwnedKey {
    #[inline]
    fn from(v: Key) -> OwnedKey {
        OwnedKey(v)
    }
}
impl AsRef<Handle> for OwnedKey {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0 .0
    }
}

impl From<&RegKeyFullInfo> for RegKeyInfo {
    #[inline]
    fn from(v: &RegKeyFullInfo) -> RegKeyInfo {
        RegKeyInfo {
            last_write:         FileTime::from(v.last_write),
            value_count:        v.values,
            subkey_count:       v.subkeys,
            max_class_len:      v.max_class_len,
            max_value_len:      v.max_value_len,
            max_subkey_len:     v.max_name_len,
            max_value_name_len: v.max_value_name_len,
        }
    }
}

impl StringLikeU16 for RegKeyFullInfo {}
impl StringLike<u16> for RegKeyFullInfo {
    #[inline]
    fn len(&self) -> usize {
        self.class_length as usize / 2
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.class_length == 0
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        unsafe { from_raw_parts(self.as_ptr(), self.len()) }
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        unsafe { (self as *const RegKeyFullInfo as *const u8).add(self.class_offset as usize) as *const u16 }
    }
}

impl StringLikeU16 for RegKeyBasicInfo {}
impl StringLike<u16> for RegKeyBasicInfo {
    #[inline]
    fn len(&self) -> usize {
        self.name_length as usize / 2
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.name_length == 0
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        unsafe { from_raw_parts(self.name.as_ptr(), self.len()) }
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        self.name.as_ptr()
    }
}

impl StringLikeU16 for RegValueFullInfo {}
impl StringLike<u16> for RegValueFullInfo {
    #[inline]
    fn len(&self) -> usize {
        self.name_length as usize / 2
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.name_length == 0
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        unsafe { from_raw_parts(self.name.as_ptr(), self.len()) }
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        self.name.as_ptr()
    }
}

fn data(t: u8, b: &[u8]) -> Win32Result<RegValue> {
    match t {
        VALUE_DWORD if b.len() < 4 => Err(Win32Error::InvalidSize),
        VALUE_DWORD => Ok(RegValue::Dword(u32::from_le_bytes(unsafe {
            *(b.as_ptr() as *const [u8; 4])
        }))),
        VALUE_QWORD if b.len() < 8 => Err(Win32Error::InvalidSize),
        VALUE_QWORD => Ok(RegValue::Qword(u64::from_le_bytes(unsafe {
            *(b.as_ptr() as *const [u8; 8])
        }))),
        VALUE_BINARY => Ok(RegValue::Binary(b.to_vec())),
        VALUE_STRING => Ok(RegValue::String(utf16_to_string(unsafe {
            from_raw_parts(b.as_ptr() as *const u16, b.len() / 2)
        }))),
        VALUE_STRING_LIST => {
            let v = unsafe { from_raw_parts(b.as_ptr() as *const u16, b.len() / 2) };
            let (mut n, mut l) = (Vec::new(), 0);
            for (i, d) in unsafe { v.get_unchecked(0..v.len().saturating_sub(1)) }.iter().enumerate() {
                if *d > 0 {
                    continue;
                }
                if i.saturating_sub(l) > 0 {
                    // Always in bounds
                    n.push(utf16_to_string(unsafe { v.get_unchecked(l..i) }))
                }
                l += 1;
            }
            Ok(RegValue::StringList(n))
        },
        VALUE_EXPAND_STRING => Ok(RegValue::ExpandString(utf16_to_string(&expand_slice(
            unsafe { from_raw_parts(b.as_ptr() as *const u16, b.len() / 2) },
            GetEnvironment(),
        )))),
        _ => Err(Win32Error::InvalidType),
    }
}

unicode_display!(RegKeyBasicInfo, RegKeyFullInfo, RegValueFullInfo);

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::registry::{Key, OwnedKey};
    use crate::structs::RegValue;

    impl Debug for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "Key: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0, f)
        }
    }

    impl Debug for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "OwnedKey: 0x{:X}", self.0 .0)
        }
    }
    impl LowerHex for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0 .0, f)
        }
    }
    impl UpperHex for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0 .0, f)
        }
    }

    impl Debug for RegValue {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                RegValue::Dword(v) => f.debug_tuple("Dword").field(v).finish(),
                RegValue::Qword(v) => f.debug_tuple("Qword").field(v).finish(),
                RegValue::String(v) => f.debug_tuple("String").field(v).finish(),
                RegValue::Binary(v) => f.debug_tuple("Binary").field(v).finish(),
                RegValue::ExpandString(v) => f.debug_tuple("ExpandString").field(v).finish(),
                RegValue::StringList(v) => f.debug_tuple("StringList").field(v).finish(),
            }
        }
    }
}
