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
#![allow(async_fn_in_trait, internal_features)]
#![feature(allocator_api, likely_unlikely, slice_internals, trait_alias)]
#![cfg_attr(
    not(target_family = "windows"),
    feature(buf_read_has_data_left, can_vector, tcp_linger, read_buf, seek_stream_len)
)]
#![cfg_attr(target_os = "linux", feature(tcp_deferaccept))]

#[macro_export]
macro_rules! coco {
    ($c:stmt) => {
        $crate::add(async { $c });
    };
    ($c:block) => {
        $crate::add(async $c);
    };
}

pub mod future;
pub mod runtime;
pub mod signals;
pub mod stxa;

pub use runtime::{add, controller, link, run, run_one, run_until_empty, run_while};

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{
    //    $crate::p1!("{} {:?} | ", $crate::Time::now(), $crate::current().id());
    //    $crate::p2!($($arg)*);
    }};
}

extern crate xrmt_stx;
pub use xrmt_stx::thread::current;
pub use xrmt_stx::time::extra::Time;
pub use xrmt_stx::{print as p1, println as p2};
