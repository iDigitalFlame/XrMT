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
#![cfg(windows)]

use core::sync::atomic::{AtomicU8, Ordering};
use core::{hint, ptr};

use crate::util::stx::prelude::*;

const STATE_NEW: u8 = 0;
const STATE_INIT: u8 = 1;
const STATE_READY: u8 = 2;

pub struct Lazy(AtomicU8);

impl Lazy {
    #[inline]
    pub const fn new() -> Lazy {
        Lazy(AtomicU8::new(STATE_NEW))
    }
    #[inline]
    pub const fn new_ready() -> Lazy {
        Lazy(AtomicU8::new(STATE_READY))
    }

    #[inline]
    pub fn force(&self) {
        self.0.store(STATE_READY, Ordering::Release);
    }
    #[inline]
    pub fn is_new(&self) -> bool {
        self.0.load(Ordering::Acquire) == STATE_NEW
    }
    #[inline]
    pub fn is_init(&self) -> bool {
        self.0.load(Ordering::Acquire) == STATE_INIT
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.0.load(Ordering::Acquire) == STATE_READY
    }
    pub fn load(&self, f: impl FnOnce()) -> bool {
        match self
            .0
            .compare_exchange(STATE_NEW, STATE_INIT, Ordering::AcqRel, Ordering::Relaxed)
        {
            Ok(_) => {
                f();
                self.0.store(STATE_READY, Ordering::Release);
                return true;
            },
            Err(v) => match v {
                STATE_NEW => core::unreachable!(),
                STATE_INIT => {
                    while self.0.load(Ordering::Acquire) == STATE_INIT {
                        unsafe { ptr::read_volatile(&self.0) }; // Prevent optimization of loop.
                        hint::spin_loop()
                    }
                },
                _ => (),
            },
        };
        false
    }
}
