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
#![no_std]
#![allow(internal_features, suspicious_runtime_symbol_definitions)]
#![feature(core_intrinsics)]
#![cfg_attr(
    all(target_family = "windows", not(feature = "std"), not(test)),
    feature(lang_items)
)]

#[cfg(all(not(feature = "std"), not(target_os = "none")))]
pub use self::builtins::*;

#[cfg(all(not(feature = "std"), not(target_os = "none")))]
#[doc(hidden)]
#[path = "."]
mod builtins {
    extern crate core;

    use core::ffi::c_void;

    #[cfg(target_arch = "x86_64")]
    #[doc(hidden)]
    #[path = "x64.rs"]
    mod inner;
    #[cfg(not(target_arch = "x86_64"))]
    #[doc(hidden)]
    #[path = "other.rs"]
    mod inner;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[doc(hidden)]
    mod asm;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[doc(hidden)]
    pub use self::asm::*;

    #[unsafe(export_name = "strlen")]
    pub unsafe extern "C" fn strlen(b: *const i8) -> usize {
        inner::strlen(b)
    }
    #[unsafe(export_name = "memset")]
    pub unsafe extern "C" fn memset(b: *mut c_void, c: i32, n: usize) -> *mut c_void {
        inner::set(b, c as u8, n);
        b
    }
    #[unsafe(export_name = "memcmp")]
    pub unsafe extern "C" fn memcmp(j: *const c_void, k: *const c_void, n: usize) -> i32 {
        inner::compare(j, k, n)
    }
    #[unsafe(export_name = "memcpy")]
    pub unsafe extern "C" fn memcpy(d: *mut c_void, s: *const c_void, n: usize) -> *mut c_void {
        inner::copy_forward(d, s, n);
        d
    }
    #[unsafe(export_name = "memmove")]
    pub unsafe extern "C" fn memmove(d: *mut c_void, s: *const c_void, n: usize) -> *mut c_void {
        if (d as usize).wrapping_sub(s as usize) >= n {
            inner::copy_forward(d, s, n);
        } else {
            inner::copy_backward(d, s, n);
        }
        d
    }
}

#[cfg(all(target_family = "windows", not(feature = "std"), not(test)))]
#[doc(hidden)]
#[unsafe(no_mangle)]
extern "C" fn __CxxFrameHandler3() {}

#[cfg(all(target_family = "windows", not(feature = "std"), not(test)))]
#[doc(hidden)]
#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}
