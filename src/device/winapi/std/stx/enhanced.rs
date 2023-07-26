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

use core::arch::asm;
use core::clone::Clone;
use core::cmp::{Eq, Ord};
use core::marker::Copy;
use core::ops::FnOnce;
use core::{intrinsics, mem};

#[cfg(target_feature = "sse2")]
#[cfg_attr(rustfmt, rustfmt_skip)]
use core::arch::x86_64::{__m128i, _mm_cmpeq_epi8, _mm_movemask_epi8, _mm_set1_epi8};

#[cfg(target_feature = "sse2")]
#[inline(always)]
pub(super) unsafe fn strlen(mut s: *const u8) -> usize {
    let mut n = 0;
    for _ in 0..4 {
        if *s == 0 {
            return n;
        }
        n += 1;
        s = s.add(1);
    }
    let a = s as usize & 0xF;
    let mut s = ((s as usize) - a) as *const __m128i;
    let z = _mm_set1_epi8(0);
    let x = {
        let r;
        asm!(
            "movdqa ({addr}), {dest}",
            addr = in(reg) s,
            dest = out(xmm_reg) r,
            options(att_syntax, nostack),
        );
        r
    };
    let v = _mm_movemask_epi8(_mm_cmpeq_epi8(x, z)) >> a;
    if v != 0 {
        return n + v.trailing_zeros() as usize;
    }
    n += 0xF - a;
    s = s.add(1);
    loop {
        let x = {
            let r;
            asm!(
                "movdqa ({addr}), {dest}",
                addr = in(reg) s,
                dest = out(xmm_reg) r,
                options(att_syntax, nostack),
            );
            r
        };
        let v = _mm_movemask_epi8(_mm_cmpeq_epi8(x, z)) as u32;
        if v == 0 {
            n += 0xF;
            s = s.add(1);
        } else {
            return n + v.trailing_zeros() as usize;
        }
    }
}
#[cfg(not(target_feature = "sse2"))]
#[inline(always)]
pub(super) unsafe fn strlen(mut s: *const u8) -> usize {
    let mut n = 0;
    while s as usize & 7 != 0 {
        if *s == 0 {
            return n;
        }
        n += 1;
        s = s.add(1);
    }
    let mut s = s as *const u64;
    loop {
        let mut cs = {
            let r: u64;
            asm!(
                "mov ({addr}), {dest}",
                addr = in(reg) s,
                dest = out(reg) r,
                options(att_syntax, nostack),
            );
            r
        };
        if (cs.wrapping_sub(0x0101010101010101) & !cs & 0x8080808080808080) != 0 {
            loop {
                if cs & 0xFF == 0 {
                    return n;
                } else {
                    cs >>= 8;
                    n += 1;
                }
            }
        } else {
            n += 8;
            s = s.add(1);
        }
    }
}
#[inline(always)]
pub(super) unsafe fn set(mut dest: *mut u8, c: u8, count: usize) {
    #[cfg(target_feature = "ermsb")]
    {
        asm!(
            "repe stosb %al, (%rdi)",
            inout("rcx") count => _,
            inout("rdi") dest => _,
            inout("al") c => _,
            options(att_syntax, nostack, preserves_flags)
        )
    }
    #[cfg(not(target_feature = "ermsb"))]
    {
        let c = c as u64 * 0x0101_0101_0101_0101;
        let (p, q, b) = split(dest, count);
        asm!(
            "rep stosb",
            inout("ecx") p => _,
            inout("rdi") dest => dest,
            in("rax") c,
            options(att_syntax, nostack, preserves_flags)
        );
        asm!(
            "rep stosq",
            inout("rcx") q => _,
            inout("rdi") dest => dest,
            in("rax") c,
            options(att_syntax, nostack, preserves_flags)
        );
        asm!(
            "rep stosb",
            inout("ecx") b => _,
            inout("rdi") dest => _,
            in("rax") c,
            options(att_syntax, nostack, preserves_flags)
        );
    }
}
#[inline(always)]
pub(super) unsafe fn compare(a: *const u8, b: *const u8, n: usize) -> i32 {
    let g = |mut a: *const u8, mut b: *const u8, n| {
        for _ in 0..n {
            if a.read() != b.read() {
                return (a.read() as i32) - (b.read() as i32);
            }
            a = a.add(1);
            b = b.add(1);
        }
        0
    };
    // NOTE(dij): This is not compressed as it's very complex.
    let h = |a: *const u16, b, n| _cmp(a, b, n, g);
    let j = |a: *const u32, b, n| _cmp(a, b, n, h);
    let k = |a: *const u64, b, n| _cmp(a, b, n, j);
    let l = |a: *const u128, b, n| _cmp(a, b, n, k);
    l(a.cast(), b.cast(), n)
}
#[inline(always)]
pub(super) unsafe fn copy_backward(dest: *mut u8, src: *const u8, count: usize) {
    let (p, q, b) = split(dest, count);
    asm!(
        "std",
        "rep movsb",
        "sub $7, %rsi",
        "sub $7, %rdi",
        "mov {q}, %rcx",
        "rep movsq",
        "test {p:e}, {p:e}",
        "add $7, %rsi",
        "add $7, %rdi",
        "mov {p:e}, %ecx",
        "rep movsb",
        "cld",
        p = in(reg) p,
        q = in(reg) q,
        inout("ecx") b => _,
        inout("rdi") dest.add(count - 1) => _,
        inout("rsi") src.add(count - 1) => _,
        options(att_syntax, nostack, preserves_flags)
    );
}
#[inline(always)]
pub(super) unsafe fn copy_forward(mut dest: *mut u8, mut src: *const u8, count: usize) {
    #[cfg(target_feature = "ermsb")]
    {
        asm!(
            "repe movsb (%rsi), (%rdi)",
            inout("rcx") count => _,
            inout("rdi") dest => _,
            inout("rsi") src => _,
            options(att_syntax, nostack, preserves_flags)
        );
    }
    #[cfg(not(target_feature = "ermsb"))]
    {
        let (p, q, b) = split(dest, count);
        asm!(
            "rep movsb",
            inout("ecx") p => _,
            inout("rdi") dest => dest,
            inout("rsi") src => src,
            options(att_syntax, nostack, preserves_flags)
        );
        asm!(
            "rep movsq",
            inout("rcx") q => _,
            inout("rdi") dest => dest,
            inout("rsi") src => src,
            options(att_syntax, nostack, preserves_flags)
        );
        asm!(
            "rep movsb",
            inout("ecx") b => _,
            inout("rdi") dest => _,
            inout("rsi") src => _,
            options(att_syntax, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
fn split(dest: *mut u8, mut count: usize) -> (usize, usize, usize) {
    let p = ((8 - (dest as usize & 0b111)) & 0b111).min(count);
    count -= p;
    (p, count >> 3, count & 0b111)
}

#[inline(always)]
unsafe fn _cmp<T: Clone + Copy + Eq, U: Clone + Copy + Eq, F: FnOnce(*const U, *const U, usize) -> i32>(mut a: *const T, mut b: *const T, n: usize, f: F) -> i32 {
    let end = a.add(intrinsics::unchecked_div(n, mem::size_of::<T>()));
    while a != end {
        if a.read_unaligned() != b.read_unaligned() {
            return f(a.cast(), b.cast(), mem::size_of::<T>());
        }
        a = a.add(1);
        b = b.add(1);
    }
    f(
        a.cast(),
        b.cast(),
        intrinsics::unchecked_rem(n, mem::size_of::<T>()),
    )
}
