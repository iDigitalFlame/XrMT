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

use crate::sync::{Event, Mutex};
use crate::util::stx;

pub struct WaitGroup {
    lock:  Mutex<usize>,
    inner: Event,
}

impl WaitGroup {
    #[inline]
    pub const fn new() -> WaitGroup {
        WaitGroup {
            lock:  Mutex::new(0),
            inner: Event::new(),
        }
    }

    #[inline]
    pub fn wait(&self) {
        self.inner.wait()
    }
    #[inline]
    pub fn done(&self) {
        // NOTE(dij): We panic here as this shouldn't happen, hopefully.
        let mut i = stx::unwrap(self.lock.lock());
        *i = (*i).saturating_sub(1);
        if *i == 0 {
            // NOTE(dij): We panic here as this shouldn't happen, hopefully.
            stx::unwrap(self.inner.signal())
        }
    }
    #[inline]
    pub fn add(&self, delta: usize) {
        let mut i = stx::unwrap(self.lock.lock());
        *i = (*i).saturating_add(delta);
    }
}
