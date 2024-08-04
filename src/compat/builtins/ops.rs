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
#![cfg(all(target_family = "windows", not(feature = "std")))]

pub use self::inner::*;

#[doc(hidden)]
#[no_mangle]
pub static _fltused: i32 = 0;

#[cfg(target_arch = "x86")]
#[doc(hidden)]
mod inner {
    extern crate core;
    use core::arch::asm;

    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _chkstk() {
        asm!(
            "push ecx
             lea  ecx,             dword ptr [esp + 0x4]
             sub  ecx,             eax
             sbb  eax,             eax
             not  eax
             and  ecx,             eax
             mov  eax,             esp
             and  eax,             0xFFFFF000
             2:
             cmp  ecx,             eax
             jb   3f
             mov  eax,             ecx
             pop  ecx
             xchg eax,             esp
             mov  eax,             dword ptr [eax]
             mov  dword ptr [esp], eax
             ret
             3:
             sub  eax,             0x1000
             test dword ptr [eax], eax
             jmp  2b",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _allshl(value: i64, positions: i64) -> i64 {
        asm!(
            "cmp  cl,  64
             jnb  3f
             cmp  cl,  32
             jnb  2f
             shld edx, eax, cl
             shl  eax, cl
             ret
             2:
             mov  edx, eax
             xor  eax, eax
             and  cl,  32-1
             shl  edx, cl
             ret
             3:
             xor  eax, eax
             xor  edx, edx
             ret",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _allshr(value: i64, positions: i64) -> i64 {
        asm!(
            "cmp  cl,  64
             jnb  3f
             cmp  cl,  32
             jnb  2f
             shrd eax, edx, cl
             sar  edx, cl
             ret
             2:
             mov  eax, edx
             sar  edx, 32-1
             and  cl,  32-1
             sar  eax, cl
             ret
             3:
             sar  edx, 0x1F
             mov  eax, edx
             ret",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _aullshr(value: u64, positions: u64) -> u64 {
        asm!(
            "cmp  cl,  64
             jnb  3f
             cmp  cl,  32
             jnb  2f
             shrd eax, edx, cl
             shr  edx, cl
             ret
             2:
             mov  eax, edx
             xor  edx, edx
             and  cl,  32-1
             shr  eax, cl
             ret
             3:
             xor  eax, eax
             xor  edx, edx
             ret",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _alldiv(dividend: i64, divisor: i64) -> i64 {
        asm!(
            "push edi
             push esi
             push ebx
             xor  edi,                    edi
             mov  eax,                    dword ptr [esp + 0x14]
             or   eax,                    eax
             jge  2f
             inc  edi
             mov  edx,                    dword ptr [esp + 0x10]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x14], eax
             mov  dword ptr [esp + 0x10], edx
             2:
             mov  eax,                    dword ptr [esp + 0x1C]
             or   eax,                    eax
             jge  3f
             inc  edi
             mov  edx,                    dword ptr [esp + 0x18]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x1C], eax
             mov  dword ptr [esp + 0x18], edx
             3:
             or   eax,                    eax
             jnz  4f
             mov  ecx,                    dword ptr [esp + 0x18]
             mov  eax,                    dword ptr [esp + 0x14]
             xor  edx,                    edx
             div  ecx
             mov  ebx,                    eax
             mov  eax,                    dword ptr [esp + 0x10]
             div  ecx
             mov  edx,                    ebx
             jmp  8f
             4:
             mov  ebx,                    eax
             mov  ecx,                    dword ptr [esp + 0x18]
             mov  edx,                    dword ptr [esp + 0x14]
             mov  eax,                    dword ptr [esp + 0x10]
             5:
             shr  ebx,                    1
             rcr  ecx,                    1
             shr  edx,                    1
             rcr  eax,                    1
             or   ebx,                    ebx
             jnz  5b
             div  ecx
             mov  esi,                    eax
             mul  dword ptr [esp + 0x1C]
             mov  ecx,                    eax
             mov  eax,                    dword ptr [esp + 0x18]
             mul  esi
             add  edx,                    ecx
             jb   6f
             cmp  edx,                    dword ptr [esp + 0x14]
             ja   6f
             jb   7f
             cmp  eax,                    dword ptr [esp + 0x10]
             jbe  7f
             6:
             dec  esi
             7:
             xor  edx,                    edx
             mov  eax,                    esi
             8:
             dec  edi
             jnz  9f
             neg  edx
             neg  eax
             sbb  edx,                    0
             9:
             pop  ebx
             pop  esi
             pop  edi
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _allrem(dividend: i64, divisor: i64) -> i64 {
        asm!(
            "push ebx
             push edi
             xor  edi,                    edi
             mov  eax,                    dword ptr [esp + 0x10]
             or   eax,                    eax
             jge  2f
             inc  edi
             mov  edx,                    dword ptr [esp + 0xC]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x10], eax
             mov  dword ptr [esp + 0xC],  edx
             2:
             mov  eax,                    dword ptr [esp + 0x18]
             or   eax,                    eax
             jge  3f
             mov  edx,                    dword ptr [esp + 0x14]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x18], eax
             mov  dword ptr [esp + 0x14], edx
             3:
             or   eax,                    eax
             jnz  4f
             mov  ecx,                    dword ptr [esp + 0x14]
             mov  eax,                    dword ptr [esp + 0x10]
             xor  edx,                    edx
             div  ecx
             mov  eax,                    dword ptr [esp + 0xC]
             div  ecx
             mov  eax,                    edx
             xor  edx,                    edx
             dec  edi
             jns  8f
             jmp  9f
             4:
             mov  ebx,                    eax
             mov  ecx,                    dword ptr [esp + 0x14]
             mov  edx,                    dword ptr [esp + 0x10]
             mov  eax,                    dword ptr [esp + 0xC]
             5:
             shr  ebx,                    1
             rcr  ecx,                    1
             shr  edx,                    1
             rcr  eax,                    1
             or   ebx,                    ebx
             jnz  5b
             div  ecx
             mov  ecx,                    eax
             mul  dword ptr [esp + 0x18]
             xchg eax,                    ecx
             mul  dword ptr [esp + 0x14]
             add  edx,                    ecx
             jb   6f
             cmp  edx,                    dword ptr [esp + 0x10]
             ja   6f
             jb   7f
             cmp  eax,                    dword ptr [esp + 0xC]
             jbe  7f
             6:
             sub  eax,                    dword ptr [esp + 0x14]
             sbb  edx,                    dword ptr [esp + 0x18]
             7:
             sub  eax,                    dword ptr [esp + 0xC]
             sbb  edx,                    dword ptr [esp + 0x10]
             dec  edi
             jns  9f
             8:
             neg  edx
             neg  eax
             sbb  edx,                    0
             9:
             pop  edi
             pop  ebx
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _aulldiv(dividend: u64, divisor: u64) -> u64 {
        asm!(
            "push ebx
             push esi
             mov  eax, dword ptr [esp + 0x18]
             or   eax, eax
             jnz  2f
             mov  ecx, dword ptr [esp + 0x14]
             mov  eax, dword ptr [esp + 0x10]
             xor  edx, edx
             div  ecx
             mov  ebx, eax
             mov  eax, dword ptr [esp + 0xC]
             div  ecx
             mov  edx, ebx
             jmp  6f
             2:
             mov  ecx, eax
             mov  ebx, dword ptr [esp + 0x14]
             mov  edx, dword ptr [esp + 0x10]
             mov  eax, dword ptr [esp + 0xC]
             3:
             shr  ecx, 1
             rcr  ebx, 1
             shr  edx, 1
             rcr  eax, 1
             or   ecx, ecx
             jnz  3b
             div  ebx
             mov  esi, eax
             mul  dword ptr [esp + 0x18]
             mov  ecx, eax
             mov  eax, dword ptr [esp + 0x14]
             mul  esi
             add  edx, ecx
             jb   4f
             cmp  edx, dword ptr [esp + 0x10]
             ja   4f
             jb   5f
             cmp  eax, dword ptr [esp + 0xC]
             jbe  5f
             4:
             dec  esi
             5:
             xor  edx, edx
             mov  eax, esi
             6:
             pop  esi
             pop  ebx
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _aullrem(dividend: u64, divisor: u64) -> u64 {
        asm!(
            "push ebx
             mov  eax, dword ptr [esp + 0x14]
             or   eax, eax
             jnz  2f
             mov  ecx, dword ptr [esp + 0x10]
             mov  eax, dword ptr [esp + 0xC]
             xor  edx, edx
             div  ecx
             mov  eax, dword ptr [esp + 0x8]
             div  ecx
             mov  eax, edx
             xor  edx, edx
             jmp  6f
             2:
             mov  ecx, eax
             mov  ebx, dword ptr [esp + 0x10]
             mov  edx, dword ptr [esp + 0xC]
             mov  eax, dword ptr [esp + 0x8]
             3:
             shr  ecx, 1
             rcr  ebx, 1
             shr  edx, 1
             rcr  eax, 1
             or   ecx, ecx
             jnz  3b
             div  ebx
             mov  ecx, eax
             mul  dword ptr [esp + 0x14]
             xchg eax, ecx
             mul  dword ptr [esp + 0x10]
             add  edx, ecx
             jb   4f
             cmp  edx, dword ptr [esp + 0xC]
             ja   4f
             jb   5f
             cmp  eax, dword ptr [esp + 0x8]
             jbe  5f
             4:
             sub  eax, dword ptr [esp + 0x10]
             sbb  edx, dword ptr [esp + 0x14]
             5:
             sub  eax, dword ptr [esp + 0x8]
             sbb  edx, dword ptr [esp + 0xC]
             neg  edx
             neg  eax
             sbb  edx, 0
             6:
             pop  ebx
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _aulldvrm(dividend: u64, divisor: u64) -> u64 {
        asm!(
            "push esi
             mov  eax, dword ptr [esp + 0x14]
             or   eax, eax
             jnz  2f
             mov  ecx, dword ptr [esp + 0x10]
             mov  eax, dword ptr [esp + 0xC]
             xor  edx, edx
             div  ecx
             mov  ebx, eax
             mov  eax, dword ptr [esp + 0x8]
             div  ecx
             mov  esi, eax
             mov  eax, ebx
             mul  dword ptr [esp + 0x10]
             mov  ecx, eax
             mov  eax, esi
             mul  dword ptr [esp + 0x10]
             add  edx, ecx
             jmp  6f
             2:
             mov  ecx, eax
             mov  ebx, dword ptr [esp + 0x10]
             mov  edx, dword ptr [esp + 0xC]
             mov  eax, dword ptr [esp + 0x8]
             3:
             shr  ecx, 1
             rcr  ebx, 1
             shr  edx, 1
             rcr  eax, 1
             or   ecx, ecx
             jnz  3b
             div  ebx
             mov  esi, eax
             mul  dword ptr [esp + 0x14]
             mov  ecx, eax
             mov  eax, dword ptr [esp + 0x10]
             mul  esi
             add  edx, ecx
             jb   4f
             cmp  edx, dword ptr [esp + 0xC]
             ja   4f
             jb   5f
             cmp  eax, dword ptr [esp + 0x8]
             jbe  5f
             4:
             dec  esi
             sub  eax, dword ptr [esp + 0x10]
             sbb  edx, dword ptr [esp + 0x14]
             5:
             xor  ebx, ebx
             6:
             sub  eax, dword ptr [esp + 0x8]
             sbb  edx, dword ptr [esp + 0xC]
             neg  edx
             neg  eax
             sbb  edx, 0
             mov  ecx, edx
             mov  edx, ebx
             mov  ebx, ecx
             mov  ecx, eax
             mov  eax, esi
             pop  esi
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn _allmul(multiplier: i64, multiplicand: i64) -> i64 {
        asm!(
            "mov  eax, dword ptr [esp + 0x8]
             mov  ecx, dword ptr [esp + 0x10]
             or   ecx, eax
             mov  ecx, dword ptr [esp + 0xC]
             jnz  2f
             mov  eax, dword ptr [esp + 0x4]
             mul  ecx
             ret  2*8
             2:
             push ebx
             mul  ecx
             mov  ebx, eax
             mov  eax, dword ptr [esp + 0x8]
             mul  dword ptr [esp + 0x14]
             add  ebx, eax
             mov  eax, dword ptr [esp + 0x8]
             mul  ecx
             add  edx, ebx
             pop  ebx
             ret  2*8",
            options(noreturn)
        );
    }
    #[naked]
    #[no_mangle]
    #[allow(improper_ctypes_definitions)]
    pub unsafe extern "C" fn _alldvrm(dividend: i64, divisor: i64) -> (i64, i64) {
        asm!(
            "push edi
             push esi
             push ebp
             xor  edi,                    edi
             xor  ebp,                    ebp
             mov  eax,                    dword ptr [esp + 0x14]
             or   eax,                    eax
             jge  2f
             inc  edi
             inc  ebp
             mov  edx,                    dword ptr [esp + 0x10]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x14], eax
             mov  dword ptr [esp + 0x10], edx
             2:
             mov  eax,                    dword ptr [esp + 0x1C]
             or   eax,                    eax
             jge  3f
             inc  edi
             mov  edx,                    dword ptr [esp + 0x18]
             neg  eax
             neg  edx
             sbb  eax,                    0
             mov  dword ptr [esp + 0x1C], eax
             mov  dword ptr [esp + 0x18], edx
             3:
             or   eax,                    eax
             jnz  4f
             mov  ecx,                    dword ptr [esp + 0x18]
             mov  eax,                    dword ptr [esp + 0x14]
             xor  edx,                    edx
             div  ecx
             mov  ebx,                    eax
             mov  eax,                    dword ptr [esp + 0x10]
             div  ecx
             mov  esi,                    eax
             mov  eax,                    ebx
             mul  dword ptr [esp + 0x18]
             mov  ecx,                    eax
             mov  eax,                    esi
             mul  dword ptr [esp + 0x18]
             add  edx,                    ecx
             jmp  8f
             4:
             mov  ebx,                    eax
             mov  ecx,                    dword ptr [esp + 0x18]
             mov  edx,                    dword ptr [esp + 0x14]
             mov  eax,                    dword ptr [esp + 0x10]
             5:
             shr  ebx,                    1
             rcr  ecx,                    1
             shr  edx,                    1
             rcr  eax,                    1
             or   ebx,                    ebx
             jnz  5b
             div  ecx
             mov  esi,                    eax
             mul  dword ptr [esp + 0x1C]
             mov  ecx,                    eax
             mov  eax,                    dword ptr [esp + 0x18]
             mul  esi
             add  edx,                    ecx
             jb   6f
             cmp  edx,                    dword ptr [esp + 0x14]
             ja   6f
             jb   7f
             cmp  eax,                    dword ptr [esp + 0x10]
             jbe  7f
             6:
             dec  esi
             sub  eax,                    dword ptr [esp + 0x18]
             sbb  edx,                    dword ptr [esp + 0x1C]
             7:
             xor  ebx,                    ebx
             8:
             sub  eax,                    dword ptr [esp + 0x10]
             sbb  edx,                    dword ptr [esp + 0x14]
             dec  ebp
             jns  9f
             neg  edx
             neg  eax
             sbb  edx,                    0
             9:
             mov  ecx,                    edx
             mov  edx,                    ebx
             mov  ebx,                    ecx
             mov  ecx,                    eax
             mov  eax,                    esi
             dec  edi
             jnz  12f
             neg  edx
             neg  eax
             sbb  edx,                    0
             12:
             pop  ebp
             pop  esi
             pop  edi
             ret  2*8",
            options(noreturn)
        );
    }
}
#[cfg(target_arch = "x86_64")]
#[doc(hidden)]
mod inner {
    extern crate core;
    use core::arch::asm;

    #[naked]
    #[no_mangle]
    pub unsafe extern "C" fn __chkstk() {
        asm!(
            "sub   rsp,                   0x10
             mov   qword ptr [rsp],       r10
             mov   qword ptr [rsp + 0x8], r11
             xor   r11,                   r11
             lea   r10,                   qword ptr [rsp + 0x18]
             sub   r10,                   rax
             cmovb r10,                   r11
             mov   r11,                   qword ptr gs:[0x10]
             cmp   r10,                   r11
             jnb   3f
             and   r10w,                  0xF000
             2:
             lea   r11,                   qword ptr [r11 - 0x1000]
             test  byte ptr [r11],        r11b
             cmp   r10,                   r11
             jb    2b
             3:
             mov   r10,                   qword ptr [rsp]
             mov   r11,                   qword ptr [rsp + 0x8]
             add   rsp,                   10h
             ret",
            options(noreturn)
        );
    }
}
