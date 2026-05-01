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

extern crate xrmt_time;

use core::clone::Clone;
use core::convert::From;
use core::default::Default;
use core::iter::Iterator;
use core::marker::Copy;
use core::ops::{Deref, DerefMut};
use core::option::Option;
use core::option::Option::{None, Some};

use xrmt_time::Time;

use crate::functions::{time_to_windows_time, GetCurrentProcessPEB};
use crate::structs::WCharSlice;

pub const WIN_TIME_EPOCH: i64 = 0x19DB1DED53E8000;

const KERNEL_SHARED_DATA: usize = 0x7FFE0000usize;

#[repr(C)]
pub struct SystemTime {
    pub low:  u32,
    pub high: i32,
    pad:      i32,
}
#[repr(transparent)]
pub struct SysTime(i64);
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
pub struct SystemVersion {
    pub sp:    u8,
    pub csd:   u8,
    pub major: u8,
    pub minor: u8,
    pub build: u16,
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
    pad1_1:                    u32,
    pub tick_count_multiplier: u32,
    pad1:                      [u8; 12],
    pub system_time:           SystemTime,
    pub time_zone_bias:        SystemTime,
    pub image_low:             u16,
    pub image_high:            u16,
    pub system_root:           [u16; 260],
    pub max_stack_trace:       u32,
    pad2:                      u32,
    pub time_zone_id:          u32,
    pub large_page_min:        u32,
    pad3:                      u32,
    pub app_compat_flag:       u32,
    pub rng_seed_ver:          u64,
    pub global_val_runlevel:   u32,
    pub time_zone_bias_stamp:  u32,
    pub build_number:          u32,
    pub product_type:          u32,
    pad4:                      u16,
    pub native_proc_arch:      u16,
    pub major_version:         u32,
    pub minor_version:         u32,
    pub processor_features:    [u8; 64],
    pad5:                      u64,
    pub time_slip:             u32,
    pad6:                      u32,
    pub expiration_date:       u64,
    pub suite_mask:            u32,
    pub debugger_enabled:      u8,
    pub mitigation_policies:   u8,
    pad7:                      u16,
    pub active_console_id:     u32,
    pad8:                      [u8; 12],
    pub physical_pages:        u32,
    pub safe_boot_mode:        u8,
    pub virtualization_flags:  u8,
    pad9:                      u16,
    pub shared_flags:          u32,
    pad10:                     u32,
    pad11:                     u64,
    pub system_call:           u32,
    pub system_call_ret:       u32,
    pub system_call_10:        u32,
    pub user_cert_flags:       u32,
    pad12:                     u64,
    pad13:                     u64,
    pub tick_count:            u64,
    pad14:                     u64,
    pub cookie:                u32,
}

impl SysTime {
    #[inline]
    pub const fn empty() -> SysTime {
        SysTime(0i64)
    }

    #[inline]
    pub fn as_time(&self) -> Time {
        Time::from_unix(0, self.as_unix_ns())
    }
    #[inline]
    pub fn as_unix_ns(&self) -> i64 {
        (self.0.saturating_sub(WIN_TIME_EPOCH)).saturating_mul(100)
    }
}
impl SystemTime {
    #[inline]
    pub fn as_i64(&self) -> i64 {
        unsafe { (self.high as i64).unchecked_shl(32) | self.low as i64 }
    }
    #[inline]
    pub fn as_unix_ns(&self) -> i64 {
        (self.as_i64().saturating_sub(WIN_TIME_EPOCH)).saturating_mul(100)
    }
}
impl SystemVersion {
    #[inline]
    pub fn get() -> SystemVersion {
        let p = GetCurrentProcessPEB();
        SystemVersion {
            sp:    unsafe { p.os_csd_version.unchecked_shr(8) as u8 },
            csd:   p.os_csd_version as u8,
            major: ((p.os_major_version as u8)
                // Add 1 to Windows 10, if the Build is >= 22000. Windows 11
                // still uses major version 10, but the build number for 11
                // starts at 22000. This includes Server builds!
                //
                // See https://en.wikipedia.org/wiki/List_of_Microsoft_Windows_versions
                .saturating_add(if p.os_major_version == 10 && p.os_build >= 22000 {
                    1
                } else {
                    0
                })),
            minor: p.os_minor_version as u8,
            build: p.os_build,
        }
    }
}
impl KernelUserShared {
    #[inline]
    pub const fn get<'a>() -> &'a KernelUserShared {
        unsafe { &*(KERNEL_SHARED_DATA as *const KernelUserShared) }
    }

    #[inline]
    pub fn kernel_time_offset(&self) -> i64 {
        self.system_time.as_i64().saturating_sub(self.time_zone_bias.as_i64())
    }
    #[inline]
    pub fn system_root<'a>(&self) -> WCharSlice<'a> {
        let b = &KernelUserShared::get().system_root;
        let r = match b.iter().position(|v| *v == 0) {
            Some(i) if i > 1 && unsafe { *b.get_unchecked(i - 1) == 0x5C } => i - 1,
            Some(i) => i,
            None => b.len(),
        };
        WCharSlice::from(unsafe { b.get_unchecked(0..r) })
    }
}

impl Copy for SysTime {}
impl Clone for SysTime {
    #[inline]
    fn clone(&self) -> SysTime {
        SysTime(self.0)
    }
}
impl Deref for SysTime {
    type Target = i64;

    #[inline]
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl Default for SysTime {
    #[inline]
    fn default() -> SysTime {
        SysTime::empty()
    }
}
impl DerefMut for SysTime {
    #[inline]
    fn deref_mut(&mut self) -> &mut i64 {
        &mut self.0
    }
}
impl From<i64> for SysTime {
    #[inline]
    fn from(v: i64) -> SysTime {
        SysTime(v)
    }
}
impl From<u64> for SysTime {
    #[inline]
    fn from(v: u64) -> SysTime {
        SysTime(v as i64)
    }
}
impl From<Time> for SysTime {
    #[inline]
    fn from(v: Time) -> SysTime {
        SysTime(time_to_windows_time(v))
    }
}
impl From<Option<Time>> for SysTime {
    #[inline]
    fn from(v: Option<Time>) -> SysTime {
        SysTime(v.map_or(0, time_to_windows_time))
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

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, Result};

    use crate::structs::{SysTime, SystemVersion};

    impl Debug for SysTime {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
    }
    impl Display for SysTime {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(&self.0, f)
        }
    }

    impl Debug for SystemVersion {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("SystemVersion")
                .field("service_pack", &self.sp)
                .field("csd", &self.csd)
                .field("major", &self.major)
                .field("minor", &self.minor)
                .field("build", &self.build)
                .finish()
        }
    }
}
