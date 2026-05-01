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

// Initial Attributes
// ==============================
#![no_implicit_prelude]
#![feature(
    allocator_api,
    layout_for_ptr,
    box_as_ptr,
    likely_unlikely,
    coroutines,
    unchecked_shifts
)]
#![cfg_attr(windows, no_std, no_main)]

extern crate xrmt;

mod testing;

#[cfg(unix)]
#[inline]
fn main() {
    testing::main();
}

#[cfg(windows)]
#[unsafe(no_mangle)]
extern "C" fn _main() {
    xrmt::runtime::run(testing::main)
}
