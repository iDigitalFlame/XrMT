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

use core::cell::UnsafeCell;
use core::time::Duration;

use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle};
use crate::sync::Lazy;
use crate::util::stx;
use crate::util::stx::io::{self, Error, ErrorKind};
use crate::util::stx::prelude::*;

pub struct Mutant {
    lazy:   Lazy,
    handle: UnsafeCell<OwnedHandle>,
}

impl Mutant {
    #[inline]
    pub const fn new() -> Mutant {
        Mutant {
            lazy:   Lazy::new(),
            handle: UnsafeCell::new(OwnedHandle::empty()),
        }
    }

    #[inline]
    pub fn open(n: impl AsRef<str>) -> io::Result<Mutant> {
        Ok(Mutant {
            lazy:   Lazy::new_ready(),
            // 0x1F0003 - FULL_CONTROL
            handle: UnsafeCell::new(winapi::OpenMutex(0x1F0001, false, n.as_ref()).map_err(Error::from)?),
        })
    }
    #[inline]
    pub fn named(n: impl AsRef<str>) -> io::Result<Mutant> {
        Ok(Mutant {
            lazy:   Lazy::new_ready(),
            handle: UnsafeCell::new(winapi::CreateMutex(None, false, false, n.as_ref()).map_err(Error::from)?),
        })
    }

    #[inline]
    pub unsafe fn close(&self) {
        if self.lazy.is_ready() {
            winapi::close_handle(*(*self.handle.get()));
            unsafe { (*self.handle.get()).set(0) };
        } else {
            self.lazy.force()
        }
    }

    #[inline]
    pub fn is_locked(&self) -> bool {
        self.lazy.is_ready() && winapi::QueryMutex(self).map_err(Error::from).map_or(false, |v| v > 0)
    }
    #[inline]
    pub fn lock(&self) -> io::Result<()> {
        self.init(false);
        winapi::WaitForSingleAsHandle(self, -1, false)
            .map_err(Error::from)
            .and_then(|v| match v {
                0xC0 => Err(ErrorKind::Interrupted.into()), // STATUS_USER_APC
                0 => Ok(()),
                _ => Err(ErrorKind::TimedOut.into()),
            })
    }
    #[inline]
    pub fn unlock(&self) -> io::Result<()> {
        if self.init(false) {
            Ok(())
        } else {
            winapi::ReleaseMutex(self).map_err(Error::from)
        }
    }
    #[inline]
    pub fn try_lock(&self) -> io::Result<bool> {
        if self.init(true) {
            Ok(true)
        } else {
            Ok(winapi::WaitForSingleAsHandle(self, 0, false).map_err(Error::from)? == 0)
        }
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> io::Result<()> {
        if self.lazy.is_ready() {
            winapi::WaitForSingleAsHandle(self, d.as_micros() as i32, false)
                .map_err(Error::from)
                .and_then(|v| match v {
                    0xC0 => Err(ErrorKind::Interrupted.into()), // STATUS_USER_APC
                    0 => Ok(()),
                    _ => Err(ErrorKind::TimedOut.into()),
                })
        } else {
            Ok(())
        }
    }

    #[inline]
    fn init(&self, initial: bool) -> bool {
        self.lazy
            .load(|| unsafe { *self.handle.get() = stx::unwrap(winapi::CreateMutex(None, false, initial, None)) })
    }
}

impl AsHandle for Mutant {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.init(false);
        unsafe { *(*self.handle.get()) }
    }
}

unsafe impl Send for Mutant {}
unsafe impl Sync for Mutant {}
