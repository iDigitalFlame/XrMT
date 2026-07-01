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
#![cfg(all(not(feature = "std"), not(target_os = "none"), target_arch = "x86_64"))]

extern crate core;

use core::arch::asm;
use core::clone::Clone;
use core::cmp::{Eq, Ord};
use core::ffi::c_void;
use core::intrinsics::{unchecked_div, unchecked_rem};
use core::marker::Copy;
use core::mem::size_of;
use core::ops::FnOnce;

#[cfg(target_feature = "sse2")]
#[cfg_attr(rustfmt, rustfmt_skip)]
use core::arch::x86_64::{__m128i, _mm_cmpeq_epi8, _mm_movemask_epi8, _mm_set1_epi8};

#[cfg(target_feature = "sse2")]
pub fn strlen(x: *const i8) -> usize {
    let (mut n, mut s) = (0usize, x);
    for _ in 0..4 {
        if unsafe { *s } == 0 {
            return n;
        }
        (n, s) = unsafe { (n + 1, s.add(1)) };
    }
    let m = s as usize & 0xF;
    let (mut y, b) = unsafe { (((s as usize) - m) as *const __m128i, _mm_set1_epi8(0)) };
    let v = unsafe {
        let a;
        asm!(
            "movdqa {1}, [{0:r}]",
            in(reg) y,
            out(xmm_reg) a,
            options(nostack),
        );
        _mm_movemask_epi8(_mm_cmpeq_epi8(a, b)).unchecked_shr(m as u32)
    };
    if v != 0 {
        return n + v.trailing_zeros() as usize;
    }
    (n, y) = unsafe { (n + (0xF - m), y.add(1)) };
    loop {
        let i = unsafe {
            let a;
            asm!(
                "movdqa {1}, [{0:r}]",
                in(reg) y,
                out(xmm_reg) a,
                options(nostack),
            );
            _mm_movemask_epi8(_mm_cmpeq_epi8(a, b))
        } as u32;
        if i == 0 {
            (n, y) = unsafe { (n + 0xF, y.add(1)) };
        } else {
            break n + i.trailing_zeros() as usize;
        }
    }
}
#[cfg(not(target_feature = "sse2"))]
pub fn strlen(x: *const c_void) -> usize {
    let (mut n, mut s) = (0usize, x);
    while s as usize & 7 != 0 {
        if unsafe { *s } == 0 {
            return n;
        }
        (n, s) = unsafe { (n + 1, s.add(1)) };
    }
    let mut y = s as *const u64;
    loop {
        let mut v: u64;
        unsafe {
            asm!(
                "mov {1}, [{0:r}]",
                in(reg) s,
                out(reg) v,
                options(nostack),
            )
        };
        if (v.wrapping_sub(0x0101010101010101) & !v & 0x8080808080808080) != 0 {
            loop {
                if v & 0xFF == 0 {
                    return n;
                } else {
                    (n, v) = unsafe { (n + 1, v.unchecked_shr(8)) };
                }
            }
        } else {
            (n, y) = (n + 0x8, unsafe { y.add(1) });
        }
    }
}
#[inline]
pub fn set(x: *mut c_void, c: u8, count: usize) {
    #[cfg(target_feature = "ermsb")]
    unsafe {
        asm!(
            "repe stosb [rdi],al",
            inout("al") c => _,
            inout("rdi") x => _,
            inout("rcx") count => _,
            options(nostack, preserves_flags)
        )
    }
    #[cfg(not(target_feature = "ermsb"))]
    unsafe {
        let (mut d, v) = (x, (c as u64) * 0x101010101010101);
        let (p, q, b) = split(x, count);
        asm!(
            "rep stosb",
            in("rax") v,
            inout("ecx") p => _,
            inout("rdi") d => d,
            options(nostack, preserves_flags)
        );
        asm!(
            "rep stosq",
            in("rax") v,
            inout("rcx") q => _,
            inout("rdi") d => d,
            options(nostack, preserves_flags)
        );
        asm!(
            "rep stosb",
            in("rax") v,
            inout("ecx") b => _,
            inout("rdi") d => _,
            options(nostack, preserves_flags)
        )
    }
}
#[inline]
pub fn compare(x: *const c_void, y: *const c_void, n: usize) -> i32 {
    let g = |mut a: *const u8, mut b: *const u8, n| {
        for _ in 0..n {
            unsafe {
                if a.read() != b.read() {
                    return (a.read() as i32) - (b.read() as i32);
                }
                (a, b) = (a.add(1), b.add(1));
            }
        }
        0
    };
    let l = |a: *const u128, b, n| {
        cmp(a, b, n, |a: *const u64, b, n| {
            cmp(a, b, n, |a: *const u32, b, n| {
                cmp(a, b, n, |a: *const u16, b, n| cmp(a, b, n, g))
            })
        })
    };
    l(x.cast(), y.cast(), n)
}
#[inline]
pub fn copy_forward(x: *mut c_void, y: *const c_void, count: usize) {
    #[cfg(target_feature = "ermsb")]
    unsafe {
        asm!(
            "repe movsb [rdi],[rsi]",
            inout("rcx") count => _,
            inout("rdi") x => _,
            inout("rsi") y => _,
            options(nostack, preserves_flags)
        )
    }
    #[cfg(not(target_feature = "ermsb"))]
    unsafe {
        let (mut d, mut s) = (x, y);
        let (p, q, b) = split(x, count);
        asm!(
            "rep movsb",
            inout("ecx") p => _,
            inout("rsi") s => s,
            inout("rdi") d => d,
            options(nostack, preserves_flags)
        );
        asm!(
            "rep movsq",
            inout("rcx") q => _,
            inout("rsi") s => s,
            inout("rdi") d => d,
            options(nostack, preserves_flags)
        );
        asm!(
            "rep movsb",
            inout("ecx") b => _,
            inout("rsi") s => _,
            inout("rdi") d => _,
            options(nostack, preserves_flags)
        )
    }
}
#[inline]
pub fn copy_backward(x: *mut c_void, y: *const c_void, count: usize) {
    let (p, q, b) = split(x, count);
    unsafe {
        asm!(
            "std",
            "rep movsb",
            "sub    rsi, 7",
            "sub    rdi, 7",
            "mov    rcx, {1:r}",
            "rep movsq",
            "test {0:e}, {0:e}",
            "add    rsi, 7",
            "add    rdi, 7",
            "mov    ecx, {0:e}",
            "rep movsb",
            "cld",
            in(reg) p,
            in(reg) q,
            inout("ecx") b => _,
            inout("rdi") x.add(count - 1) => _,
            inout("rsi") y.add(count - 1) => _,
            options(nostack, preserves_flags)
        )
    }
}

#[inline]
fn split(x: *mut c_void, count: usize) -> (usize, usize, usize) {
    let p = ((0x8 - (x as usize & 0x7)) & 0x7).min(count);
    let c = count - p;
    unsafe { (p, c.unchecked_shr(3), c & 0x7) }
}
#[inline]
fn cmp<T: Clone + Copy + Eq, U: Clone + Copy + Eq, F: FnOnce(*const U, *const U, usize) -> i32>(a: *const T, b: *const T, n: usize, f: F) -> i32 {
    let (mut x, mut y, i) = unsafe { (a, b, a.add(unchecked_div(n, size_of::<T>()))) };
    while x != i {
        unsafe {
            if x.read_unaligned() != y.read_unaligned() {
                return f(x.cast(), y.cast(), size_of::<T>());
            }
            (x, y) = (x.add(1), y.add(1));
        }
    }
    unsafe { f(x.cast(), y.cast(), unchecked_rem(n, size_of::<T>())) }
}
