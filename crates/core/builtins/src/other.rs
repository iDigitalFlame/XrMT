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

//
// Module assistance from the compiler-builtins crate
//  https://github.com/rust-lang/compiler-builtins
//
// Also help from the Windows builtins repo
//  https://github.com/MauriceKayser/rs-windows-builtins
//  https://skanthak.homepage.t-online.de/nomsvcrt.html
//

#![no_implicit_prelude]
#![cfg(all(not(feature = "std"), not(target_os = "none"), not(target_arch = "x86_64")))]

extern crate core;

use core::intrinsics::likely;
use core::mem::size_of;
use core::ptr::read_volatile;

const SIZE: usize = size_of::<usize>();
const MASK: usize = SIZE - 1;
const THRESHOLD: usize = if SIZE * 0x2 > 0x10 { 2 * SIZE } else { 0x10 };

#[inline]
pub fn strlen(x: *const u8) -> usize {
    let (mut i, mut v) = (0usize, x);
    unsafe {
        while *v != 0 {
            (i, v) = (i + 1, v.add(1));
        }
    }
    i
}
#[inline]
pub fn set(x: *mut u8, c: u8, n: usize) {
    let (mut i, mut v) = (n, x);
    if likely(n >= THRESHOLD) {
        let a = (v as usize).wrapping_neg() & MASK;
        bytes(v, c, a);
        (i, v) = unsafe { (i - a, v.add(a)) };
        let b = n & !MASK;
        words(v, c, b);
        (i, v) = unsafe { (i - b, v.add(b)) };
    }
    bytes(v, c, i);
}
#[inline]
pub fn copy_forward(x: *mut u8, y: *const u8, n: usize) {
    let (mut i, mut d, mut s) = (n, x, y);
    if n >= THRESHOLD {
        let a = (d as usize).wrapping_neg() & MASK;
        forward_bytes(d, s, a);
        (i, d, s) = unsafe { (i - a, d.add(a), s.add(a)) };
        let b = n & !MASK;
        if likely((s as usize & MASK) == 0) {
            forward_align(d, s, b);
        } else {
            forward(d, s, b);
        }
        (i, d, s) = unsafe { (i - b, d.add(b), s.add(b)) };
    }
    forward_bytes(d, s, i);
}
#[inline]
pub fn copy_backward(x: *mut u8, y: *const u8, n: usize) {
    let (mut i, mut d, mut s) = unsafe { (n, x.add(n), y.add(n)) };
    if n >= THRESHOLD {
        let a = d as usize & MASK;
        backward_bytes(d, s, a);
        (i, d, s) = unsafe { (i - a, d.sub(a), s.sub(a)) };
        let b = i & !MASK;
        if likely((s as usize & MASK) == 0) {
            backward_align(d, s, b);
        } else {
            backward(d, s, b);
        }
        (i, d, s) = unsafe { (i - b, d.sub(b), s.sub(b)) };
    }
    backward_bytes(d, s, i);
}
#[inline]
pub fn compare(x: *const u8, y: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let (a, b) = unsafe { (*x.add(i), *y.add(i)) };
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
    }
    0
}

#[inline]
fn bytes(x: *mut u8, c: u8, n: usize) {
    let (i, mut v) = unsafe { (x.add(n), x) };
    while v < i {
        unsafe { (*v, v) = (c, v.add(1)) };
    }
}
#[inline]
fn words(s: *mut u8, c: u8, n: usize) {
    let (mut v, mut i) = (c as usize, 8usize);
    while i < SIZE * 8 {
        (v, i) = unsafe { (v | v.unchecked_shl(i as u32), i * 0x2) };
    }
    let (mut a, b) = unsafe { (s as *mut usize, s.add(n) as *mut usize) };
    while a < b {
        unsafe { (*a, a) = (v, a.add(1)) };
    }
}
#[inline]
fn forward(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, a) = unsafe { (x as *mut usize, x.add(n) as *mut usize) };
    let (mut s, b) = (
        (y as usize & !MASK) as *mut usize,
        (y as usize & MASK) * 0x8,
    );
    let mut v = unsafe { read_volatile(s) };
    while d < a {
        unsafe {
            s = s.add(1);
            let i = *s;
            let r = if cfg!(target_endian = "little") {
                v.unchecked_shr(b as u32) | i.unchecked_shl(((SIZE * 0x8) - b) as u32)
            } else {
                v.unchecked_shl(b as u32) | i.unchecked_shr(((SIZE * 0x8) - b) as u32)
            };
            (v, *d, d) = (i, r, d.add(1));
        }
    }
}
#[inline]
fn backward(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, a) = unsafe { (x as *mut usize, x.sub(n) as *mut usize) };
    let (mut s, b) = (
        (y as usize & !MASK) as *mut usize,
        (y as usize & MASK) * 0x8,
    );
    let mut v = unsafe { read_volatile(s) };
    while a < d {
        unsafe {
            s = s.sub(1);
            let i = *s;
            let r = if cfg!(target_endian = "little") {
                v.unchecked_shl(((SIZE * 0x8) - b) as u32) | i.unchecked_shr(b as u32)
            } else {
                v.unchecked_shr(((SIZE * 0x8) - b) as u32) | i.unchecked_shl(b as u32)
            };
            (v, d) = (i, d.sub(1));
            *d = r;
        }
    }
}
#[inline]
fn forward_align(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, mut s, a) = unsafe { (x as *mut usize, y as *mut usize, x.add(n) as *mut usize) };
    while d < a {
        unsafe {
            *d = *s;
            (d, s) = (d.add(1), s.add(1));
        }
    }
}
#[inline]
fn forward_bytes(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, mut s, a) = unsafe { (x, y, x.add(n)) };
    while d < a {
        unsafe {
            *d = *s;
            (d, s) = (d.add(1), s.add(1));
        }
    }
}
#[inline]
fn backward_align(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, mut s, a) = unsafe { (x as *mut usize, y as *mut usize, x.sub(n) as *mut usize) };
    while a < d {
        unsafe {
            (d, s) = (d.sub(1), s.sub(1));
            *d = *s;
        }
    }
}
#[inline]
fn backward_bytes(x: *mut u8, y: *const u8, n: usize) {
    let (mut d, mut s, a) = unsafe { (x, y, x.sub(n)) };
    while a < d {
        unsafe {
            (d, s) = (d.add(1), s.add(1));
            *d = *s;
        }
    }
}
