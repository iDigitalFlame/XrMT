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

use core::convert::Into;
use core::future::Future;
use core::net::SocketAddr;
use core::pin::Pin;
use core::result::Result::{Err, Ok};
use core::task::{Context, Poll};
use core::time::Duration;

use libc::{connect, fcntl, socket, AF_INET, AF_INET6, F_SETFL, O_CLOEXEC, O_NONBLOCK, SOCK_STREAM};
use xrmt_stx::io::IoResult;
use xrmt_stx::os::fd::OwnedFd;
use xrmt_stx::os::{Handle, OwnedHandle};

use crate::errcheck;
use crate::future::{State, Status};
use crate::stxa::{AsyncReader, AsyncWriter, ErrNo};

pub enum SockState {
    Ok(OwnedFd),
    Wait(AsyncConnect),
}

pub struct AsyncConnect {
    fd:   OwnedHandle,
    dur:  Duration,
    addr: SockAddr,
    size: u8,
}

pub type AsyncSockReader<'a> = AsyncReader<'a>;
pub type AsyncSockWriter<'a> = AsyncWriter<'a>;

#[repr(C)]
struct SockAddr {
    family: u16,
    port:   u16,
    addr4:  u32,
    addr6:  [u8; 16],
    scope:  u32,
}

impl SockAddr {
    #[inline]
    fn new(v: &SocketAddr) -> (SockAddr, u8) {
        // Shamelessly stolen from my winapi code :3
        match v {
            SocketAddr::V4(a) => (
                SockAddr {
                    family: 0x2u16,
                    port:   u16::from_be(a.port()),
                    addr4:  u32::from_le_bytes(a.ip().octets()),
                    addr6:  [0u8; 16],
                    scope:  0u32,
                },
                0x10,
            ),
            SocketAddr::V6(a) => (
                SockAddr {
                    family: 0x17u16,
                    port:   u16::from_be(a.port()),
                    addr4:  a.flowinfo(),
                    addr6:  a.ip().octets(),
                    scope:  a.scope_id(),
                },
                0x1C,
            ),
        }
    }
}
impl SockState {
    #[inline]
    pub async fn get(self) -> IoResult<OwnedFd> {
        match self {
            SockState::Ok(v) => Ok(v),
            SockState::Wait(v) => v.await,
        }
    }
}
impl AsyncConnect {
    #[inline]
    pub fn new_tcp(addr: &SocketAddr, dur: Duration) -> IoResult<SockState> {
        AsyncConnect::new(addr, SOCK_STREAM, dur)
    }
    #[inline]
    pub fn new(addr: &SocketAddr, ty: i32, dur: Duration) -> IoResult<SockState> {
        let f = match addr {
            SocketAddr::V4(_) => AF_INET,
            SocketAddr::V6(_) => AF_INET6,
        };
        let s = OwnedHandle::new(errcheck!(socket(f, ty, 0))?);
        errcheck!(fcntl(**s, F_SETFL, O_NONBLOCK | O_CLOEXEC))?;
        let (a, n) = SockAddr::new(addr);
        if unsafe { connect(**s, &a as *const SockAddr as _, n as _) } == 0 {
            return Ok(SockState::Ok(unsafe { Handle::take(s) }.into()));
        }
        let e = ErrNo::get();
        if !e.is_blocking() {
            return Err(e.throw());
        }
        Ok(SockState::Wait(AsyncConnect {
            dur,
            fd: s,
            size: n,
            addr: a,
        }))
    }

    #[inline]
    fn connect(&mut self) -> Poll<IoResult<OwnedFd>> {
        let r = unsafe {
            connect(
                **self.fd,
                &self.addr as *const SockAddr as _,
                self.size as _,
            )
        };
        if r == 0 {
            return Poll::Ready(Ok(unsafe { OwnedHandle::take(&mut self.fd) }.into()));
        }
        let e = ErrNo::get();
        if e.is_blocking() {
            Poll::Pending
        } else {
            Poll::Ready(Err(e.throw()))
        }
    }
}

impl Future for AsyncConnect {
    type Output = IoResult<OwnedFd>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<OwnedFd>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) if v.is_timeout() => State::timeout(),
            Status::Ready(_) => self.connect(),
            Status::Setup(v) => {
                if let Err(e) = v.register_queue(true, &self.fd) {
                    return Poll::Ready(Err(e));
                }
                if !self.dur.is_zero() {
                    v.set_timeout(self.dur);
                }
                Poll::Pending
            },
        }
    }
}
