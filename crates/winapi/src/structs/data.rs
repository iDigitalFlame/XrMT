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

use core::clone::Clone;
use core::default::Default;

#[repr(C)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data:  [u8; 8],
}
#[repr(C)]
pub struct QuotaLimit {
    pub paged_pool_limit:     isize,
    pub non_paged_pool_limit: isize,
    pub min_working_set:      isize,
    pub max_working_set:      isize,
    pub page_file_limit:      isize,
    pub time_limit:           i64,
}
#[repr(C)]
pub struct PoolTagEntry {
    pub tag:          u32,
    pub paged_allocs: u32,
    pub paged_frees:  u32,
    pub paged_used:   usize,
    pub allocs:       u32,
    pub frees:        u32,
    pub used:         usize,
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

impl Clone for PoolTagEntry {
    #[inline]
    fn clone(&self) -> PoolTagEntry {
        PoolTagEntry {
            tag:          self.tag,
            used:         self.used,
            frees:        self.frees,
            allocs:       self.allocs,
            paged_used:   self.paged_used,
            paged_frees:  self.paged_frees,
            paged_allocs: self.paged_allocs,
        }
    }
}

impl Default for QuotaLimit {
    #[inline]
    fn default() -> QuotaLimit {
        QuotaLimit {
            time_limit:           0i64,
            min_working_set:      -1isize,
            max_working_set:      -1isize,
            page_file_limit:      0isize,
            paged_pool_limit:     0isize,
            non_paged_pool_limit: 0isize,
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
