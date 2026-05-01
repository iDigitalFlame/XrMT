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

use core::ops::SubAssign;
use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::abort_unlikely;
use crate::sync::extra::EventConstant;
use crate::sync::Mutex;

pub struct WaitGroup {
    v: Mutex<usize>,
    e: EventConstant,
}

impl WaitGroup {
    #[inline]
    pub const fn new() -> WaitGroup {
        WaitGroup {
            v: Mutex::new(0),
            e: EventConstant::new(),
        }
    }

    #[inline]
    pub fn wait(&self) {
        self.e.wait()
    }
    #[inline]
    pub fn done(&self) {
        let mut i = abort_unlikely!(self.v.lock());
        *i = (*i).saturating_sub(1);
        if *i == 0 {
            let _ = self.e.set();
        }
    }
    #[inline]
    pub fn add(&self, delta: usize) {
        abort_unlikely!(self.v.lock()).sub_assign(delta);
    }
}

impl UnwindSafe for WaitGroup {}
impl RefUnwindSafe for WaitGroup {}
