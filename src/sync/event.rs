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
#![cfg(not(target_family = "windows"))]

use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use std::sync::{Condvar, Mutex};

use crate::io::{self, ErrorKind};
use crate::prelude::*;

pub struct Event {
    set:   AtomicBool,
    cond:  Condvar,
    mutex: Mutex<bool>,
}

impl Event {
    #[inline]
    pub const fn new() -> Event {
        Event {
            set:   AtomicBool::new(false),
            cond:  Condvar::new(),
            mutex: Mutex::new(false),
        }
    }

    #[inline]
    pub fn wait(&self) {
        if self.set.load(Ordering::Acquire) {
            return;
        }
        let _unused = self.cond.wait(unwrap_unlikely(self.mutex.lock()));
    }
    #[inline]
    pub fn set_ignore(&self) {
        self.set.store(true, Ordering::Release);
        self.cond.notify_all()
    }
    #[inline]
    pub fn reset_ignore(&self) {
        self.set.store(false, Ordering::Release)
    }
    #[inline]
    pub fn signal_ignore(&self) {
        if !self.set.load(Ordering::Acquire) {
            self.set.store(true, Ordering::Release);
            self.cond.notify_all()
        }
        self.set.store(false, Ordering::Release)
    }
    #[inline]
    pub fn is_set(&self) -> bool {
        self.set.load(Ordering::Acquire)
    }
    #[inline]
    pub fn set(&self) -> io::Result<()> {
        self.set_ignore();
        Ok(())
    }
    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        self.reset_ignore();
        Ok(())
    }
    #[inline]
    pub fn signal(&self) -> io::Result<()> {
        self.signal_ignore();
        Ok(())
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> io::Result<()> {
        match self.cond.wait_timeout(unwrap_unlikely(self.mutex.lock()), d) {
            Ok(r) => {
                if r.1.timed_out() {
                    Err(ErrorKind::TimedOut.into())
                } else {
                    Ok(())
                }
            },
            Err(_) => Ok(()),
        }
    }
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}
