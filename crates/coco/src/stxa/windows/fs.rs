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

use core::convert::{AsRef, From};
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
use crate::stxa::fs::File;

pub struct AsyncFileReader<'a> {
    h:   &'a Handle,
    buf: &'a mut [u8],
    pos: &'a mut u64,
    olp: Overlapped,
}
pub struct AsyncFileWriter<'a> {
    h:   &'a Handle,
    buf: &'a [u8],
    pos: &'a mut u64,
    olp: Overlapped,
}

impl<'a> AsyncFileReader<'a> {
    #[inline]
    pub fn file(f: &'a mut File, buf: &'a mut [u8]) -> AsyncFileReader<'a> {
        AsyncFileReader {
            buf,
            h: f.v.as_ref(),
            pos: &mut f.p,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn read(&mut self) -> IoResult<Option<usize>> {
        match NtReadFile(self.h, Some(&mut self.olp), self.buf, Some(*self.pos)) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => {
                *self.pos += n as u64;
                Ok(Some(n))
            },
        }
    }
    #[inline]
    fn result(&mut self) -> IoResult<Option<usize>> {
        match self.olp.status_no_wait() {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => {
                *self.pos += n as u64;
                Ok(Some(n))
            },
        }
    }
}
impl<'a> AsyncFileWriter<'a> {
    #[inline]
    pub fn file(f: &'a mut File, buf: &'a [u8]) -> AsyncFileWriter<'a> {
        AsyncFileWriter {
            buf,
            h: f.v.as_ref(),
            pos: &mut f.p,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn write(&mut self) -> IoResult<Option<usize>> {
        match NtWriteFile(self.h, Some(&mut self.olp), self.buf, Some(*self.pos)) {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => {
                *self.pos += n as u64;
                Ok(Some(n))
            },
        }
    }
    #[inline]
    fn result(&mut self) -> IoResult<Option<usize>> {
        match self.olp.status_no_wait() {
            Err(Win32Error::IoPending) => Ok(None),
            Err(Win32Error::BrokenPipe) => Ok(Some(0)),
            Err(e) => Err(IoError::from(e)),
            Ok(n) => {
                *self.pos += n as u64;
                Ok(Some(n))
            },
        }
    }
}

impl Future for AsyncFileReader<'_> {
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
impl Future for AsyncFileWriter<'_> {
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
