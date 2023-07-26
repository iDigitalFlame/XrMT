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
#![no_main]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    not(feature = "std"),
    feature(
        start,
        lang_items,
        ptr_as_uninit,
        naked_functions,
        core_intrinsics,
        panic_info_message,
        maybe_uninit_slice,
        maybe_uninit_write_slice,
    )
)]
#![cfg_attr(any(unix, feature = "std"), feature(io_error_more, can_vector, seek_stream_len))]
#![feature(ip_in_core, error_in_core, new_uninit)]
// We'll enable these later :P
//#![windows_subsystem = "console"]
//#![windows_subsystem = "windows"]

pub mod c2;
pub mod com;
pub mod data;
pub mod device;
pub mod net;
pub mod process;
pub mod sync;
pub mod thread;
pub mod util;

use crate::com::{Flag, Packet};
use crate::data::Writer;
use crate::util::stx::prelude::*;

#[cfg(all(windows, not(feature = "std")))]
#[global_allocator]
static GLOBAL: device::winapi::HeapAllocator = device::winapi::HeapAllocator::new();

fn main_inner() -> i32 {
    let mut p = Packet::default();
    p.id = 233;
    p.flags.set_group(0x123);
    p.flags.set_position(133);
    p.flags.set_len(999);
    p.flags |= Flag::CRYPT;
    p.write_str(&"derp").unwrap();

    println!("packet {:?}", p);

    c2::shoot("localhost:8080", p).unwrap();

    0
}

#[cfg(any(unix, feature = "std"))]
#[inline]
#[no_mangle]
fn main() {
    process::exit(main_inner())
}

#[cfg(all(windows, not(feature = "std")))]
#[inline]
#[no_mangle]
extern "C" fn _main() {
    let r = main_inner();
    device::winapi::unload_libraries();
    process::exit(r)
}

#[cfg(all(windows, not(feature = "std")))]
#[inline]
#[no_mangle]
extern "C" fn WinMain(_h: usize, _z: usize, _a: usize, _s: u32) -> i32 {
    let r = main_inner();
    device::winapi::unload_libraries();
    r
}

#[cfg(not(test))]
#[cfg(all(windows, not(feature = "std")))]
#[allow(unused_variables)]
#[panic_handler]
fn panic(p: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(not(feature = "implant"))]
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
    device::winapi::unload_libraries();
    process::exit(255)
}
