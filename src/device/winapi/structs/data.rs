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

use core::{mem, ptr};

use crate::device::winapi::{self, Handle, SecAttrs, SecQoS, SecurityDescriptor, SecurityQualityOfService, UnicodeString};
use crate::util::stx::prelude::*;

#[repr(C)]
pub struct FileTime {
    pub low:  u32,
    pub high: u32,
}
#[repr(C)]
pub struct Overlapped {
    pub internal:      usize,
    pub internal_high: usize,
    pub offset:        u32,
    pub offset_high:   u32,
    pub event:         Handle,
}
#[repr(C)]
pub struct FileBasicInfo {
    pub creation_time:    i64,
    pub last_access_time: i64,
    pub last_write_time:  i64,
    pub change_time:      i64,
    pub attributes:       u32,
}
#[repr(C)]
pub struct ObjectAttributes {
    pub length:              u32,
    pub root_directory:      Handle,
    pub object_name:         *const UnicodeString,
    pub attributes:          u32,
    pub security_descriptor: *mut SecurityDescriptor,
    pub security_qos:        *const SecurityQualityOfService,
}
#[repr(C)]
pub struct FileStandardInfo {
    pub allocation_size: u64,
    pub end_of_file:     u64,
    pub number_of_links: u32,
    pub delete_pending:  u32,
    pub is_directory:    u32,
}
#[repr(C)]
pub struct FileIdBothDirInfo {
    pub next_entry:        u32,
    pub file_index:        u32,
    pub creation_time:     i64,
    pub last_access_time:  i64,
    pub last_write_time:   i64,
    pub change_time:       i64,
    pub end_of_file:       u64,
    pub allocation_size:   u64,
    pub attributes:        u32,
    pub name_length:       u32,
    pub ea_size:           u32,
    pub short_name_length: u8,
    pub short_name:        [u16; 12],
    pub file_id:           u64,
    pub file_name:         [u16; 1],
}
#[repr(C)]
pub struct FileStatInformation {
    pub file_id:          u64,
    pub creation_time:    i64,
    pub last_access_time: i64,
    pub last_write_time:  i64,
    pub change_time:      i64,
    pub allocation_size:  u64,
    pub end_of_file:      u64,
    pub attributes:       u32,
    pub reparse_tag:      u32,
    pub number_of_links:  u32,
    pub access:           u32,
}
#[repr(C)]
pub struct ObjectBasicInformation {
    pub attributes:     u32,
    pub access:         u32,
    pub handles:        u32,
    pub pointers:       u32,
    pub paged_pool:     u32,
    pub non_paged_pool: u32,
    pad:                [u32; 3],
    pub name_size:      u32,
    pub type_size:      u32,
    pub sec_desc_size:  u32,
    pub created:        u64,
}

pub type OverlappedIo<'a> = Option<&'a mut Overlapped>;

impl FileTime {
    #[inline]
    pub fn as_nano(&self) -> u64 {
        unsafe { core::mem::transmute::<FileTime, u64>(*self) }
            .reverse_bits()
            .saturating_div(100)
    }
}
impl ObjectAttributes {
    #[inline]
    pub fn new(name: Option<&UnicodeString>, inherit: bool, attrs: u32, sa: SecAttrs, qos: SecQoS) -> ObjectAttributes {
        ObjectAttributes::root(name, winapi::INVALID, inherit, attrs, sa, qos)
    }
    pub fn root(name: Option<&UnicodeString>, root: Handle, inherit: bool, attrs: u32, sa: SecAttrs, qos: SecQoS) -> ObjectAttributes {
        let mut o = ObjectAttributes {
            length:              mem::size_of::<ObjectAttributes>() as u32,
            attributes:          attrs | if inherit { 0x2 } else { 0 }, // 0x2 - OBJ_INHERIT,
            object_name:         ptr::null_mut(),
            security_qos:        ptr::null_mut(),
            root_directory:      root,
            security_descriptor: ptr::null_mut(),
        };
        if let Some(v) = sa {
            o.security_descriptor = v.security_descriptor;
            if v.inherit == 1 {
                o.attributes |= 0x2; // 0x2 - OBJ_INHERIT
            }
        }
        if let Some(q) = qos {
            o.security_qos = q
        }
        if let Some(s) = name {
            o.object_name = s
        }
        o
    }
}

impl Copy for FileTime {}
impl Clone for FileTime {
    #[inline]
    fn clone(&self) -> FileTime {
        FileTime { low: self.low, high: self.high }
    }
}
impl Default for FileTime {
    #[inline]
    fn default() -> FileTime {
        FileTime { low: 0, high: 0 }
    }
}
impl From<u64> for FileTime {
    #[inline]
    fn from(v: u64) -> FileTime {
        unsafe { mem::transmute(v.reverse_bits()) }
    }
}

impl Default for Overlapped {
    #[inline]
    fn default() -> Overlapped {
        Overlapped {
            event:         Handle::default(),
            internal:      0,
            internal_high: 0,
            offset:        0,
            offset_high:   0,
        }
    }
}
impl Default for FileBasicInfo {
    #[inline]
    fn default() -> FileBasicInfo {
        FileBasicInfo {
            attributes:       0,
            change_time:      0,
            creation_time:    0,
            last_write_time:  0,
            last_access_time: 0,
        }
    }
}

impl Default for ObjectAttributes {
    #[inline]
    fn default() -> ObjectAttributes {
        ObjectAttributes {
            length:              mem::size_of::<ObjectAttributes>() as u32,
            attributes:          0,
            object_name:         ptr::null_mut(),
            security_qos:        ptr::null_mut(),
            root_directory:      Handle::default(),
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl Default for FileStandardInfo {
    #[inline]
    fn default() -> FileStandardInfo {
        FileStandardInfo {
            end_of_file:     0,
            is_directory:    0,
            delete_pending:  0,
            allocation_size: 0,
            number_of_links: 0,
        }
    }
}
impl Default for FileStatInformation {
    #[inline]
    fn default() -> FileStatInformation {
        FileStatInformation {
            access:           0,
            file_id:          0,
            attributes:       0,
            change_time:      0,
            end_of_file:      0,
            reparse_tag:      0,
            creation_time:    0,
            last_write_time:  0,
            allocation_size:  0,
            number_of_links:  0,
            last_access_time: 0,
        }
    }
}
impl Default for ObjectBasicInformation {
    #[inline]
    fn default() -> ObjectBasicInformation {
        ObjectBasicInformation {
            pad:            [0; 3],
            access:         0,
            handles:        0,
            created:        0,
            pointers:       0,
            name_size:      0,
            type_size:      0,
            paged_pool:     0,
            attributes:     0,
            sec_desc_size:  0,
            non_paged_pool: 0,
        }
    }
}
