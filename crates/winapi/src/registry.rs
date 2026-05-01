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

extern crate core;

use core::convert::Into;
use core::option::Option::None;

use crate::functions::{RegCreateKeyEx, RegDeleteKey, RegDeleteKeyValue, RegDeleteTree, RegOpenKeyEx};
use crate::structs::WCharLike;
use crate::Win32Result;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use crate::structs::{Key, OwnedKey, HKEY_CLASSES_ROOT, HKEY_CURRENT_CONFIG, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, HKEY_USERS, VALUE_BINARY, VALUE_DWORD, VALUE_EXPAND_STRING, VALUE_QWORD, VALUE_STRING, VALUE_STRING_LIST};

#[inline]
pub fn delete_key<'a>(key: Key, subkey: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    RegDeleteKey(key, subkey, 0)
}
#[inline]
pub fn delete_value<'a>(key: Key, value: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    RegDeleteKeyValue(key, value)
}
#[inline]
pub fn delete_key_tree<'a>(key: Key, subkey: impl Into<WCharLike<'a>>) -> Win32Result<()> {
    RegDeleteTree(key, subkey)
}
#[inline]
pub fn open_key<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, access: u32) -> Win32Result<OwnedKey> {
    RegOpenKeyEx(key, subkey, 0, access).map(|v| v.into())
}
#[inline]
pub fn create_key<'a>(key: Key, subkey: impl Into<WCharLike<'a>>, access: u32) -> Win32Result<(OwnedKey, bool)> {
    RegCreateKeyEx(key, subkey, WCharLike::Null, 0, access, None).map(|v| (v.0.into(), v.1))
}
