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
#![allow(internal_features)]
#![feature(
    alloc_layout_extra,
    allocator_api,
    core_intrinsics,
    extend_one,
    format_args_nl,
    layout_for_ptr,
    likely_unlikely,
    log_syntax,
    never_type,
    panic_internals,
    slice_concat_trait,
    slice_range,
    trace_macros
)]
#![cfg_attr(not(target_family = "windows"), feature(exitcode_exit_method))]
#![cfg_attr(
    feature = "compat",
    feature(
        assert_matches,
        bstr,
        cfg_select,
        concat_bytes,
        const_format_args,
        f128,
        f16,
        new_range_api,
        pattern_type_macro,
        portable_simd,
        random,
        ub_checks
    )
)]
#![cfg_attr(feature = "std", feature(exitcode_exit_method))]
//#![cfg_attr(any(not(target_family = "windows"), feature = "std"),
//#![cfg_attr(any(not(target_family feature(print_internals))]

extern crate xrmt_builtins;

mod compat;
pub mod os;
pub mod prelude;
pub mod runtime;
pub mod sync;

// Event though this imports `[,e]print[,ln]` it still warns as imported oddly
// enough.
#[allow(unused_imports)]
pub use self::compat::*;
pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "."]
mod inner {
    pub mod env;
    pub mod ffi;
    pub mod fs;
    pub mod io;
    pub mod net;
    pub mod path;
    pub mod process;
    pub mod thread;
    pub mod time;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
#[path = "."]
mod inner {
    extern crate std;

    pub use std::{env, ffi, fs, net, path, process, thread};

    pub mod io {
        extern crate xrmt_io;
        pub use xrmt_io::*;
    }
    pub mod time;
}
