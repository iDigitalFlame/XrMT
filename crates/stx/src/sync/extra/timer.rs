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

extern crate xrmt_winapi;

use core::convert::{AsRef, From};
use core::iter::{IntoIterator, Iterator};
use core::marker::{Send, Sync};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{Err, Ok};
use core::time::Duration;

use xrmt_winapi::functions::{duration_to_micros, CancelWaitableTimer, CreateWaitableTimer, OpenWaitableTimer, QueryWaitableTimer, SetWaitableTimer, WaitForSingleObject};
use xrmt_winapi::structs::OwnedHandle;
use xrmt_winapi::INFINITE;

use crate::io::{ErrorKind, IoError, IoResult};
use crate::os::Handle;
use crate::time::extra::Time;

pub struct Timer(OwnedHandle);
pub struct TimeIntoIter(Timer);
pub struct TimeIter<'a>(&'a Timer);

impl Timer {
    #[inline]
    pub fn new() -> IoResult<Timer> {
        Ok(Timer(CreateWaitableTimer(None, false, false, None)?))
    }
    #[inline]
    pub fn open(n: impl AsRef<str>) -> IoResult<Timer> {
        // 0x1F0003 - FULL_CONTROL
        Ok(Timer(OpenWaitableTimer(0x1F0003, false, n.as_ref())?))
    }
    #[inline]
    pub fn new_with_name(n: impl AsRef<str>) -> IoResult<Timer> {
        Ok(Timer(CreateWaitableTimer(None, false, false, n.as_ref())?))
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        QueryWaitableTimer(&self.0).map_or(false, |v| v.0)
    }
    #[inline]
    pub fn wait(&self) -> IoResult<()> {
        let _ = WaitForSingleObject(&self.0, INFINITE, false)?;
        Ok(())
    }
    #[inline]
    pub fn stop(&self) -> IoResult<bool> {
        Ok(CancelWaitableTimer(&self.0)?)
    }
    #[inline]
    pub fn iter<'a>(&'a self) -> TimeIter<'a> {
        TimeIter(self)
    }
    #[inline]
    pub fn period(&self) -> IoResult<(bool, u64)> {
        Ok(QueryWaitableTimer(&self.0)?)
    }
    #[inline]
    pub fn start(&self, d: Duration) -> IoResult<()> {
        self.start_repeating_split(d, Duration::ZERO)
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> IoResult<()> {
        self.wait_for_alert(false, d)
    }
    #[inline]
    pub fn start_repeating(&self, d: Duration) -> IoResult<()> {
        self.start_repeating_split(d, d)
    }
    #[inline]
    pub fn wait_for_alert(&self, alertable: bool, d: Duration) -> IoResult<()> {
        match WaitForSingleObject(&self.0, duration_to_micros(d), alertable)? {
            0xC0 => Err(IoError::from(ErrorKind::Interrupted)), // STATUS_USER_APC
            0 => Ok(()),
            _ => Err(IoError::from(ErrorKind::TimedOut)),
        }
    }
    #[inline]
    pub fn start_repeating_split(&self, initial: Duration, repeat: Duration) -> IoResult<()> {
        SetWaitableTimer(
            self,
            initial.as_micros() as u64,
            repeat.as_millis() as u32,
            None,
            None,
            false,
        )?;
        Ok(())
    }
}

impl AsRef<Handle> for Timer {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl IntoIterator for Timer {
    type Item = Time;
    type IntoIter = TimeIntoIter;

    #[inline]
    fn into_iter(self) -> TimeIntoIter {
        TimeIntoIter(self)
    }
}

impl Iterator for TimeIter<'_> {
    type Item = Time;

    #[inline]
    fn next(&mut self) -> Option<Time> {
        if WaitForSingleObject(&self.0, INFINITE, false).ok()? == 0 {
            Some(Time::now())
        } else {
            None
        }
    }
}
impl Iterator for TimeIntoIter {
    type Item = Time;

    #[inline]
    fn next(&mut self) -> Option<Time> {
        if WaitForSingleObject(&self.0, INFINITE, false).ok()? == 0 {
            Some(Time::now())
        } else {
            None
        }
    }
}

impl UnwindSafe for Timer {}
impl RefUnwindSafe for Timer {}

unsafe impl Send for Timer {}
unsafe impl Sync for Timer {}
