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

pub mod crypt;
pub mod log;
mod numbers;

pub use self::numbers::*;

#[cfg(all(windows, not(feature = "std")))]
#[path = "device/winapi/std/stx/stx.rs"]
pub mod stx;
#[cfg(any(unix, feature = "std"))]
#[cfg(unix)]
pub mod stx {
    extern crate alloc;
    extern crate core;
    extern crate std;

    use alloc::string::ToString;
    use core::option::Option;
    use core::option::Option::{None, Some};
    use core::result::Result;
    use core::result::Result::{Err, Ok};

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use std::*;

    pub mod ffi {
        extern crate std;

        pub use std::ffi::{OsStr, OsString};
        pub use std::path::{Path, PathBuf};
    }
    pub mod prelude {
        pub extern crate alloc;
        pub extern crate core;
        pub extern crate std;

        #[cfg(not(feature = "implant"))]
        pub use alloc::format;
        pub use alloc::vec;
        #[cfg(not(feature = "implant"))]
        pub use core::{todo, write, writeln};
        pub use std::prelude::rust_2021::*;

        pub use super::printer::*;
    }

    #[cfg(feature = "implant")]
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
    #[cfg(not(feature = "implant"))]
    mod printer {
        extern crate std;
        pub use std::{print, println};
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
                std::println!("panic start: {_msg}");
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
}

#[inline]
pub fn copy(dst: &mut [u8], src: &[u8]) -> usize {
    if src.len() == 0 || dst.len() == 0 {
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
