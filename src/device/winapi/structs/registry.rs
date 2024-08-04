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
#![cfg(target_family = "windows")]

use crate::data::blob::Blob;
use crate::device::winapi::{self, FileTime};
use crate::prelude::*;

#[repr(C)]
pub struct RegKeyInfo {
    pub subkey_count:       u32,
    pub max_subkey_len:     u32,
    pub max_class_len:      u32,
    pub value_count:        u32,
    pub max_value_name_len: u32,
    pub max_value_len:      u32,
    pub last_write:         FileTime,
}
#[repr(C)]
pub struct RegKeyFullInfo {
    pub last_write:         u64,
    pub index:              u32,
    pub class_offset:       u32,
    pub class_length:       u32,
    pub subkeys:            u32,
    pub max_name_len:       u32,
    pub max_class_len:      u32,
    pub values:             u32,
    pub max_value_name_len: u32,
    pub max_value_len:      u32,
    pub class_name:         [u16; 261],
}
#[repr(C)]
pub struct RegKeyBasicInfo {
    pub last_write:  u64,
    pub index:       u32,
    pub name_length: u32,
    pub name:        [u16; 256],
}
#[repr(C)]
pub struct RegKeyValueFullInfo {
    pub index:       u32,
    pub value_type:  u32,
    pub data_offset: u32,
    pub data_length: u32,
    pub name_length: u32,
    pub name:        [u16; 261],
}

impl RegKeyValueFullInfo {
    #[inline]
    pub fn data<'a>(&self, buf: &'a Blob<u8, 256>) -> &'a [u8] {
        &buf[self.data_offset as usize..(self.data_offset + self.data_length) as usize]
    }
}

impl Default for RegKeyInfo {
    #[inline]
    fn default() -> RegKeyInfo {
        RegKeyInfo {
            last_write:         FileTime::default(),
            value_count:        0u32,
            subkey_count:       0u32,
            max_value_len:      0u32,
            max_class_len:      0u32,
            max_subkey_len:     0u32,
            max_value_name_len: 0u32,
        }
    }
}
impl Default for RegKeyFullInfo {
    #[inline]
    fn default() -> RegKeyFullInfo {
        RegKeyFullInfo {
            index:              0u32,
            values:             0u32,
            subkeys:            0u32,
            last_write:         0u64,
            class_name:         [0u16; 261],
            class_offset:       0u32,
            class_length:       0u32,
            max_name_len:       0u32,
            max_class_len:      0u32,
            max_value_len:      0u32,
            max_value_name_len: 0u32,
        }
    }
}

impl Default for RegKeyBasicInfo {
    #[inline]
    fn default() -> RegKeyBasicInfo {
        RegKeyBasicInfo {
            name:        [0u16; 256],
            index:       0u32,
            last_write:  0u64,
            name_length: 0u32,
        }
    }
}
impl ToString for RegKeyBasicInfo {
    #[inline]
    fn to_string(&self) -> String {
        winapi::utf16_to_str_trim(&self.name[0..self.name_length as usize / 2])
    }
}

impl Copy for RegKeyValueFullInfo {}
impl Clone for RegKeyValueFullInfo {
    #[inline]
    fn clone(&self) -> RegKeyValueFullInfo {
        RegKeyValueFullInfo {
            name:        self.name,
            index:       self.index,
            value_type:  self.value_type,
            data_offset: self.data_offset,
            data_length: self.data_length,
            name_length: self.name_length,
        }
    }
}
impl Default for RegKeyValueFullInfo {
    #[inline]
    fn default() -> RegKeyValueFullInfo {
        RegKeyValueFullInfo {
            name:        [0u16; 261],
            index:       0u32,
            value_type:  0u32,
            data_offset: 0u32,
            data_length: 0u32,
            name_length: 0u32,
        }
    }
}
impl ToString for RegKeyValueFullInfo {
    #[inline]
    fn to_string(&self) -> String {
        winapi::utf16_to_str_trim(&self.name[0..self.name_length as usize / 2])
    }
}

impl From<RegKeyFullInfo> for RegKeyInfo {
    #[inline]
    fn from(v: RegKeyFullInfo) -> RegKeyInfo {
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
