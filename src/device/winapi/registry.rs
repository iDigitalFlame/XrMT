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

use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use core::ops::Deref;
use core::{cmp, iter, mem, slice};

use crate::data::blob::Blob;
use crate::device::winapi::{self, AsHandle, DecodeUtf16, Handle, MaybeString, RegKeyInfo, Win32Error, Win32Result};
use crate::util::stx::prelude::*;

pub const VALUE_STRING: u8 = 1;
pub const VALUE_EXPAND_STRING: u8 = 2;
pub const VALUE_BINARY: u8 = 3;
pub const VALUE_DWORD: u8 = 4;
pub const VALUE_STRING_LIST: u8 = 7;
pub const VALUE_QWORD: u8 = 11;

pub const HKEY_CLASSES_ROOT: Key = Key(Handle(0x80000000));
pub const HKEY_CURRENT_USER: Key = Key(Handle(0x80000001));
pub const HKEY_LOCAL_MACHINE: Key = Key(Handle(0x80000002));
pub const HKEY_USERS: Key = Key(Handle(0x80000003));
pub const HKEY_CURRENT_CONFIG: Key = Key(Handle(0x80000005));

pub enum TypeError {
    InvalidSize,
    InvalidType(u8),
    Err(Win32Error),
}

#[repr(transparent)]
pub struct OwnedKey(Key);
#[repr(transparent)]
pub struct Key(pub(super) Handle);

impl Key {
    #[inline]
    pub fn take(v: OwnedKey) -> Key {
        let h = v.0;
        mem::forget(v); // Prevent double close
        h
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn close(self) -> Win32Result<()> {
        if self.is_invalid() || self.is_predefined() {
            return Ok(());
        }
        winapi::RegCloseKey(self)
    }
    #[inline]
    pub fn flush(&self) -> Win32Result<()> {
        winapi::RegFlushKey(*self)
    }
    #[inline]
    pub fn values(&self) -> Win32Result<Vec<String>> {
        winapi::RegGetValueNames(*self)
    }
    #[inline]
    pub fn subkeys(&self) -> Win32Result<Vec<String>> {
        winapi::RegGetKeyNames(*self)
    }
    #[inline]
    pub fn stat(&self) -> Win32Result<(String, RegKeyInfo)> {
        winapi::RegQueryInfoKey(*self)
    }
    #[inline]
    pub fn delete(&self, value: impl MaybeString) -> Win32Result<()> {
        delete_value(*self, value)
    }
    #[inline]
    pub fn delete_key(&self, subkey: impl AsRef<str>) -> Win32Result<()> {
        delete_key(*self, subkey)
    }
    #[inline]
    pub fn delete_tree(&self, subkey: impl MaybeString) -> Win32Result<()> {
        delete_tree(*self, subkey)
    }

    #[inline]
    pub fn open(&self, subkey: impl MaybeString, access: u32) -> Win32Result<OwnedKey> {
        open(*self, subkey, access)
    }
    pub fn value_number(&self, value: impl MaybeString) -> Result<(u64, u8), TypeError> {
        let (b, t) = self.value_raw(value, 8).map_err(|e| TypeError::Err(e))?;
        match t {
            VALUE_DWORD => {
                if b.len() != 4 {
                    return Err(TypeError::InvalidSize);
                }
                Ok((
                    u32::from_le_bytes(b.as_array::<4>().ok_or(TypeError::InvalidSize)?) as u64,
                    t,
                ))
            },
            VALUE_QWORD => {
                if b.len() != 8 {
                    return Err(TypeError::InvalidSize);
                }
                Ok((
                    u64::from_le_bytes(b.as_array::<8>().ok_or(TypeError::InvalidSize)?),
                    t,
                ))
            },
            _ => Err(TypeError::InvalidType(t)),
        }
    }
    #[inline]
    pub fn set_value_dword(&self, value: impl MaybeString, data: u32) -> Win32Result<()> {
        let b = data.to_le_bytes();
        winapi::RegSetValueEx(*self, value, VALUE_DWORD as u32, Some(b), 4)
    }
    #[inline]
    pub fn set_value_qword(&self, value: impl MaybeString, data: u64) -> Win32Result<()> {
        let b = data.to_le_bytes();
        winapi::RegSetValueEx(*self, value, VALUE_QWORD as u32, Some(b), 8)
    }
    pub fn value_string(&self, value: impl MaybeString) -> Result<(String, u8), TypeError> {
        let (b, t) = self.value_raw(value, 64).map_err(|e| TypeError::Err(e))?;
        match t {
            VALUE_STRING | VALUE_EXPAND_STRING => (),
            _ => return Err(TypeError::InvalidType(t)),
        }
        if b.len() == 0 {
            return Ok((String::new(), t));
        }
        let v = unsafe { b.as_slice_of() };
        // SAFETY: Always in bounds.
        Ok((unsafe { v.get_unchecked(0..v.len() - 1) }.decode_utf16(), t))
    }
    pub fn value_strings(&self, value: impl MaybeString) -> Result<Vec<String>, TypeError> {
        let (b, t) = self.value_raw(value, 8).map_err(|e| TypeError::Err(e))?;
        if t != VALUE_STRING_LIST {
            return Err(TypeError::InvalidType(t));
        }
        let mut r = Vec::new();
        if b.len() == 0 {
            return Ok(r);
        }
        let s = unsafe { b.as_slice_of() };
        let mut last = 0;
        // SAFETY: Always in bounds.
        for (i, v) in unsafe { s.get_unchecked(0..s.len() - 1) }.iter().enumerate() {
            if *v > 0 {
                continue;
            }
            if i - last > 0 {
                // SAFETY: Always in bounds.
                r.push(unsafe { s.get_unchecked(last..i) }.decode_utf16());
            }
            last = i + 1;
        }
        Ok(r)
    }
    #[inline]
    pub fn value_binary(&self, value: impl MaybeString) -> Result<Blob<u8, 256>, TypeError> {
        let (b, t) = self.value_raw(value, 64).map_err(|e| TypeError::Err(e))?;
        if t != VALUE_BINARY {
            Err(TypeError::InvalidType(t))
        } else {
            Ok(b)
        }
    }
    #[inline]
    pub fn create(&self, subkey: impl AsRef<str>, access: u32) -> Win32Result<(OwnedKey, bool)> {
        create(*self, subkey, access)
    }
    #[inline]
    pub fn value_raw(&self, value: impl MaybeString, size: u32) -> Win32Result<(Blob<u8, 256>, u8)> {
        winapi::RegQueryValueEx(*self, value, size)
    }
    #[inline]
    pub fn set_value_string(&self, value: impl MaybeString, data: impl AsRef<str>) -> Win32Result<()> {
        let t = data.as_ref().encode_utf16().chain(iter::once(0)).collect::<Vec<u16>>();
        let b = unsafe { slice::from_raw_parts(t.as_ptr() as *const u8, t.len() * 2) };
        winapi::RegSetValueEx(
            *self,
            value,
            VALUE_STRING as u32,
            Some(b),
            cmp::min(b.len(), u32::MAX as usize) as u32,
        )
    }
    pub fn set_value_strings(&self, value: impl MaybeString, data: Vec<impl AsRef<str>>) -> Win32Result<()> {
        let mut t: Blob<u16, 256> = Blob::with_capacity(64 * data.len());
        for i in data {
            t.extend(i.as_ref().encode_utf16().chain(iter::once(0)))
        }
        t.push(0);
        let b = unsafe { t.as_slice_of() };
        winapi::RegSetValueEx(
            *self,
            value,
            VALUE_STRING_LIST as u32,
            Some(b),
            cmp::min(b.len(), u32::MAX as usize) as u32,
        )
    }
    #[inline]
    pub fn set_value_binary(&self, value: impl MaybeString, data: Option<impl AsRef<[u8]>>) -> Win32Result<()> {
        let n = data
            .as_ref()
            .map_or(0, |v| cmp::min(v.as_ref().len(), u32::MAX as usize) as u32);
        winapi::RegSetValueEx(*self, value, VALUE_BINARY as u32, data, n)
    }
    #[inline]
    pub fn set_value_raw(&self, value: impl MaybeString, value_type: u8, data: Option<impl AsRef<[u8]>>) -> Win32Result<()> {
        let n = data
            .as_ref()
            .map_or(0, |v| cmp::min(v.as_ref().len(), u32::MAX as usize) as u32);
        winapi::RegSetValueEx(*self, value, value_type as u32, data, n)
    }

    #[inline]
    pub(super) fn is_class_root(&self) -> bool {
        self.0 .0 & 0xFFFFFFF == 0
    }
    #[inline]
    pub(super) fn is_predefined(&self) -> bool {
        self.0 .0 & 0xF0000000 == 0x80000000
    }
}
impl OwnedKey {
    #[inline]
    pub fn duplicate(&self) -> Win32Result<OwnedKey> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        Ok(Key(winapi::DuplicateHandleEx(
            self,
            winapi::CURRENT_PROCESS,
            winapi::CURRENT_PROCESS,
            0,
            false,
            0x2,
        )?)
        .into())
    }
}
impl TypeError {
    #[inline]
    pub fn is_err(&self) -> bool {
        match self {
            TypeError::InvalidType(_) => false,
            _ => true,
        }
    }
    #[inline]
    pub fn real_type(&self) -> u8 {
        match self {
            TypeError::InvalidType(t) => *t,
            _ => 0,
        }
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
impl AsHandle for Key {
    fn as_handle(&self) -> Handle {
        self.0
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
        self.0 .0 == other.0 .0
    }
}

impl Eq for OwnedKey {}
impl Drop for OwnedKey {
    #[inline]
    fn drop(&mut self) {
        if self.is_invalid() || self.is_predefined() {
            return;
        }
        winapi::close_handle(self.0 .0)
    }
}
impl Deref for OwnedKey {
    type Target = Key;

    #[inline]
    fn deref(&self) -> &Key {
        &self.0
    }
}
impl AsHandle for OwnedKey {
    fn as_handle(&self) -> Handle {
        self.0 .0
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

impl Error for TypeError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for TypeError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for TypeError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::InvalidSize => Display::fmt(&Win32Error::UnexpectedKeySize, f),
            TypeError::InvalidType(_) => Display::fmt(&Win32Error::UnexpectedKeyType, f),
            TypeError::Err(e) => Display::fmt(e, f),
        }
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::{Key, OwnedKey};
    use crate::util::stx::prelude::*;

    impl Debug for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Key: 0x{:X}", self.0 .0)
        }
    }
    impl LowerHex for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0 .0, f)
        }
    }
    impl UpperHex for Key {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0 .0, f)
        }
    }

    impl Debug for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "OwnedKey: 0x{:X}", self.0 .0 .0)
        }
    }
    impl LowerHex for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0 .0 .0, f)
        }
    }
    impl UpperHex for OwnedKey {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0 .0 .0, f)
        }
    }
}

#[inline]
pub fn delete_key(key: Key, subkey: impl AsRef<str>) -> Win32Result<()> {
    winapi::RegDeleteKey(key, subkey, 0)
}
#[inline]
pub fn delete_value(key: Key, value: impl MaybeString) -> Win32Result<()> {
    winapi::RegDeleteKeyValue(key, value)
}
#[inline]
pub fn delete_tree(key: Key, subkey: impl MaybeString) -> Win32Result<()> {
    winapi::RegDeleteTree(key, subkey)
}
#[inline]
pub fn open(key: Key, subkey: impl MaybeString, access: u32) -> Win32Result<OwnedKey> {
    winapi::RegOpenKeyEx(key, subkey, 0, access).map(|v| v.into())
}
#[inline]
pub fn create(key: Key, subkey: impl AsRef<str>, access: u32) -> Win32Result<(OwnedKey, bool)> {
    winapi::RegCreateKeyEx(key, subkey, None, 0, access, None).map(|v| (v.0.into(), v.1))
}
