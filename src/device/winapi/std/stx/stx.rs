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
#![cfg(all(windows, not(feature = "std")))]

extern crate alloc;
extern crate core;

use alloc::string::ToString;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};

pub mod ffi;
pub mod io;

pub mod prelude {
    pub extern crate alloc;
    pub extern crate core;

    #[cfg(not(feature = "implant"))]
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into, TryFrom, TryInto};
    pub use core::default::Default;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};
    #[cfg(not(feature = "implant"))]
    pub use core::{todo, write, writeln};

    pub use crate::{print, println};
}

#[cfg(feature = "std")]
pub mod builtin {}
#[cfg(not(feature = "std"))]
#[path = "."]
pub mod builtin {
    #[cfg(target_arch = "x86_64")]
    #[path = "enhanced.rs"]
    mod inner;
    #[cfg(not(target_arch = "x86_64"))]
    #[path = "compat.rs"]
    mod inner;
    #[cfg(all(windows, any(target_arch = "x86", target_arch = "x86_64")))]
    mod ops;

    #[cfg(all(windows, any(target_arch = "x86", target_arch = "x86_64")))]
    pub use self::ops::*;

    #[no_mangle]
    #[inline]
    pub unsafe extern "C" fn strlen(b: *const u8) -> usize {
        inner::strlen(b)
    }
    #[no_mangle]
    #[inline]
    pub unsafe extern "C" fn memset(b: *mut u8, c: i32, n: usize) -> *mut u8 {
        inner::set(b, c as u8, n);
        b
    }
    #[no_mangle]
    #[inline]
    pub unsafe extern "C" fn memcmp(mem1: *const u8, mem2: *const u8, n: usize) -> i32 {
        inner::compare(mem1, mem2, n)
    }
    #[no_mangle]
    #[inline]
    pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
        inner::copy_forward(dest, src, n);
        dest
    }
    #[no_mangle]
    #[inline]
    pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
        if (dest as usize).wrapping_sub(src as usize) >= n {
            inner::copy_forward(dest, src, n);
        } else {
            inner::copy_backward(dest, src, n);
        }
        dest
    }
}

#[cfg(all(windows, not(feature = "implant")))]
mod printer {
    extern crate alloc;

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stdout(alloc::format!($($arg)*));
        }};
    }
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stdout(alloc::format!($($arg)*));
            let _ = crate::device::winapi::write_stdout("\n");
        }};
    }
}
#[cfg(any(unix, all(windows, feature = "implant")))]
mod printer {
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{}};
    }
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{}};
    }
}

#[inline]
pub fn abort() -> ! {
    crate::process::abort()
}
#[inline]
pub fn panic(_msg: &str) -> ! {
    #[cfg(not(feature = "implant"))]
    {
        if !_msg.is_empty() {
            crate::println!("panic start: {_msg}");
            crate::bugtrack!("panic start: {_msg}");
        }
    }
    abort()
}
#[inline]
pub fn take<T>(r: Option<T>) -> T {
    match r {
        Some(v) => v,
        None => abort(),
    }
}
#[inline]
pub fn unwrap<T, E: ToString>(r: Result<T, E>) -> T {
    #[cfg(feature = "implant")]
    match r {
        Ok(v) => v,
        Err(_) => abort(),
    }
    #[cfg(not(feature = "implant"))]
    match r {
        Ok(v) => v,
        Err(e) => panic(&e.to_string()),
    }
}

// We can keep this here as it doesn't really do anything.
#[cfg(not(test))]
#[no_mangle]
extern "C" fn __CxxFrameHandler3() {}

// We can keep this here as it doesn't really do anything.
#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}
