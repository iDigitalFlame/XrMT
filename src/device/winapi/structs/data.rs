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

use core::mem::{size_of, transmute};
use core::ptr;

use crate::device::winapi::{Handle, SecAttrs, SecQoS, SecurityDescriptor, SecurityQualityOfService, UnicodeString};
use crate::prelude::*;

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
pub struct DiskGeometry {
    pub cylinders:           u64,
    pub media_type:          u32,
    pub tracks_per_cylinder: u32,
    pub sectors_per_track:   u32,
    pub bytes_per_sector:    u32,
    pub size:                u64,
    pad:                     usize,
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
pub struct FileAllInformation {
    pub basic:          FileBasicInformation,
    pub standard:       FileStandardInformation,
    pub file_id:        u64,
    pub ea_size:        u32,
    pub access:         u32,
    pub current_offset: u64,
    pub mode:           u32,
    pub alignment:      u32,
    pub name_length:    u32,
    pub name:           [u16; 256],
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
pub struct FileBasicInformation {
    pub creation_time:    i64,
    pub last_access_time: i64,
    pub last_write_time:  i64,
    pub change_time:      i64,
    pub attributes:       u32,
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
#[repr(C)]
pub struct FileStandardInformation {
    pub allocation_size: u64,
    pub end_of_file:     u64,
    pub number_of_links: u32,
    pub delete_pending:  u32,
    pub is_directory:    u32,
}

pub type OverlappedIo<'a> = Option<&'a mut Overlapped>;

impl FileTime {
    #[inline]
    pub fn as_unix(&self) -> u64 {
        unsafe { transmute::<FileTime, u64>(*self) }
            .reverse_bits()
            .saturating_div(100)
    }
}
impl ObjectAttributes {
    #[inline]
    pub fn new(name: Option<&UnicodeString>, inherit: bool, attrs: u32, sa: SecAttrs, qos: SecQoS) -> ObjectAttributes {
        ObjectAttributes::root(name, Handle::INVALID, inherit, attrs, sa, qos)
    }
    pub fn root(name: Option<&UnicodeString>, root: Handle, inherit: bool, attrs: u32, sa: SecAttrs, qos: SecQoS) -> ObjectAttributes {
        let mut o = ObjectAttributes {
            length:              size_of::<ObjectAttributes>() as u32,
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
        FileTime { low: 0u32, high: 0u32 }
    }
}
impl From<u64> for FileTime {
    #[inline]
    fn from(v: u64) -> FileTime {
        unsafe { transmute(v.reverse_bits()) }
    }
}

impl Default for Overlapped {
    #[inline]
    fn default() -> Overlapped {
        Overlapped {
            event:         Handle::INVALID,
            internal:      0usize,
            internal_high: 0usize,
            offset:        0u32,
            offset_high:   0u32,
        }
    }
}
impl Default for DiskGeometry {
    #[inline]
    fn default() -> DiskGeometry {
        DiskGeometry {
            pad:                 0usize,
            size:                0u64,
            cylinders:           0u64,
            media_type:          0u32,
            bytes_per_sector:    0u32,
            sectors_per_track:   0u32,
            tracks_per_cylinder: 0u32,
        }
    }
}
impl Default for ObjectAttributes {
    #[inline]
    fn default() -> ObjectAttributes {
        ObjectAttributes {
            length:              size_of::<ObjectAttributes>() as u32,
            attributes:          0u32,
            object_name:         ptr::null_mut(),
            security_qos:        ptr::null_mut(),
            root_directory:      Handle::default(),
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl Default for FileAllInformation {
    #[inline]
    fn default() -> FileAllInformation {
        FileAllInformation {
            mode:           0u32,
            name:           [0u16; 256],
            basic:          FileBasicInformation::default(),
            access:         0u32,
            file_id:        0u64,
            ea_size:        0u32,
            standard:       FileStandardInformation::default(),
            alignment:      0u32,
            name_length:    0u32,
            current_offset: 0u64,
        }
    }
}
impl Default for FileStatInformation {
    #[inline]
    fn default() -> FileStatInformation {
        FileStatInformation {
            access:           0u32,
            file_id:          0u64,
            attributes:       0u32,
            change_time:      0i64,
            end_of_file:      0u64,
            reparse_tag:      0u32,
            creation_time:    0i64,
            last_write_time:  0i64,
            allocation_size:  0u64,
            number_of_links:  0u32,
            last_access_time: 0i64,
        }
    }
}
impl Default for FileBasicInformation {
    #[inline]
    fn default() -> FileBasicInformation {
        FileBasicInformation {
            attributes:       0u32,
            change_time:      0i64,
            creation_time:    0i64,
            last_write_time:  0i64,
            last_access_time: 0i64,
        }
    }
}
impl Default for ObjectBasicInformation {
    #[inline]
    fn default() -> ObjectBasicInformation {
        ObjectBasicInformation {
            pad:            [0u32; 3],
            access:         0u32,
            handles:        0u32,
            created:        0u64,
            pointers:       0u32,
            name_size:      0u32,
            type_size:      0u32,
            paged_pool:     0u32,
            attributes:     0u32,
            sec_desc_size:  0u32,
            non_paged_pool: 0u32,
        }
    }
}
impl Default for FileStandardInformation {
    #[inline]
    fn default() -> FileStandardInformation {
        FileStandardInformation {
            end_of_file:     0u64,
            is_directory:    0u32,
            delete_pending:  0u32,
            allocation_size: 0u64,
            number_of_links: 0u32,
        }
    }
}

pub(crate) unsafe extern "stdcall" fn _copy_file_ex(_z: u64, _t: u64, _c: u64, n: u64, i: u32, _r: u32, _s: usize, _d: usize, d: *mut usize) -> u32 {
    if i == 1 {
        *(d as *mut u64) = n;
    }
    0
}
