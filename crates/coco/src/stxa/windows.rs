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

extern crate xrmt_stx;
extern crate xrmt_winapi;

use core::convert::From;
use core::default::Default;
use core::future::Future;
use core::option::Option::{self, None, Some};
use core::pin::Pin;
use core::result::Result::{Err, Ok};
use core::task::{Context, Poll};

use xrmt_stx::io::{IoError, IoResult};
use xrmt_stx::os::Handle;
use xrmt_winapi::functions::{NtReadFile, NtWriteFile};
use xrmt_winapi::structs::Overlapped;
use xrmt_winapi::Win32Error;

use crate::future::{State, Status};

#[path = "windows/fs.rs"]
mod fs;
#[path = "windows/net.rs"]
mod net;

pub use self::fs::*;
pub use self::net::*;

pub struct AsyncReader<'a> {
    h:   &'a Handle,
    buf: &'a mut [u8],
    olp: Overlapped,
}
pub struct AsyncWriter<'a> {
    h:   &'a Handle,
    buf: &'a [u8],
    olp: Overlapped,
}

impl<'a> AsyncReader<'a> {
    #[inline]
    pub fn new(h: &'a Handle, buf: &'a mut [u8]) -> AsyncReader<'a> {
        AsyncReader {
            h,
            buf,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn read(&mut self) -> IoResult<Option<usize>> {
        match NtReadFile(self.h, Some(&mut self.olp), self.buf, None) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
    #[inline]
    fn result(&mut self) -> IoResult<Option<usize>> {
        match self.olp.status_no_wait() {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
}
impl<'a> AsyncWriter<'a> {
    #[inline]
    pub fn new(h: &'a Handle, buf: &'a [u8]) -> AsyncWriter<'a> {
        AsyncWriter {
            h,
            buf,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn write(&mut self) -> IoResult<Option<usize>> {
        match NtWriteFile(self.h, Some(&mut self.olp), self.buf, None) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
    #[inline]
    fn result(&mut self) -> IoResult<Option<usize>> {
        match self.olp.status_no_wait() {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => Ok(Some(n)),
        }
    }
}

impl Future for AsyncReader<'_> {
    type Output = IoResult<usize>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<usize>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) if v.is_timeout() => State::timeout(),
            Status::Ready(_) => State::make_poll(self.result()),
            Status::Setup(v) => {
                if let Err(e) = v.register_queue(false, self.h) {
                    return Poll::Ready(Err(e));
                }
                self.olp.event = *v.iocp_handle();
                State::make_poll(self.read())
            },
        }
    }
}
impl Future for AsyncWriter<'_> {
    type Output = IoResult<usize>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<usize>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) if v.is_timeout() => State::timeout(),
            Status::Ready(_) => State::make_poll(self.result()),
            Status::Setup(v) => {
                if let Err(e) = v.register_queue(false, self.h) {
                    return Poll::Ready(Err(e));
                }
                self.olp.event = *v.iocp_handle();
                State::make_poll(self.write())
            },
        }
    }
}
