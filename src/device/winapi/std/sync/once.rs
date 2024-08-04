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
#![cfg(target_family = "windows")]

use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::prelude::*;
use crate::sync::{Lazy, Mutant};

pub struct Once {
    lazy: Lazy,
    lock: Mutant,
}
pub struct OnceState;

impl Once {
    #[inline]
    pub const fn new() -> Once {
        Once {
            lazy: Lazy::new(),
            lock: Mutant::new(),
        }
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        self.lazy.is_ready()
    }
    #[track_caller]
    #[inline]
    pub fn call_once(&self, f: impl FnOnce()) {
        if self.lazy.is_ready() {
            return;
        }
        // NOTE(dij): We panic here as this shouldn't happen. This is a panic as
        //            the lock shouldn't fail.
        unwrap_unlikely(self.lock.lock());
        if self.lazy.load(f) {
            // NOTE(dij): We panic here as this shouldn't happen, hopefully.
            unwrap_unlikely(self.lock.unlock());
            // Close the Mutant Handle, we no longer need it.
            unsafe { self.lock.close() };
        }
    }
    #[inline]
    pub fn call_once_force(&self, f: impl FnOnce(&OnceState)) {
        if self.lazy.is_ready() {
            return;
        }
        // NOTE(dij): We panic here as this shouldn't happen. This is a panic as
        //            the lock shouldn't fail.
        unwrap_unlikely(self.lock.lock());
        if self.lazy.load(|| f(&OnceState)) {
            // NOTE(dij): We panic here as this shouldn't happen, hopefully.
            unwrap_unlikely(self.lock.unlock());
            // Close the Mutant Handle, we no longer need it.
            unsafe { self.lock.close() };
        }
    }
}
impl OnceState {
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        false
    }
}

impl UnwindSafe for Once {}
impl RefUnwindSafe for Once {}
