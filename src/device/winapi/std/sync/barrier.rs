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

use core::fmt::{self, Debug, Formatter};
use core::sync::atomic::{self, Ordering};

use crate::sync::{Event, Mutex};
use crate::util::stx;
use crate::util::stx::prelude::*;

pub struct Barrier {
    lock:  Mutex<(usize, usize)>,
    inner: Event,
    limit: usize,
}
pub struct BarrierWaitResult(bool);

impl Barrier {
    #[inline]
    pub fn new(n: usize) -> Barrier {
        Barrier {
            lock:  Mutex::new((0, 0)),
            inner: Event::new(),
            limit: n,
        }
    }

    pub fn wait(&self) -> BarrierWaitResult {
        let s = {
            // NOTE(dij): We panic here as this shouldn't happen, hopefully.
            let mut i = stx::unwrap(self.lock.lock());
            i.0 += 1;
            if i.0 < self.limit {
                false
            } else {
                i.0 = 0;
                i.1 = i.1.wrapping_add(1);
                true
            }
        };
        atomic::fence(Ordering::SeqCst);
        if s {
            // NOTE(dij): We panic here as this shouldn't happen, hopefully.
            stx::unwrap(self.inner.signal());
        } else {
            self.inner.wait()
        }
        BarrierWaitResult(s)
    }
}
impl BarrierWaitResult {
    #[inline]
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

impl Debug for BarrierWaitResult {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
