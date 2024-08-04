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

use core::net::SocketAddr;
use core::time::Duration;

use crate::com::netadv::{foreach_addr, Conn, Listener};
use crate::io;
use crate::net::{TcpListener, TcpStream};
use crate::prelude::*;

impl Conn for TcpStream {
    #[inline]
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.peer_addr()
    }
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
    #[inline]
    fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout()
    }
    #[inline]
    fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout()
    }
    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }
    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_write_timeout(dur)
    }
}

impl Listener for TcpListener {
    #[inline]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
    #[inline]
    fn accept(&self) -> io::Result<(Box<dyn Conn>, SocketAddr)> {
        let (c, a) = self.accept()?;
        Ok((Box::new(c), a))
    }
}

#[inline]
pub fn tcp_listen(addr: &str) -> io::Result<TcpListener> {
    TcpListener::bind(addr)
}
#[inline]
pub fn tcp_connect(addr: impl AsRef<str>, dur: Option<Duration>) -> io::Result<TcpStream> {
    let c = foreach_addr(
        addr.as_ref(),
        dur,
        |a, t| TcpStream::connect_timeout(a, t),
        |a| TcpStream::connect(a),
    )?;
    if dur.is_some() {
        c.set_read_timeout(dur)?;
        c.set_write_timeout(dur)?;
    }
    Ok(c)
}
