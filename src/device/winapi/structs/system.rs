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

use crate::data::blob::Slice;
use crate::device::winapi;
use crate::util::stx::prelude::*;

pub const WIN_TIME_EPOCH: i64 = 0x19DB1DED53E8000;

const KERNEL_SHARED_DATA: usize = 0x7FFE0000;

#[repr(C)]
pub struct SystemTime {
    pub low:  u32,
    pub high: i32,
    pad:      i32,
}
#[repr(C)]
pub struct SystemBasicInfo {
    pad:                           u32,
    pub time_resolution:           u32,
    pub page_size:                 u32,
    pub number_physical_pages:     u32,
    pub lowest_physical_page_num:  u32,
    pub highest_physical_page_num: u32,
    pub allocation_granularity:    u32,
    pub min_user_address:          usize,
    pub max_user_address:          usize,
    pub affinity_mask:             usize,
    pub number_of_processors:      u8,
}
#[repr(C)]
pub struct KernelUserShared {
    pad1:                     [u8; 20],
    pub system_time:          SystemTime,
    pad2:                     [u8; 16],
    pub system_root:          [u16; 260],
    pub max_stack_trace:      u32,
    pad3:                     u32,
    pad4:                     [u8; 32],
    pub build_number:         u32,
    pad5:                     [u8; 8],
    pub major_version:        u32,
    pub minor_version:        u32,
    pub processor_features:   [u8; 64],
    pad6:                     [u8; 20],
    pub expiration_date:      u64,
    pad7:                     [u8; 4],
    pub ke_debugger_enabled:  u8,
    pub mitigation_policies:  u8,
    pad8:                     [u8; 2],
    pub active_console_id:    u32,
    pad9:                     [u8; 12],
    pub physical_pages:       u32,
    pub safe_boot_mode:       u8,
    pub virtualization_flags: u8,
    pad10:                    [u8; 2],
    pub shared_flags:         u32,
}

#[inline]
pub fn kernel_nano_time() -> i64 {
    let k = KERNEL_SHARED_DATA as *const KernelUserShared;
    unsafe { ((((*k).system_time.high as i64) << 32 | (*k).system_time.low as i64) - WIN_TIME_EPOCH) * 100 }
}
pub fn system_dir() -> Slice<u16, 270> {
    let mut b: Slice<u16, 270> = unsafe { (*winapi::kernel_user_shared()).system_root }.into();
    let p = match b.iter().position(|v| *v == 0) {
        Some(i) => {
            if b[i - 1] != b'\\' as u16 {
                b[i] = b'\\' as u16;
                i + 1
            } else {
                i
            }
        },
        None => return b,
    };
    b.len = p + 8;
    b.data[p] = b'S' as u16;
    b[p + 1] = b'y' as u16;
    b[p + 2] = b's' as u16;
    b[p + 3] = b't' as u16;
    b[p + 4] = b'e' as u16;
    b[p + 5] = b'm' as u16;
    b[p + 6] = b'3' as u16;
    b[p + 7] = b'2' as u16;
    b[p + 8] = 0;
    b
}
#[inline]
pub fn system_root() -> Slice<u8, 260> {
    let r = unsafe { &(*winapi::kernel_user_shared()).system_root };
    let mut b = Slice::with_len(r.iter().position(|v| *v == 0).unwrap_or(260));
    for i in 0..b.len {
        b[i] = r[i] as u8
    }
    b
}
#[inline]
pub fn kernel_user_shared() -> *const KernelUserShared {
    KERNEL_SHARED_DATA as *const KernelUserShared
}
