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

use core::convert::{From, Into};
use core::net::SocketAddr;
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::time::Duration;

use xrmt_stx::io::{BorrowedCursor, ErrorKind, IoError, IoResult, Read, Write};
use xrmt_stx::net::{self, Shutdown, ToSocketAddrs};

use crate::stxa::io::{AsyncRead, AsyncWrite};
use crate::stxa::{AsyncConnect, AsyncSockReader, AsyncSockWriter};

pub struct TcpStream(pub(crate) net::TcpStream);

impl TcpStream {
    #[inline]
    pub async fn connect_async(addr: impl ToSocketAddrs) -> IoResult<TcpStream> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            l = match TcpStream::sock(&a, Duration::ZERO).await {
                Ok(s) => return Ok(s),
                Err(e) => Some(e),
            };
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }
    #[inline]
    pub async fn connect_timeout_async(addr: &SocketAddr, timeout: Duration) -> IoResult<TcpStream> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            l = match TcpStream::sock(&a, timeout).await {
                Ok(s) => return Ok(s),
                Err(e) => Some(e),
            };
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }

    #[inline]
    pub fn connect(addr: impl ToSocketAddrs) -> IoResult<TcpStream> {
        TcpStream::new(net::TcpStream::connect(addr)?)
    }
    #[inline]
    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> IoResult<TcpStream> {
        TcpStream::new(net::TcpStream::connect_timeout(addr, timeout)?)
    }

    pub fn ttl(&self) -> IoResult<u32> {
        self.0.ttl()
    }
    #[inline]
    pub fn nodelay(&self) -> IoResult<bool> {
        self.0.nodelay()
    }
    #[inline]
    pub fn try_clone(&self) -> IoResult<TcpStream> {
        self.0.try_clone().map(TcpStream)
    }
    #[inline]
    pub fn peer_addr(&self) -> IoResult<SocketAddr> {
        self.0.peer_addr()
    }
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> IoResult<()> {
        self.0.set_ttl(ttl)
    }
    #[inline]
    pub fn local_addr(&self) -> IoResult<SocketAddr> {
        self.0.local_addr()
    }
    #[inline]
    pub fn linger(&self) -> IoResult<Option<Duration>> {
        self.0.linger()
    }
    #[inline]
    pub fn take_error(&self) -> IoResult<Option<IoError>> {
        self.0.take_error()
    }
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        self.0.shutdown(how)
    }
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.peek(buf)
    }
    #[inline]
    pub fn read_timeout(&self) -> IoResult<Option<Duration>> {
        // 0x1006 - SO_RCVTIMEO
        self.0.read_timeout()
    }
    #[inline]
    pub fn write_timeout(&self) -> IoResult<Option<Duration>> {
        // 0x1005 - SO_SNDTIMEO
        self.0.write_timeout()
    }
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> IoResult<()> {
        self.0.set_nodelay(no_delay)
    }
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> IoResult<()> {
        self.0.set_nonblocking(non_blocking)
    }
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> IoResult<()> {
        self.0.set_linger(linger)
    }
    #[inline]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
        // 0x1006 - SO_RCVTIMEO
        self.0.set_read_timeout(dur)
    }
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
        // 0x1005 - SO_SNDTIMEO
        self.0.set_write_timeout(dur)
    }

    #[inline]
    async fn sock(addr: &SocketAddr, dur: Duration) -> IoResult<TcpStream> {
        match AsyncConnect::new_tcp(addr, dur) {
            Ok(v) => TcpStream::new(v.get().await?.into()),
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn new(v: net::TcpStream) -> IoResult<TcpStream> {
        #[cfg(not(target_family = "windows"))]
        {
            // We only do this for non-Windows as we already set it on Windows
            // but, different *nix types have ways of doing this, so just setting
            // O_NONBLOCK via fnctl might not be everything we need.
            v.set_nonblocking(true)?;
        }
        Ok(TcpStream(v))
    }
}

impl Read for TcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.read(buf)
    }
    #[inline]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        self.0.read_buf(buf)
    }
}
impl Read for &TcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (&self.0).read(buf)
    }
    #[inline]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        (&self.0).read_buf(buf)
    }
}
impl Write for TcpStream {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }
}
impl Write for &TcpStream {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        (&self.0).flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&self.0).write(buf)
    }
}
impl AsyncRead for TcpStream {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        AsyncSockReader::new(inner::sock_handle(self), buf).await
    }
}
impl AsyncWrite for TcpStream {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        AsyncSockWriter::new(inner::sock_handle(self), buf).await
    }
}
impl AsyncRead for &TcpStream {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        AsyncSockReader::new(inner::sock_handle(self), buf).await
    }
}
impl AsyncWrite for &TcpStream {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        (&self.0).flush()
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        AsyncSockWriter::new(inner::sock_handle(self), buf).await
    }
}

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_stx;

    use core::mem::transmute;

    use xrmt_stx::os::windows::io::{AsSocket, OwnedSocket};

    use crate::stxa::net::TcpStream;

    #[inline]
    pub fn sock_handle(v: &TcpStream) -> &OwnedSocket {
        // They're the same size so this is fine.
        unsafe { transmute(v.0.as_socket()) }
    }
}
#[cfg(not(target_family = "windows"))]
mod inner {
    extern crate xrmt_stx;

    use xrmt_stx::os::{AsFdRef, Handle};

    use crate::stxa::net::TcpStream;

    #[inline]
    pub fn sock_handle(v: &TcpStream) -> &Handle {
        v.0.as_ref()
    }
}
