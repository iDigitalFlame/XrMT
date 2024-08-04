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

use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle};
use crate::ignore_error;
use crate::io::{self, Error, ErrorKind};
use crate::prelude::*;

pub struct Semaphore(OwnedHandle);

impl Semaphore {
    #[inline]
    pub fn new(limit: u32) -> io::Result<Semaphore> {
        Semaphore::starting_with(limit, limit)
    }
    #[inline]
    pub fn open(n: impl AsRef<str>) -> io::Result<Semaphore> {
        Ok(Semaphore(
            // 0x1F0003 - FULL_CONTROL
            winapi::OpenSemaphore(0x1F0003, false, n.as_ref())?,
        ))
    }
    #[inline]
    pub fn starting_with(limit: u32, start: u32) -> io::Result<Semaphore> {
        Ok(Semaphore(winapi::CreateSemaphore(
            None, false, start, limit, None,
        )?))
    }
    #[inline]
    pub fn named(limit: u32, n: impl AsRef<str>) -> io::Result<Semaphore> {
        Semaphore::named_starting_with(limit, limit, n)
    }
    #[inline]
    pub fn named_starting_with(limit: u32, start: u32, n: impl AsRef<str>) -> io::Result<Semaphore> {
        Ok(Semaphore(winapi::CreateSemaphore(
            None,
            false,
            start,
            limit,
            n.as_ref(),
        )?))
    }

    #[inline]
    pub fn wait(&self) {
        ignore_error!(winapi::WaitForSingleObject(self, -1, false));
    }
    #[inline]
    pub fn limit(&self) -> u32 {
        winapi::QuerySemaphore(self).map_err(Error::from).map_or(0, |v| v.1)
    }
    #[inline]
    pub fn current(&self) -> u32 {
        winapi::QuerySemaphore(self).map_err(Error::from).map_or(0, |v| v.0)
    }
    #[inline]
    pub fn stats(&self) -> (u32, u32) {
        winapi::QuerySemaphore(self).map_err(Error::from).unwrap_or((0, 0))
    }
    #[inline]
    pub fn release(&self) -> io::Result<u32> {
        winapi::ReleaseSemaphore(self, 1).map_err(Error::from)
    }
    #[inline]
    pub fn wait_for(&self, d: Duration) -> io::Result<()> {
        winapi::WaitForSingleObject(self, d.as_micros() as i32, false)
            .map_err(Error::from)
            .and_then(|v| match v {
                0xC0 => Err(ErrorKind::Interrupted.into()), // STATUS_USER_APC
                0 => Ok(()),
                _ => Err(ErrorKind::TimedOut.into()),
            })
    }
}

impl AsHandle for Semaphore {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0.as_handle()
    }
}
