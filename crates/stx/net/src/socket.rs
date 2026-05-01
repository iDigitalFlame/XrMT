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
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

extern crate xrmt_io;
extern crate xrmt_winapi;

use core::clone::Clone;
use core::cmp::{min, Eq, PartialEq};
use core::convert::Into;
use core::marker::Copy;
use core::net::SocketAddr;
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::time::Duration;

use xrmt_io::{IoError, IoResult};
use xrmt_winapi::functions::{WSAAccept, WSABind, WSAConnect, WSADuplicateSocket, WSAGetPeerName, WSAGetSockName, WSAGetSockOption, WSAIoctl, WSAListen, WSARecv, WSARecvFrom, WSASend, WSASendTo, WSASetSockOption, WSAShutdownSock, WSASocket, WSAWaitSock};
use xrmt_winapi::structs::OwnedSocket;
use xrmt_winapi::Win32Error;

pub enum Shutdown {
    Read,
    Write,
    Both,
}

pub struct Socket(pub(super) OwnedSocket);

impl Socket {
    #[inline]
    pub fn new(family: u32, socket_type: u32) -> IoResult<Socket> {
        Ok(Socket(WSASocket(family, socket_type, 0, false, false)?))
    }

    #[inline]
    pub fn ttl(&self) -> IoResult<u32> {
        // 0x0 - IPPROTO_IP
        // 0x4 - IP_TTL
        Ok(WSAGetSockOption(&self.0, 0, 0x4)?)
    }
    #[inline]
    pub fn nodelay(&self) -> IoResult<bool> {
        // 0x6 - IPPROTO_TCP
        // 0x1 - TCP_NODELAY
        Ok(WSAGetSockOption::<u32>(&self.0, 0x6, 0x1).map(|e| e == 1)?)
    }
    #[inline]
    pub fn duplicate(&self) -> IoResult<Socket> {
        Ok(WSADuplicateSocket(&self.0).map(|h| Socket(h))?)
    }
    #[inline]
    pub fn peer_addr(&self) -> IoResult<SocketAddr> {
        Ok(WSAGetPeerName(&self.0)?)
    }
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> IoResult<()> {
        // 0x0 - IPPROTO_IP
        // 0x4 - IP_TTL
        Ok(WSASetSockOption(&self.0, 0, 0x4, ttl)?)
    }
    #[inline]
    pub fn socket_addr(&self) -> IoResult<SocketAddr> {
        Ok(WSAGetSockName(&self.0)?)
    }
    #[inline]
    pub fn listen(&self, backlog: i32) -> IoResult<()> {
        Ok(WSAListen(&self.0, backlog)?)
    }
    #[inline]
    pub fn linger(&self) -> IoResult<Option<Duration>> {
        // 0xFFFF - SOL_SOCKET
        // 0x0080 - SO_LINGER
        Ok(
            WSAGetSockOption::<[u16; 2]>(&self.0, 0xFFFF, 0x80).and_then(|d| unsafe {
                // Size is already known.
                if *d.get_unchecked(0) == 0 {
                    Ok(None)
                } else {
                    Ok(Some(Duration::from_secs(*d.get_unchecked(1) as u64)))
                }
            })?,
        )
    }
    #[inline]
    pub fn take_error(&self) -> IoResult<Option<IoError>> {
        // 0xFFFF - SOL_SOCKET
        // 0x1007 - SO_ERROR
        Ok(WSAGetSockOption::<u32>(&self.0, 0xFFFF, 0x1007).map(|r| {
            if r == 0 {
                None
            } else {
                Some(Win32Error::from_code(r).into())
            }
        })?)
    }
    #[inline]
    pub fn bind(&self, addr: &SocketAddr) -> IoResult<()> {
        Ok(WSABind(&self.0, addr)?)
    }
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        Ok(WSAShutdownSock(&self.0, match how {
            Shutdown::Write => 1, // 0x1 - SD_SEND
            Shutdown::Read => 0,  // 0x0 - SD_RECEIVE
            Shutdown::Both => 2,  // 0x2 - SD_BOTH
        })?)
    }
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> IoResult<usize> {
        // 0x2 - MSG_PEEK
        self.recv(0x2, buf)
    }
    #[inline]
    pub fn accept(&self) -> IoResult<(Socket, SocketAddr)> {
        Ok(WSAAccept(&self.0).map(|v| (Socket(v.0), v.1))?)
    }
    #[inline]
    pub fn connect(&self, addr: &SocketAddr) -> IoResult<()> {
        Ok(WSAConnect(&self.0, addr).or_else(|e| {
            if e == Win32Error::IoPending {
                WSAWaitSock(&self.0, None)
            } else {
                Err(e)
            }
        })?)
    }
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> IoResult<()> {
        // 0x6 - IPPROTO_TCP
        // 0x1 - TCP_NODELAY
        Ok(WSASetSockOption(
            &self.0,
            0x6,
            0x1,
            if no_delay { 1 } else { 0 },
        )?)
    }
    #[inline]
    pub fn send(&self, flags: u32, buf: &[u8]) -> IoResult<usize> {
        Ok(WSASend(&self.0, None, flags, buf)?)
    }
    #[inline]
    pub fn timeout(&self, kind: u32) -> IoResult<Option<Duration>> {
        // 0xFFFF - SOL_SOCKET
        Ok(WSAGetSockOption::<u32>(&self.0, 0xFFFF, kind).map(|d| {
            if d == 0 {
                None
            } else {
                Some(Duration::new((d / 1000) as u64, (d % 1000) * 1000000))
            }
        })?)
    }
    #[inline]
    pub fn recv(&self, flags: u32, buf: &mut [u8]) -> IoResult<usize> {
        Ok(WSARecv(&self.0, None, flags, buf)?)
    }
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> IoResult<()> {
        let mut r = 0u32;
        WSAIoctl(
            &self.0,
            None,
            0x8004667E,
            if non_blocking { 1 } else { 0 },
            &mut r,
        )?;
        Ok(())
    }
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> IoResult<()> {
        let o = linger.map_or([0u16, 0u16], |d| {
            [1, min(d.as_secs(), u16::MAX as u64) as u16]
        });
        // 0xFFFF - SOL_SOCKET
        // 0x0080 - SO_LINGER
        Ok(WSASetSockOption(&self.0, 0xFFFF, 0x80, o)?)
    }
    #[inline]
    pub fn peek_from(&self, buf: &mut [u8]) -> IoResult<(usize, SocketAddr)> {
        // 0x2 - MSG_PEEK
        self.recv_from(0x2, buf)
    }
    #[inline]
    pub fn set_timeout(&self, kind: u32, dur: Option<Duration>) -> IoResult<()> {
        Ok(WSASetSockOption(
            &self.0,
            0xFFFF, // 0xFFFF - SOL_SOCKET
            kind,
            dur.map_or(0, |d| min(d.as_millis(), 0xFFFFFFFF) as u32),
        )?)
    }
    #[inline]
    pub fn connect_timeout(&self, addr: &SocketAddr, timeout: Duration) -> IoResult<()> {
        self.set_nonblocking(true)?;
        let r = WSAConnect(&self.0, addr);
        self.set_nonblocking(false)?;
        Ok(r.or_else(|e| {
            if e == Win32Error::IoPending {
                WSAWaitSock(&self.0, Some(timeout))
            } else {
                Err(e)
            }
        })?)
    }
    #[inline]
    pub fn send_to(&self, addr: &SocketAddr, flags: u32, buf: &[u8]) -> IoResult<usize> {
        Ok(WSASendTo(&self.0, addr, None, flags, buf)?)
    }
    #[inline]
    pub fn recv_from(&self, flags: u32, buf: &mut [u8]) -> IoResult<(usize, SocketAddr)> {
        Ok(WSARecvFrom(&self.0, None, flags, buf)?)
    }
}

impl Eq for Shutdown {}
impl Copy for Shutdown {}
impl Clone for Shutdown {
    #[inline]
    fn clone(&self) -> Shutdown {
        *self
    }
}
impl PartialEq for Shutdown {
    #[inline]
    fn eq(&self, other: &Shutdown) -> bool {
        match (self, other) {
            (Shutdown::Both, Shutdown::Both) => true,
            (Shutdown::Read, Shutdown::Read) => true,
            (Shutdown::Write, Shutdown::Write) => true,
            _ => false,
        }
    }
}
