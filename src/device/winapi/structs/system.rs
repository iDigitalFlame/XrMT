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

use crate::data::blob::Slice;
use crate::device::winapi;
use crate::prelude::*;

pub const WIN_TIME_EPOCH: i64 = 0x19DB1DED53E8000;

const KERNEL_SHARED_DATA: usize = 0x7FFE0000;

#[repr(C)]
pub struct SystemTime {
    pub low:  u32,
    pub high: i32,
    pad:      i32,
}
#[repr(C)]
pub struct TimeZoneInfo {
    pub bias:          u32,
    pub standard_name: [u16; 32],
    pad1:              [u8; 16],
    pub standard_bias: u32,
    pub daylight_name: [u16; 32],
    pad2:              [u8; 16],
    pub daylight_bias: u32,
}
#[repr(C)]
pub struct OsVersionInfo {
    pub ver_info_size: u32,
    pub major:         u32,
    pub minor:         u32,
    pub build:         u32,
    pub platform:      u32,
    pub sp_name:       [u16; 128],
    pub sp_major:      u16,
    pub sp_minor:      u16,
    pub mask:          u16,
    pub product:       u8,
    pad:               u8,
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
    pub time_zone_bias:       SystemTime,
    pad2:                     u32,
    pub system_root:          [u16; 260],
    pub max_stack_trace:      u32,
    pad3:                     u32,
    pub time_zone_id:         u32,
    pub large_page_min:       u32,
    pad4:                     u32,
    pub app_compat_flag:      u32,
    pub rng_seed_ver:         u64,
    pub global_val_runlevel:  u32,
    pub time_zone_bias_stamp: u32,
    pub build_number:         u32,
    pub nt_product_type:      u32,
    pad5:                     [u8; 2],
    pub native_proc_arch:     u16,
    pub major_version:        u32,
    pub minor_version:        u32,
    pub processor_features:   [u8; 64],
    pad6:                     [u8; 8],
    pub time_slip:            u32,
    pad7:                     [u8; 8],
    pub expiration_date:      u64,
    pub suite_mask:           u32,
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

impl SystemTime {
    #[inline]
    pub fn as_i64(&self) -> i64 {
        ((self.high as i64) << 32) | self.low as i64
    }
    #[inline]
    pub fn as_unix_ns(&self) -> i64 {
        (self.as_i64() - WIN_TIME_EPOCH) * 100
    }
}

impl Default for TimeZoneInfo {
    #[inline]
    fn default() -> TimeZoneInfo {
        TimeZoneInfo {
            pad1:          [0u8; 16],
            pad2:          [0u8; 16],
            bias:          0u32,
            standard_name: [0u16; 32],
            standard_bias: 0u32,
            daylight_name: [0u16; 32],
            daylight_bias: 0u32,
        }
    }
}
impl Default for OsVersionInfo {
    #[inline]
    fn default() -> OsVersionInfo {
        OsVersionInfo {
            pad:           0u8,
            mask:          0u16,
            major:         0u32,
            minor:         0u32,
            build:         0u32,
            sp_name:       [0u16; 128],
            product:       0u8,
            platform:      032,
            sp_major:      0u16,
            sp_minor:      0u16,
            ver_info_size: 0x11C,
        }
    }
}
impl Default for SystemBasicInfo {
    #[inline]
    fn default() -> SystemBasicInfo {
        SystemBasicInfo {
            pad:                       0u32,
            page_size:                 0u32,
            affinity_mask:             0usize,
            time_resolution:           0u32,
            min_user_address:          0usize,
            max_user_address:          0usize,
            number_of_processors:      0u8,
            number_physical_pages:     0u32,
            allocation_granularity:    0u32,
            lowest_physical_page_num:  0u32,
            highest_physical_page_num: 0u32,
        }
    }
}

#[inline]
pub fn kernel_time_offset() -> i64 {
    let s = kernel_user_shared();
    s.system_time.as_i64() - s.time_zone_bias.as_i64()
}
#[inline]
pub fn kernel_nano_sec_time() -> i64 {
    kernel_user_shared().system_time.as_unix_ns()
}
pub fn system_dir() -> Slice<u16, 270> {
    let mut b: Slice<u16, 270> = winapi::kernel_user_shared().system_root.into();
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
    let r = kernel_user_shared().system_root;
    let mut b = Slice::with_len(r.iter().position(|v| *v == 0).unwrap_or(260));
    for i in 0..b.len {
        b[i] = r[i] as u8
    }
    b
}
#[inline]
pub fn kernel_nano_sec_local_time() -> i64 {
    (kernel_time_offset() - winapi::WIN_TIME_EPOCH) * 100
}
#[inline]
pub fn kernel_user_shared<'a>() -> &'a KernelUserShared {
    unsafe { &*(KERNEL_SHARED_DATA as *const KernelUserShared) }
}
