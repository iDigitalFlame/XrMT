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

use core::cell::UnsafeCell;
use core::time::Duration;

use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle};
use crate::ignore_error;
use crate::io::{self, Error, ErrorKind};
use crate::prelude::*;
use crate::sync::Lazy;

pub struct Event {
    lazy:   Lazy,
    handle: UnsafeCell<OwnedHandle>,
}

impl Event {
    #[inline]
    pub const fn new() -> Event {
        Event {
            lazy:   Lazy::new(),
            handle: UnsafeCell::new(OwnedHandle::empty()),
        }
    }

    #[inline]
    pub fn open(n: impl AsRef<str>) -> io::Result<Event> {
        Ok(Event {
            lazy:   Lazy::new_ready(),
            // 0x1F0003 - FULL_CONTROL
            handle: UnsafeCell::new(winapi::OpenEvent(0x1F0003, false, n.as_ref()).map_err(Error::from)?),
        })
    }
    #[inline]
    pub fn named(n: impl AsRef<str>) -> io::Result<Event> {
        Ok(Event {
            lazy:   Lazy::new_ready(),
            handle: UnsafeCell::new(winapi::CreateEvent(None, false, false, false, n.as_ref()).map_err(Error::from)?),
        })
    }

    #[inline]
    pub fn wait(&self) {
        self.init(false);
        ignore_error!(winapi::WaitForSingleObject(self, -1, false));
    }
    #[inline]
    pub fn set_ignore(&self) {
        if self.init(true) {
            return;
        }
        ignore_error!(winapi::SetEvent(self));
    }
    #[inline]
    pub fn reset_ignore(&self) {
        if self.init(false) {
            return;
        }
        ignore_error!(winapi::ResetEvent(self));
    }
    #[inline]
    pub fn signal_ignore(&self) {
        if !self.init(true) {
            ignore_error!(self.set());
        }
        ignore_error!(self.reset());
    }
    #[inline]
    pub fn is_set(&self) -> bool {
        self.lazy.is_ready() && winapi::QueryEvent(self).map_or(false, |v| v > 0)
    }
    #[inline]
    pub fn set(&self) -> io::Result<()> {
        if self.init(true) {
            Ok(())
        } else {
            winapi::SetEvent(self).map_err(Error::from)
        }
    }
    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        if self.init(false) {
            Ok(())
        } else {
            winapi::ResetEvent(self).map_err(Error::from)
        }
    }
    #[inline]
    pub fn signal(&self) -> io::Result<()> {
        if !self.init(true) {
            self.set()?;
        }
        self.reset()
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> io::Result<()> {
        self.init(false);
        winapi::WaitForSingleObject(self, d.as_micros() as i32, false)
            .map_err(Error::from)
            .and_then(|v| match v {
                0xC0 => Err(ErrorKind::Interrupted.into()), // STATUS_USER_APC
                0 => Ok(()),
                _ => Err(ErrorKind::TimedOut.into()),
            })
    }

    #[inline]
    fn init(&self, initial: bool) -> bool {
        self.lazy
            .load(|| unsafe { *self.handle.get() = unwrap(winapi::CreateEvent(None, false, initial, false, None)) })
    }
}

impl AsHandle for Event {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.init(false);
        unsafe { *(*self.handle.get()) }
    }
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}
