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

extern crate core;

use core::cmp::{Ord, Ordering};
use core::marker::Copy;

pub mod crypt;
pub mod log;
mod numbers;

pub use self::numbers::*;

#[inline]
pub fn copy<T: Copy>(dst: &mut [T], src: &[T]) -> usize {
    if src.is_empty() || dst.is_empty() {
        return 0;
    }
    let (j, k) = (dst.len(), src.len());
    match j.cmp(&k) {
        Ordering::Equal => {
            dst.copy_from_slice(src);
            j
        },
        Ordering::Less => {
            dst.copy_from_slice(&src[0..j]);
            j
        },
        Ordering::Greater => {
            dst[0..k].copy_from_slice(src);
            k
        },
    }
}
