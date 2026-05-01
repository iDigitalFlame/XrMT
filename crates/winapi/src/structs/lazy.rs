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
#![cfg(target_family = "windows")]

extern crate core;

use core::hint::{spin_loop, unreachable_unchecked};
use core::ops::FnOnce;
use core::ptr::read_volatile;
use core::result::Result::{Err, Ok};
use core::sync::atomic::{fence, AtomicU8, Ordering};

const STATE_NEW: u8 = 0u8;
const STATE_INIT: u8 = 1u8;
const STATE_READY: u8 = 2u8;

pub struct Lazy(AtomicU8);

impl Lazy {
    #[inline]
    pub const fn new() -> Lazy {
        Lazy(AtomicU8::new(STATE_NEW))
    }

    #[inline]
    pub fn force(&self) {
        self.0.store(STATE_READY, Ordering::Release);
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.0.load(Ordering::Acquire) == STATE_READY
    }
    pub fn load(&self, f: impl FnOnce()) -> bool {
        fence(Ordering::SeqCst);
        match self
            .0
            .compare_exchange(STATE_NEW, STATE_INIT, Ordering::AcqRel, Ordering::Relaxed)
        {
            Ok(_) => {
                f();
                self.0.store(STATE_READY, Ordering::Release);
                true
            },
            Err(STATE_NEW) => unsafe { unreachable_unchecked() }, // Can never hit here.
            Err(STATE_INIT) => {
                // Spin while waiting..
                while self.0.load(Ordering::Acquire) == STATE_INIT {
                    let _ = unsafe { read_volatile(&self.0) }; // Prevent optimization of loop.
                    spin_loop()
                }
                false
            },
            Err(STATE_READY) => false,
            Err(_) => unsafe { unreachable_unchecked() }, // Can never hit here.
        }
    }
}
