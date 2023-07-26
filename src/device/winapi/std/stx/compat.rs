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

//
// Module assistance from the compiler-builtins crate
//  https://github.com/rust-lang/compiler-builtins
//
// Also help from the Windows builtins repo
// https://github.com/MauriceKayser/rs-windows-builtins
// https://skanthak.homepage.t-online.de/nomsvcrt.html
//

#![no_implicit_prelude]
#![cfg(not(feature = "std"))]

extern crate core;

use core::intrinsics::likely;
use core::{mem, ptr};

const SIZE: usize = mem::size_of::<usize>();
const MASK: usize = SIZE - 1;
const THRESHOLD: usize = if 2 * SIZE > 16 { 2 * SIZE } else { 16 };

#[inline(always)]
pub(super) unsafe fn strlen(mut s: *const u8) -> usize {
    let mut n = 0;
    while *s != 0 {
        n += 1;
        s = s.add(1);
    }
    n
}
#[inline(always)]
pub(super) unsafe fn set(mut s: *mut u8, c: u8, mut n: usize) {
    if likely(n >= THRESHOLD) {
        let m = (s as usize).wrapping_neg() & MASK;
        set_bytes(s, c, m);
        s = s.add(m);
        n -= m;
        let v = n & !MASK;
        set_words(s, c, v);
        s = s.add(v);
        n -= v;
    }
    set_bytes(s, c, n);
}
#[inline(always)]
pub(super) unsafe fn compare(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
    }
    0
}
#[inline(always)]
pub(super) unsafe fn copy_backward(dest: *mut u8, src: *const u8, mut n: usize) {
    let (mut d, mut s) = (dest.add(n), src.add(n));
    if n >= THRESHOLD {
        let m = d as usize & MASK;
        backward_bytes(d, s, m);
        d = d.sub(m);
        s = s.sub(m);
        n -= m;
        let v = n & !MASK;
        if likely((s as usize & MASK) == 0) {
            backward_align_words(d, s, v);
        } else {
            backward_words(d, s, v);
        }
        d = d.sub(v);
        s = s.sub(v);
        n -= v;
    }
    backward_bytes(d, s, n);
}
#[inline(always)]
pub(super) unsafe fn copy_forward(mut dest: *mut u8, mut src: *const u8, mut n: usize) {
    if n >= THRESHOLD {
        let m = (dest as usize).wrapping_neg() & MASK;
        forward_bytes(dest, src, m);
        dest = dest.add(m);
        src = src.add(m);
        n -= m;
        let v = n & !MASK;
        if likely((src as usize & MASK) == 0) {
            forward_align_words(dest, src, v);
        } else {
            forward_words(dest, src, v);
        }
        dest = dest.add(v);
        src = src.add(v);
        n -= v;
    }
    forward_bytes(dest, src, n);
}

#[inline(always)]
unsafe fn set_words(s: *mut u8, c: u8, n: usize) {
    let (mut b, mut x) = (c as usize, 8);
    while x < SIZE * 8 {
        b |= b << x;
        x *= 2;
    }
    let mut v = s as *mut usize;
    let e = s.add(n) as *mut usize;
    while v < e {
        *v = b;
        v = v.add(1);
    }
}
#[inline(always)]
unsafe fn set_bytes(mut s: *mut u8, c: u8, n: usize) {
    let e = s.add(n);
    while s < e {
        *s = c;
        s = s.add(1);
    }
}
#[inline(always)]
unsafe fn forward_words(dest: *mut u8, src: *const u8, n: usize) {
    let mut d = dest as *mut usize;
    let e = dest.add(n) as *mut usize;
    let s = (src as usize & MASK) * 8;
    let mut a = (src as usize & !MASK) as *mut usize;
    let mut p = ptr::read_volatile(a);
    while d < e {
        a = a.add(1);
        let c = *a;
        let r = if cfg!(target_endian = "little") {
            p >> s | c << (SIZE * 8 - s)
        } else {
            p << s | c >> (SIZE * 8 - s)
        };
        p = c;
        *d = r;
        d = d.add(1);
    }
}
#[inline(always)]
unsafe fn backward_words(dest: *mut u8, src: *const u8, n: usize) {
    let mut d = dest as *mut usize;
    let x = dest.sub(n) as *mut usize;
    let s = (src as usize & MASK) * 8;
    let mut a = (src as usize & !MASK) as *mut usize;
    let mut p = ptr::read_volatile(a);
    while x < d {
        a = a.sub(1);
        let c = *a;
        let r = if cfg!(target_endian = "little") {
            p << (SIZE * 8 - s) | c >> s
        } else {
            p >> (SIZE * 8 - s) | c << s
        };
        p = c;
        d = d.sub(1);
        *d = r;
    }
}
#[inline(always)]
unsafe fn forward_align_words(dest: *mut u8, src: *const u8, n: usize) {
    let (mut d, mut s) = (dest as *mut usize, src as *mut usize);
    let e = dest.add(n) as *mut usize;
    while d < e {
        *d = *s;
        d = d.add(1);
        s = s.add(1);
    }
}
#[inline(always)]
unsafe fn backward_align_words(dest: *mut u8, src: *const u8, n: usize) {
    let (mut d, mut s) = (dest as *mut usize, src as *mut usize);
    let x = dest.sub(n) as *mut usize;
    while x < d {
        d = d.sub(1);
        s = s.sub(1);
        *d = *s;
    }
}
#[inline(always)]
unsafe fn forward_bytes(mut dest: *mut u8, mut src: *const u8, n: usize) {
    let e = dest.add(n);
    while dest < e {
        *dest = *src;
        dest = dest.add(1);
        src = src.add(1);
    }
}
#[inline(always)]
unsafe fn backward_bytes(mut dest: *mut u8, mut src: *const u8, n: usize) {
    let s = dest.sub(n);
    while s < dest {
        dest = dest.sub(1);
        src = src.sub(1);
        *dest = *src;
    }
}
