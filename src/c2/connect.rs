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

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::matches;
use core::time::Duration;

use crate::com::{tcp_connect, tcp_listen, udp_connect, udp_listen, Conn, Listener};
use crate::io::{self, ErrorKind};
use crate::prelude::*;

pub enum Accepter<A: Allocator = Global> {
    None,
    Tcp,
    Udp,
    Icmp,
    Pipe,
    Wc2,
    Tls,
    Ip(u8),
    TlsEx(u8),
    TlsCerts(
        u8,
        Option<Vec<u8, A>>,
        Option<Vec<u8, A>>,
        Option<Vec<u8, A>>,
    ),
    Custom(Box<dyn CustomAccepter>),
}
pub enum Connecter<A: Allocator = Global> {
    None,
    Tcp,
    Udp,
    Icmp,
    Pipe,
    Wc2,
    Tls,
    TlsInsecure,
    Ip(u8),
    TlsEx(u8),
    TlsCerts(
        u8,
        Option<Vec<u8, A>>,
        Option<Vec<u8, A>>,
        Option<Vec<u8, A>>,
    ),
    Custom(Box<dyn CustomConnector>),
}

pub trait CustomAccepter {
    fn listen(&self, addr: &str) -> io::Result<Box<dyn Listener>>;
}
pub trait CustomConnector {
    fn connect(&self, addr: &str, dur: Option<Duration>) -> io::Result<Box<dyn Conn>>;
}

impl<A: Allocator> Accepter<A> {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Accepter::None)
    }
    pub fn listen(&self, addr: &str) -> io::Result<Box<dyn Listener>> {
        match self {
            Accepter::Tcp => return Ok(Box::new(tcp_listen(addr)?)),
            Accepter::Udp => return Ok(Box::new(udp_listen(addr)?)),
            Accepter::Tls => core::todo!(),
            Accepter::Wc2 => core::todo!(),
            Accepter::Pipe => core::todo!(),
            Accepter::Icmp => core::todo!(),
            Accepter::Ip(_) => core::todo!(),
            Accepter::TlsEx(_) => core::todo!(),
            Accepter::TlsCerts(..) => core::todo!(),
            Accepter::Custom(x) => return x.listen(addr),
            _ => (),
        }
        Err(ErrorKind::InvalidInput.into())
    }
}
impl<A: Allocator> Connecter<A> {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Connecter::None)
    }
    pub fn invert(&self) -> io::Result<Accepter> {
        match self {
            Connecter::Tcp => return Ok(Accepter::Tcp),
            Connecter::Udp => return Ok(Accepter::Udp),
            Connecter::Tls => return Ok(Accepter::Tls),
            Connecter::Pipe => return Ok(Accepter::Pipe),
            _ => Err(ErrorKind::Unsupported.into()),
        }
    }
    pub fn connect(&self, addr: impl AsRef<str>, dur: Option<Duration>) -> io::Result<Box<dyn Conn>> {
        match self {
            Connecter::Tcp => return Ok(Box::new(tcp_connect(addr, dur)?)),
            Connecter::Udp => return Ok(Box::new(udp_connect(addr, dur)?)),
            Connecter::Tls => core::todo!(),
            Connecter::Wc2 => core::todo!(),
            Connecter::Pipe => core::todo!(),
            Connecter::Icmp => core::todo!(),
            Connecter::Ip(_) => core::todo!(),
            Connecter::TlsEx(_) => core::todo!(),
            Connecter::TlsCerts(..) => core::todo!(),
            Connecter::TlsInsecure => core::todo!(),
            Connecter::Custom(x) => return x.connect(addr.as_ref(), dur),
            _ => (),
        }
        Err(ErrorKind::InvalidInput.into())
    }
}
