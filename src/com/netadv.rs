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

use crate::io::{self, ErrorKind, Read, Write};
use crate::net::ToSocketAddrs;
use crate::prelude::*;

pub(super) mod ip;
pub(super) mod pipe;
pub(super) mod tcp;
pub(super) mod udp;

pub trait Listener {
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn accept(&self) -> io::Result<(Box<dyn Conn>, SocketAddr)>;
}

pub trait Conn: Read + Write {
    fn peer_addr(&self) -> io::Result<SocketAddr>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn read_timeout(&self) -> io::Result<Option<Duration>>;
    fn write_timeout(&self) -> io::Result<Option<Duration>>;
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()>;

    #[inline]
    fn set_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)?;
        self.set_write_timeout(dur)
    }
}

fn foreach_addr<R, T: ToSocketAddrs, F: Fn(&SocketAddr, Duration) -> io::Result<R>, G: Fn(&SocketAddr) -> io::Result<R>>(v: T, dur: Option<Duration>, f1: F, f2: G) -> io::Result<R> {
    for a in v.to_socket_addrs()? {
        let r = match dur {
            Some(t) => f1(&a, t),
            None => f2(&a),
        };
        if let Ok(c) = r {
            return Ok(c);
        }
    }
    Err(ErrorKind::NetworkUnreachable.into())
}
