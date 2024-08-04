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

use core::time::Duration;

use crate::data::time::Time;
use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle};
use crate::ignore_error;
use crate::io::{self, Error, ErrorKind};
use crate::prelude::*;

pub struct Timer(OwnedHandle);
pub struct TimeIntoIter(Timer);
pub struct TimeIter<'a>(&'a Timer);

impl Timer {
    #[inline]
    pub fn new() -> io::Result<Timer> {
        Ok(Timer(winapi::CreateWaitableTimer(
            None, false, false, None,
        )?))
    }
    #[inline]
    pub fn open(n: impl AsRef<str>) -> io::Result<Timer> {
        Ok(Timer(
            // 0x1F0003 - FULL_CONTROL
            winapi::OpenWaitableTimer(0x1F0003, false, n.as_ref())?,
        ))
    }
    #[inline]
    pub fn named(n: impl AsRef<str>) -> io::Result<Timer> {
        Ok(Timer(winapi::CreateWaitableTimer(
            None,
            false,
            false,
            n.as_ref(),
        )?))
    }

    #[inline]
    pub fn wait(&self) {
        ignore_error!(winapi::WaitForSingleObject(self, -1, false));
    }
    #[inline]
    pub fn enabled(&self) -> bool {
        winapi::QueryWaitableTimer(self).map_or(false, |v| v.0)
    }
    #[inline]
    pub fn stop(&self) -> io::Result<bool> {
        winapi::CancelWaitableTimer(self).map_err(Error::from)
    }
    #[inline]
    pub fn iter<'a>(&'a self) -> TimeIter<'a> {
        TimeIter(self)
    }
    #[inline]
    pub fn period(&self) -> io::Result<(bool, u64)> {
        winapi::QueryWaitableTimer(self).map_err(Error::from)
    }
    #[inline]
    pub fn start(&self, d: Duration) -> io::Result<()> {
        self.start_repeating_split(d, Duration::ZERO)
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> io::Result<()> {
        self.wait_alert_for(false, d)
    }
    #[inline]
    pub fn start_repeating(&self, d: Duration) -> io::Result<()> {
        self.start_repeating_split(d, d)
    }
    #[inline]
    pub fn wait_alert_for(&self, alertable: bool, d: Duration) -> io::Result<()> {
        winapi::WaitForSingleObject(self, d.as_micros() as i32, alertable)
            .map_err(Error::from)
            .and_then(|v| match v {
                0xC0 => Err(ErrorKind::Interrupted.into()), // STATUS_USER_APC
                0 => Ok(()),
                _ => Err(ErrorKind::TimedOut.into()),
            })
    }
    #[inline]
    pub fn start_repeating_split(&self, initial: Duration, repeat: Duration) -> io::Result<()> {
        winapi::SetWaitableTimer(
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

impl AsHandle for Timer {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.0
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
        winapi::WaitForSingleObject(&self.0, -1, false).map_or(None, |r| if r == 0 { Some(Time::now()) } else { None })
    }
}
impl Iterator for TimeIntoIter {
    type Item = Time;

    #[inline]
    fn next(&mut self) -> Option<Time> {
        winapi::WaitForSingleObject(&self.0, -1, false).map_or(None, |r| if r == 0 { Some(Time::now()) } else { None })
    }
}
