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

extern crate core;

extern crate xrmt_bugtrack;
#[cfg(all(target_family = "windows", not(feature = "std")))]
extern crate xrmt_winapi;

use core::convert::Into;
use core::mem::forget;
use core::ops::{Drop, FnOnce};
use core::panicking::panic;

use xrmt_bugtrack::bugtrack;

use crate::process::{ExitCode, Termination};

#[macro_export]
macro_rules! abort {
    () => {
        #[cfg(debug_assertions)]
        {
            $crate::runtime::abort();
        }
        #[cfg(not(debug_assertions))]
        {
            $crate::runtime::exit_abort();
        }
    };
}
#[macro_export]
macro_rules! abort_unlikely {
    ($x:expr) => {
        {
            let __r = $x; // Don't redo the call.
            if $crate::runtime::likely(__r.is_ok()) {
                #[allow(unused_unsafe)]
                unsafe { __r.unwrap_unchecked() }
            } else {
                #[cfg(not(debug_assertions))]
                {
                    $crate::runtime::exit_abort()
                }
                #[cfg(debug_assertions)]
                {
                    __r.unwrap()
                }
            }
        }
    };
}

struct Guard(());

// Prevent users from needing the feature enabled.
#[inline(always)]
pub const fn likely(b: bool) -> bool {
    core::hint::likely(b)
}

#[inline]
pub fn exit_abort() -> ! {
    exit_with_code(ExitCode::FAILURE)
}
#[inline]
pub fn exit_success() -> ! {
    exit_with_code(ExitCode::SUCCESS)
}
#[inline]
pub fn exit_with_code(c: ExitCode) -> ! {
    #[cfg(all(target_family = "windows", not(feature = "std")))]
    {
        xrmt_winapi::unload_or_exit(c.code())
    }
    #[cfg(any(not(target_family = "windows"), feature = "std"))]
    {
        c.exit_process()
    }
}
#[inline]
pub fn exit_with(v: impl Termination) -> ! {
    exit_with_code(v.report())
}

#[inline]
pub fn run(f: impl FnOnce()) -> ! {
    exec(|| {
        f();
        ExitCode::SUCCESS
    })
}
#[inline]
pub fn exec(f: impl FnOnce() -> ExitCode) -> ! {
    let g = Guard(()); // TODO(dij): Do we need this?
                       // ^ Even though we don't anticipate panics to happen here. This will alow
                       // for unloading if not in debug mode. It will only exit with failure as
                       // we'll exit it after.
                       //
                       // Need to test this
    bugtrack!("runtime_exec(): Start!");
    let e = f();
    bugtrack!("runtime_exec(): Completed with ExitCode {e:?}!");
    forget(g); // Don't run drop code.
    exit_with_code(e)
}
#[inline]
pub fn exec_code(f: impl FnOnce() -> u32) -> ! {
    exec(|| (f() as u8).into())
}

#[doc(hidden)]
#[inline]
pub fn abort() -> ! {
    panic("");
}

impl Drop for Guard {
    #[cold]
    fn drop(&mut self) {
        bugtrack!("(Guard).drop(): Dropping runtime guard!");
        #[cfg(not(debug_assertions))]
        {
            exit_abort()
        }
    }
}

#[cfg(all(windows, not(feature = "no-support"), not(feature = "std"), not(test)))]
mod support {
    extern crate core;

    extern crate xrmt_bugtrack;
    extern crate xrmt_winapi;

    use core::panic::PanicInfo;

    use xrmt_bugtrack::bugtrack;
    use xrmt_winapi::alloc::HeapAllocator;
    use xrmt_winapi::functions::exit_process;
    #[cfg(any(debug_assertions, not(feature = "strip")))]
    use xrmt_winapi::stderr;
    use xrmt_winapi::unload_libraries;

    #[global_allocator]
    static GLOBAL: HeapAllocator = HeapAllocator::new();

    #[panic_handler]
    #[cold]
    pub fn panic_handler(_p: &PanicInfo<'_>) -> ! {
        #[cfg(any(debug_assertions, not(feature = "strip")))]
        {
            bugtrack!("==============================");
            bugtrack!("RUST PANIC OCCURRED!");
            stderr!("RUST PANIC OCCURRED!");
            bugtrack!("==============================");
            bugtrack!("{}", _p.message());
            stderr!("{}", _p.message());
            if let core::option::Option::Some(_v) = _p.location() {
                bugtrack!(
                    "Location: {} (line:{}, col:{})",
                    _v.file(),
                    _v.line(),
                    _v.column()
                );
                stderr!(
                    "Location: {} (line:{}, col:{})",
                    _v.file(),
                    _v.line(),
                    _v.column()
                );
            }
            bugtrack!("==============================");
        }
        unload_libraries();
        exit_process(5)
    }
}
