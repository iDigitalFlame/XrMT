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

extern crate core;

extern crate libc;
extern crate xrmt_stx;

use core::future::Future;
use core::option::Option::{self, None, Some};
use core::pin::Pin;
use core::result::Result::{Err, Ok};
use core::task::{Context, Poll};

use libc::{read, write, EAGAIN, EINPROGRESS, EWOULDBLOCK};
use xrmt_stx::io::{IoError, IoResult};
use xrmt_stx::os::{AsFdRef, Handle};

use crate::future::{State, Status};
use crate::stxa::fs::File;

#[path = "unix/fs.rs"]
mod fs;
#[path = "unix/net.rs"]
mod net;

pub use self::fs::*;
pub use self::net::*;

pub struct ErrNo(i32);
pub struct AsyncReader<'a> {
    h:   &'a Handle,
    buf: &'a mut [u8],
}
pub struct AsyncWriter<'a> {
    h:   &'a Handle,
    buf: &'a [u8],
}

impl ErrNo {
    #[inline]
    pub fn get() -> ErrNo {
        ErrNo(error::errorno())
    }

    #[inline]
    pub fn throw(self) -> IoError {
        IoError::from_raw_os_error(self.0)
    }
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub fn is_blocking(&self) -> bool {
        self.0 == EAGAIN || self.0 == EINPROGRESS || self.0 == EWOULDBLOCK
    }

    #[cfg(not(feature = "strip"))]
    #[inline]
    fn err(&self) -> IoError {
        IoError::from_raw_os_error(self.0)
    }
}
impl<'a> AsyncReader<'a> {
    #[inline]
    pub fn file(f: &'a File, buf: &'a mut [u8]) -> AsyncReader<'a> {
        AsyncReader { h: f.v.as_ref(), buf }
    }
    #[inline]
    pub fn new(h: &'a Handle, buf: &'a mut [u8]) -> AsyncReader<'a> {
        AsyncReader { h, buf }
    }

    fn read(&mut self) -> IoResult<Option<usize>> {
        let mut n = 0usize;
        while n < self.buf.len() {
            let v = unsafe {
                read(
                    **self.h,
                    self.buf.get_unchecked_mut(n..).as_ptr() as _,
                    self.buf.len().saturating_sub(n),
                )
            };
            let e = ErrNo::get();
            if e.is_blocking() && v == -1 {
                if n > 0 {
                    break;
                }
                // If the read would block and we don't have anything read so far
                // return None instead of Ok(0) which means EOF.
                return Ok(None);
            }
            if e.is_valid() && !e.is_blocking() {
                return Err(e.throw());
            }
            if v <= 0 {
                break;
            }
            n += v as usize;
        }
        Ok(Some(n))
    }
}
impl<'a> AsyncWriter<'a> {
    #[inline]
    pub fn file(f: &'a File, buf: &'a [u8]) -> AsyncWriter<'a> {
        AsyncWriter { h: f.v.as_ref(), buf }
    }
    #[inline]
    pub fn new(h: &'a Handle, buf: &'a [u8]) -> AsyncWriter<'a> {
        AsyncWriter { h, buf }
    }

    fn write(&mut self) -> IoResult<Option<usize>> {
        let mut n = 0usize;
        while n < self.buf.len() {
            let v = unsafe {
                write(
                    **self.h,
                    self.buf.get_unchecked(n..).as_ptr() as _,
                    self.buf.len().saturating_sub(n),
                )
            };
            let e = ErrNo::get();
            if e.is_blocking() && v == -1 {
                if n > 0 {
                    break;
                }
                // If the write would block and we don't have anything written so far
                // return None instead of Ok(0) which means EOF.
                return Ok(None);
            }
            if e.is_valid() && !e.is_blocking() {
                return Err(e.throw());
            }
            if v <= 0 {
                break;
            }
            n += v as usize;
        }
        Ok(Some(n))
    }
}

impl Future for AsyncReader<'_> {
    type Output = IoResult<usize>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<usize>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(_) => State::make_poll(self.read()),
            Status::Setup(v) => {
                if let Err(e) = v.register_queue(false, self.h) {
                    return Poll::Ready(Err(e));
                }
                // TODO(dij): We can do a fastpath, here but that might cause
                //            starvation issues?
                Poll::Pending
            },
        }
    }
}
impl Future for AsyncWriter<'_> {
    type Output = IoResult<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<usize>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(_) => State::make_poll(self.write()),
            Status::Setup(v) => {
                if let Err(e) = v.register_queue(true, self.h) {
                    return Poll::Ready(Err(e));
                }
                // TODO(dij): We can do a fastpath, here but that might cause
                //            starvation issues?
                Poll::Pending
            },
        }
    }
}

#[cfg(any(
    target_os = "netbsd",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
))]
mod error {
    extern crate libc;

    #[inline]
    pub fn errorno() -> i32 {
        #[cfg(any(target_os = "android", target_os = "netbsd"))]
        unsafe {
            *libc::__errno()
        }
        #[cfg(all(not(target_os = "android"), not(target_os = "netbsd")))]
        unsafe {
            *libc::___errno()
        }
    }
}
#[cfg(any(target_os = "freebsd", target_vendor = "apple"))]
mod error {
    extern crate libc;

    use libc::__error;

    #[inline]
    pub fn errorno() -> i32 {
        unsafe { *__error() }
    }
}
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "android"),
    not(target_os = "freebsd"),
    not(target_os = "illumos"),
    not(target_os = "solaris"),
    not(target_vendor = "apple")
))]
mod error {
    extern crate libc;

    use libc::__errno_location;

    #[inline]
    pub fn errorno() -> i32 {
        unsafe { *__errno_location() }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::write;

    use crate::stxa::ErrNo;

    impl Debug for ErrNo {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "ErrNo {}: {}", self.0, self.err())
        }
    }
}
