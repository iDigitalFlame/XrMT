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

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::intrinsics::unlikely;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::*;
#[cfg(all(target_family = "windows", not(feature = "std")))]
pub use self::builtins::*;

#[macro_export]
macro_rules! abort {
    () => {
        crate::process::abort()
    };
}
#[macro_export]
macro_rules! ok_or_break {
    ($expression:expr) => {
        match $expression {
            Ok(v) => v,
            _ => break
        }
    };
    ($expression:expr, $ret:expr) => {
        match $expression {
            Ok(v) => v,
            _ => break $ret,
        }
    };
}
#[macro_export]
macro_rules! ignore_error {
    ($expression:expr) => {
        let _ =  $expression; // IGNORE ERROR
    };
}
#[macro_export]
macro_rules! ok_or_return {
    ($expression:expr, $ret:expr) => {
        match $expression {
            Ok(v) => v,
            Err(_) => return $ret,
        }
    };
}
#[macro_export]
macro_rules! some_or_break {
    ($expression:expr) => {
        match $expression {
            Some(v) => v,
            _ => break
        }
    };
    ($expression:expr, $ret:expr) => {
        match $expression {
            Some(v) => v,
            _ => break $ret,
        }
    };
}
#[macro_export]
macro_rules! some_or_return {
    ($expression:expr, $ret:expr) => {
        match $expression {
            Some(v) => v,
            None => return $ret,
        }
    };
}
#[macro_export]
macro_rules! ok_or_continue {
    ($expression:expr) => {
        match $expression {
            Ok(v) => v,
            _ => continue
        }
    };
}
#[macro_export]
macro_rules! some_or_continue {
    ($expression:expr) => {
        match $expression {
            Some(v) => v,
            _ => continue
        }
    };
}

pub trait AllocInto<T, A: Allocator = Global>: Sized {
    fn into_alloc(self, alloc: A) -> T;
}
pub trait AllocFrom<T, A: Allocator = Global>: Sized {
    fn from_alloc(value: T, alloc: A) -> Self;
}

#[inline]
pub fn panic(_msg: &str) -> ! {
    #[cfg(not(feature = "strip"))]
    {
        if !_msg.is_empty() {
            crate::println!("panic start: {_msg}");
            bugtrack!("panic start: {_msg}");
        }
    }
    crate::process::abort()
}
#[inline]
pub fn take<T>(r: Option<T>) -> T {
    match r {
        Some(v) => v,
        None => crate::process::abort(),
    }
}
#[inline]
pub fn unwrap<T, E: ToString>(r: Result<T, E>) -> T {
    #[cfg(feature = "strip")]
    match r {
        Ok(v) => v,
        Err(_) => crate::process::abort(),
    }
    #[cfg(not(feature = "strip"))]
    match r {
        Ok(v) => v,
        Err(e) => panic(&e.to_string()),
    }
}

#[inline]
pub(crate) fn unwrap_unlikely<T, E: ToString>(r: Result<T, E>) -> T {
    if unlikely(r.is_err()) {
        #[cfg(feature = "strip")]
        {
            crate::process::abort();
        }
        #[cfg(not(feature = "strip"))]
        {
            // SAFETY: We already verified it's an error.
            unsafe { panic(&r.unwrap_err_unchecked().to_string()) };
        }
    }
    // SAFETY: We already verified it's not an error.
    unsafe { r.unwrap_unchecked() }
}

#[cfg(all(windows, not(feature = "std")))]
#[path = "."]
mod inner {
    #[doc(inline)]
    pub extern crate alloc;
    #[doc(inline)]
    pub extern crate core;

    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into, TryFrom, TryInto};
    pub use core::default::Default;
    pub use core::future::{Future, IntoFuture};
    pub use core::hash::Hash;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    #[doc(inline)]
    pub use core::option::Option::{self, None, Some};
    #[doc(inline)]
    pub use core::result::Result::{self, Err, Ok};
    pub use core::{cfg, env};

    pub use crate::{abort, bugtrack, eprint, eprintln, ok_or_continue, ok_or_return, print, println, some_or_continue, some_or_return};

    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! eprint {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! eprintln {
        ($($arg:tt)*) => {{}};
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg(not(feature = "strip"))]
    pub use alloc::format;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg(not(feature = "strip"))]
    pub use core::{todo, write, writeln, panic};

    #[cfg(not(feature = "strip"))]
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stdout(alloc::format!($($arg)*));
        }};
    }
    #[cfg(not(feature = "strip"))]
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stdout(alloc::format!($($arg)*));
            let _ = crate::device::winapi::write_stdout("\n");
        }};
    }
    #[cfg(not(feature = "strip"))]
    #[macro_export]
    macro_rules! eprint {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stderr(alloc::format!($($arg)*));
        }};
    }
    #[cfg(not(feature = "strip"))]
    #[macro_export]
    macro_rules! eprintln {
        ($($arg:tt)*) => {{
            let _ = crate::device::winapi::write_stderr(alloc::format!($($arg)*));
            let _ = crate::device::winapi::write_stderr("\n");
        }};
    }
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    #[doc(inline)]
    pub extern crate alloc;
    #[doc(inline)]
    pub extern crate core;
    #[doc(inline)]
    pub extern crate std;

    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
    pub use core::clone::Clone;
    pub use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
    pub use core::convert::{AsMut, AsRef, From, Into, TryFrom, TryInto};
    pub use core::default::Default;
    pub use core::future::{Future, IntoFuture};
    pub use core::hash::Hash;
    pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator};
    pub use core::marker::{Copy, Send, Sized, Sync, Unpin};
    pub use core::mem::drop;
    pub use core::ops::{Drop, Fn, FnMut, FnOnce};
    pub use core::option::Option::{self, None, Some};
    pub use core::result::Result::{self, Err, Ok};
    pub use core::{cfg, env};

    pub use crate::{abort, bugtrack, ok_or_continue, ok_or_return, some_or_continue, some_or_return};

    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! eprint {
        ($($arg:tt)*) => {{}};
    }
    #[cfg(feature = "strip")]
    #[macro_export]
    macro_rules! eprintln {
        ($($arg:tt)*) => {{}};
    }

    #[cfg(not(feature = "strip"))]
    pub use std::{eprint, eprintln, panic, print, println};

    #[cfg(feature = "strip")]
    pub use crate::{eprint, eprintln, print, println};

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg(not(feature = "strip"))]
    pub use alloc::format;
    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg(not(feature = "strip"))]
    pub use core::{todo, write, writeln};
}

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[doc(hidden)]
mod builtins {
    #[cfg(target_arch = "x86_64")]
    #[path = "amd64.rs"]
    mod inner;
    #[cfg(not(target_arch = "x86_64"))]
    #[path = "standard.rs"]
    mod inner;
    #[cfg(all(target_family = "windows", any(target_arch = "x86", target_arch = "x86_64")))]
    mod ops;

    #[cfg(all(target_family = "windows", any(target_arch = "x86", target_arch = "x86_64")))]
    pub use self::ops::*;

    #[no_mangle]
    #[inline(always)]
    pub unsafe extern "C" fn strlen(b: *const u8) -> usize {
        inner::strlen(b)
    }
    #[no_mangle]
    #[inline(always)]
    pub unsafe extern "C" fn memset(b: *mut u8, c: i32, n: usize) -> *mut u8 {
        inner::set(b, c as u8, n);
        b
    }
    #[no_mangle]
    #[inline(always)]
    pub unsafe extern "C" fn memcmp(mem1: *const u8, mem2: *const u8, n: usize) -> i32 {
        inner::compare(mem1, mem2, n)
    }
    #[no_mangle]
    #[inline(always)]
    pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
        inner::copy_forward(dest, src, n);
        dest
    }
    #[no_mangle]
    #[inline(always)]
    pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
        if (dest as usize).wrapping_sub(src as usize) >= n {
            inner::copy_forward(dest, src, n);
        } else {
            inner::copy_backward(dest, src, n);
        }
        dest
    }
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
#[doc(hidden)]
mod builtins {}

#[cfg(all(target_family = "windows", not(feature = "std"), not(test)))]
#[doc(hidden)]
#[no_mangle]
extern "C" fn __CxxFrameHandler3() {}

#[cfg(all(target_family = "windows", not(feature = "std"), not(test)))]
#[doc(hidden)]
#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}
