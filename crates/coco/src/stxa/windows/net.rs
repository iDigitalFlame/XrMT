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

extern crate xrmt_stx;
extern crate xrmt_winapi;

use core::convert::{From, Into};
use core::default::Default;
use core::future::Future;
use core::net::SocketAddr;
use core::option::Option::{self, None, Some};
use core::pin::Pin;
use core::result::Result::{Err, Ok};
use core::task::{Context, Poll};
use core::time::Duration;

use xrmt_stx::io::{IoError, IoResult};
use xrmt_stx::sync::extra::Event;
use xrmt_winapi::functions::{WSAConnect, WSAEventSelect, WSAIoctl, WSARecv, WSASend, WSASocket};
use xrmt_winapi::structs::{Overlapped, OwnedSocket};
use xrmt_winapi::Win32Error;

use crate::future::{State, Status};

pub enum SockState {
    Ok(OwnedSocket),
    Wait(AsyncConnect),
}

pub struct AsyncConnect {
    ev:  Event,
    fd:  OwnedSocket,
    dur: Duration,
}
pub struct AsyncSockReader<'a> {
    h:   &'a OwnedSocket,
    buf: &'a mut [u8],
    olp: Overlapped,
}
pub struct AsyncSockWriter<'a> {
    h:   &'a OwnedSocket,
    buf: &'a [u8],
    olp: Overlapped,
}

impl SockState {
    #[inline]
    pub async fn get(self) -> IoResult<OwnedSocket> {
        match self {
            SockState::Ok(v) => Ok(v.into()),
            SockState::Wait(v) => v.await,
        }
    }
}
impl AsyncConnect {
    #[inline]
    pub fn new_tcp(addr: &SocketAddr, dur: Duration) -> IoResult<SockState> {
        // 0x01 - SOCK_STREAM
        AsyncConnect::new(addr, 0x1, dur)
    }
    #[inline]
    pub fn new(addr: &SocketAddr, ty: u32, dur: Duration) -> IoResult<SockState> {
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let f = match addr {
            SocketAddr::V4(_) => 0x02,
            SocketAddr::V6(_) => 0x17,
        };
        let s = WSASocket(f, ty, 0, false, false)?;
        let mut r = 0u32;
        let _ = WSAIoctl(&s, None, 0x8004667E, 0x1, &mut r)?;
        match WSAConnect(&s, addr) {
            Ok(_) => return Ok(SockState::Ok(s)),
            Err(Win32Error::IoPending) => (),
            Err(e) => return Err(IoError::from(e)),
        }
        Ok(SockState::Wait(AsyncConnect {
            dur,
            ev: Event::new()?, // TODO(dij): Fix and make a thread pool
            fd: s,
        }))
    }
}
impl<'a> AsyncSockReader<'a> {
    #[inline]
    pub fn new(h: &'a OwnedSocket, buf: &'a mut [u8]) -> AsyncSockReader<'a> {
        AsyncSockReader {
            h,
            buf,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn read(&mut self) -> IoResult<Option<usize>> {
        match WSARecv(self.h, Some(&mut self.olp), 0, self.buf) {
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
impl<'a> AsyncSockWriter<'a> {
    #[inline]
    pub fn new(h: &'a OwnedSocket, buf: &'a [u8]) -> AsyncSockWriter<'a> {
        AsyncSockWriter {
            h,
            buf,
            olp: Overlapped::default(),
        }
    }

    #[inline]
    fn write(&mut self) -> IoResult<Option<usize>> {
        match WSASend(self.h, Some(&mut self.olp), 0, self.buf) {
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

impl Future for AsyncConnect {
    type Output = IoResult<OwnedSocket>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<OwnedSocket>> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) if v.is_timeout() => State::timeout(),
            Status::Ready(_) => Poll::Ready(Ok(unsafe { OwnedSocket::take(&mut self.fd) })),
            Status::Setup(v) => {
                if let Err(e) = v.register_handle(true, &self.ev) {
                    return Poll::Ready(Err(e));
                }
                // 0x32 - FD_CONNECT | FD_CLOSE | FD_WRITE
                if let Err(e) = WSAEventSelect(&self.fd, &self.ev, 0x32) {
                    return Poll::Ready(Err(e.into()));
                }
                if !self.dur.is_zero() {
                    v.set_timeout(self.dur);
                }
                Poll::Pending
            },
        }
    }
}
impl Future for AsyncSockReader<'_> {
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
impl Future for AsyncSockWriter<'_> {
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
