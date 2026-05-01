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

extern crate core;

extern crate xrmt_data;

use core::cmp::Ord;
#[cfg(target_family = "windows")]
use core::iter::Iterator;
use core::marker::Copy;

#[cfg(target_family = "windows")]
use xrmt_data::text::hex;

#[cfg(target_family = "windows")]
#[inline]
pub fn write_hex(b: &mut [u8], v: u8) {
    match v {
        0 => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (0x30, 0x30) },
        1..=0xF => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (0x30, hex(v)) },
        _ => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (hex(v.unchecked_shr(4)), hex(v)) },
    }
}
#[cfg(target_family = "windows")]
#[inline]
pub fn write_hex_u16(b: &mut [u16], v: u8) {
    match v {
        0 => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (0x30, 0x30) },
        1..=0xF => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (0x30, hex(v) as u16) },
        _ => unsafe { (*b.get_unchecked_mut(0), *b.get_unchecked_mut(1)) = (hex(v.unchecked_shr(4)) as u16, hex(v) as u16) },
    }
}
#[cfg(target_family = "windows")]
pub fn write_u32_u16(b: &mut [u16], v: u32) -> usize {
    if b.len() < 1 {
        return 0;
    }
    let n = match v {
        0 => {
            unsafe { *b.get_unchecked_mut(0) = 0x30 }; // 0
            return 1;
        },
        1__________..=9__________ => 1,
        1_________0..=9_________9 => 2,
        1________00..=9________99 => 3,
        1_______000..=9_______999 => 4,
        1_____0_000..=9_____9_999 => 5,
        1____00_000..=9____99_999 => 6,
        1___000_000..=9___999_999 => 7,
        1_0_000_000..=9_9_999_999 => 8,
        100_000_000..=999_999_999 => 9,
        _ => 10,
    };
    if b.len() < n {
        return 0;
    }
    let mut r = v;
    // Work backwards to write number.
    for i in (1..n).rev() {
        let t = r / 0xA;
        unsafe { *b.get_unchecked_mut(i) = 0x30 + (r - (t * 0xA)) as u16 };
        r = t;
        if r < 0xA {
            break;
        }
    }
    unsafe { *b.get_unchecked_mut(0) = 0x30 + r as u16 };
    n
}
#[inline]
pub fn copy<T: Copy>(src: &[T], dest: &mut [T]) -> usize {
    if src.is_empty() || dest.is_empty() {
        return 0;
    }
    let n = src.len().min(dest.len());
    unsafe { dest.get_unchecked_mut(0..n).copy_from_slice(src.get_unchecked(0..n)) };
    n
}
#[cfg(target_family = "windows")]
pub fn write_hex_padded(b: &mut [u16], pad: usize, r: u32) -> usize {
    let n = match r {
        0 => 0,
        0x_______1..=0x_______F => 1,
        0x______10..=0x______FF => 2,
        0x_____100..=0x_____FFF => 3,
        0x____1000..=0x____FFFF => 4,
        0x___10000..=0x___FFFFF => 5,
        0x__100000..=0x__FFFFFF => 6,
        0x_1000000..=0x_FFFFFFF => 7,
        0x10000000..=0xFFFFFFFF => 8,
    };
    let mut x = 0;
    while pad > n + x {
        // 'pad' is always less than 'b.len()'
        unsafe { *b.get_unchecked_mut(x) = 0x30 };
        x += 1;
    }
    let o = 8usize.saturating_sub(n);
    for i in o..n + o {
        unsafe {
            let v = hex(r.unchecked_shr((4 * (7 - i)) as u32) as u8) as u16;
            *b.get_unchecked_mut(x) = if v > 0x39 && v < 0x61 {
                v + 0x20 // Make lowercase.
            } else {
                v
            }
        };
        x += 1;
    }
    if pad > n {
        pad
    } else {
        n
    }
}
