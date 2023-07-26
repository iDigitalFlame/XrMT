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
#![cfg(windows)]

use core::iter::Cloned;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::option::IntoIter;
use core::slice::Iter;
use core::time::Duration;
use core::{cmp, ptr};

use crate::device::winapi::{self, AddressInfo, OwnedSocket, Win32Error};
use crate::util::stx::io::{self, Error, ErrorKind, Read, Write};
use crate::util::stx::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shutdown {
    Read,
    Write,
    Both,
}

pub struct UdpSocket(Socket);
pub struct TcpStream(Socket);
pub struct TcpListener(Socket);
pub struct IntoIncoming(TcpListener);
pub struct Incoming<'a>(&'a TcpListener);

pub trait ToSocketAddrs {
    type Iter: Iterator<Item = SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter>;
}

#[repr(C)]
struct IP6Group {
    addr:      [u8; 16],
    interface: u32,
}
struct Resolver {
    original: *const AddressInfo,
    current:  *const AddressInfo,
    port:     u16,
}
struct Socket(OwnedSocket);

impl Socket {
    #[inline]
    fn new(family: u32, socket_type: u32) -> io::Result<Socket> {
        Ok(Socket(
            winapi::WSASocket(family, socket_type, 0, false, false).map_err(Error::from)?,
        ))
    }

    #[inline]
    fn ttl(&self) -> io::Result<u32> {
        // 0x0 - IPPROTO_IP
        // 0x4 - IP_TTL
        winapi::WSAGetSockOption(&self.0, 0, 0x4).map_err(Error::from)
    }
    #[inline]
    fn nodelay(&self) -> io::Result<bool> {
        // 0x6 - IPPROTO_TCP
        // 0x1 - TCP_NODELAY
        winapi::WSAGetSockOption::<u32>(&self.0, 0x6, 0x1)
            .map(|e| e == 1)
            .map_err(Error::from)
    }
    #[inline]
    fn duplicate(&self) -> io::Result<Socket> {
        winapi::WSADuplicateSocket(&self.0).map(|h| Socket(h)).map_err(Error::from)
    }
    #[inline]
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        winapi::WSAGetPeerName(&self.0).map_err(Error::from)
    }
    #[inline]
    fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        // 0x0 - IPPROTO_IP
        // 0x4 - IP_TTL
        winapi::WSASetSockOption(&self.0, 0, 0x4, ttl).map_err(Error::from)
    }
    #[inline]
    fn socket_addr(&self) -> io::Result<SocketAddr> {
        winapi::WSAGetSockName(&self.0).map_err(Error::from)
    }
    #[inline]
    fn listen(&self, backlog: i32) -> io::Result<()> {
        winapi::WSAListen(&self.0, backlog).map_err(Error::from)
    }
    #[inline]
    fn linger(&self) -> io::Result<Option<Duration>> {
        // 0xFFFF - SOL_SOCKET
        // 0x0080 - SO_LINGER
        winapi::WSAGetSockOption::<[u16; 2]>(&self.0, 0xFFFF, 0x80)
            .and_then(|d| {
                if d[0] == 0 {
                    Ok(None)
                } else {
                    Ok(Some(Duration::from_secs(d[1] as u64)))
                }
            })
            .map_err(Error::from)
    }
    #[inline]
    fn take_error(&self) -> io::Result<Option<Error>> {
        // 0xFFFF - SOL_SOCKET
        // 0x1007 - SO_ERROR
        winapi::WSAGetSockOption::<u32>(&self.0, 0xFFFF, 0x1007)
            .map(|r| if r == 0 { None } else { Some(Win32Error::Code(r).into()) })
            .map_err(Error::from)
    }
    #[inline]
    fn bind(&self, addr: &SocketAddr) -> io::Result<()> {
        winapi::WSABind(&self.0, addr).map_err(Error::from)
    }
    #[inline]
    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        winapi::WSAShutdownSock(&self.0, match how {
            Shutdown::Write => 0x1, // 0x1 - SD_SEND
            Shutdown::Read => 0x0,  // 0x0 - SD_RECEIVE
            Shutdown::Both => 0x2,  // 0x2 - SD_BOTH
        })
        .map_err(Error::from)
    }
    #[inline]
    fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        // 0x2 - MSG_PEEK
        self.recv(0x2, buf)
    }
    #[inline]
    fn accept(&self) -> io::Result<(Socket, SocketAddr)> {
        winapi::WSAAccept(&self.0).map(|v| (Socket(v.0), v.1)).map_err(Error::from)
    }
    #[inline]
    fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        winapi::WSAConnect(&self.0, addr).map_err(Error::from)
    }
    #[inline]
    fn set_nodelay(&self, no_delay: bool) -> io::Result<()> {
        // 0x6 - IPPROTO_TCP
        // 0x1 - TCP_NODELAY
        winapi::WSASetSockOption(&self.0, 0x6, 0x1, if no_delay { 1u32 } else { 0u32 }).map_err(Error::from)
    }
    #[inline]
    fn send(&self, flags: u32, buf: &[u8]) -> io::Result<usize> {
        winapi::WSASend(&self.0, flags, buf).map_err(Error::from)
    }
    #[inline]
    fn timeout(&self, kind: u32) -> io::Result<Option<Duration>> {
        // 0xFFFF - SOL_SOCKET
        winapi::WSAGetSockOption::<u32>(&self.0, 0xFFFF, kind)
            .map(|d| {
                if d == 0 {
                    None
                } else {
                    Some(Duration::new((d / 1000) as u64, (d % 1000) * 1000000))
                }
            })
            .map_err(Error::from)
    }
    #[inline]
    fn recv(&self, flags: u32, buf: &mut [u8]) -> io::Result<usize> {
        winapi::WSARecv(&self.0, flags, buf).map_err(Error::from)
    }
    #[inline]
    fn set_nonblocking(&self, non_blocking: bool) -> io::Result<()> {
        winapi::WSAIoctl(
            &self.0,
            0x8004667E,
            if non_blocking { 1 } else { 0 },
            ptr::null_mut(),
        )
        .map_err(Error::from)?;
        Ok(())
    }
    #[inline]
    fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        let o = linger.map_or([0u16; 2], |d| {
            [1u16, cmp::min(d.as_secs(), u16::MAX as u64) as u16]
        });
        // 0xFFFF - SOL_SOCKET
        // 0x0080 - SO_LINGER
        winapi::WSASetSockOption(&self.0, 0xFFFF, 0x80, o).map_err(Error::from)
    }
    #[inline]
    fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        // 0x2 - MSG_PEEK
        self.recv_from(0x2, buf)
    }
    #[inline]
    fn set_timeout(&self, kind: u32, dur: Option<Duration>) -> io::Result<()> {
        winapi::WSASetSockOption(
            &self.0,
            0xFFFF, // 0xFFFF - SOL_SOCKET
            kind,
            dur.map_or(0u32, |d| cmp::min(d.as_millis(), u32::MAX as u128) as u32),
        )
        .map_err(Error::from)
    }
    #[inline]
    fn connect_timeout(&self, addr: &SocketAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let r = winapi::WSAConnect(&self.0, addr);
        self.set_nonblocking(false)?;
        r.or_else(|e| {
            if e == Win32Error::IoPending {
                winapi::WSAWaitSock(&self.0, timeout)
            } else {
                Err(e)
            }
        })
        .map_err(Error::from)
    }
    #[inline]
    fn send_to(&self, addr: &SocketAddr, flags: u32, buf: &[u8]) -> io::Result<usize> {
        winapi::WSASendTo(&self.0, addr, flags, buf).map_err(Error::from)
    }
    #[inline]
    fn recv_from(&self, flags: u32, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        winapi::WSARecvFrom(&self.0, flags, buf).map_err(Error::from)
    }
}
impl Resolver {
    #[inline]
    fn resolve(&mut self) -> io::Result<vec::IntoIter<SocketAddr>> {
        let p = self.port;
        Ok(self
            .map(|mut a| {
                a.set_port(p);
                a
            })
            .collect::<Vec<_>>()
            .into_iter())
    }
}
impl UdpSocket {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<UdpSocket> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match UdpSocket::try_bind(&a) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        Err(l.unwrap_or_else(|| io::ErrorKind::AddrNotAvailable.into()))
    }
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<UdpSocket> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match UdpSocket::try_connect(&a) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        Err(l.unwrap_or_else(|| io::ErrorKind::AddrNotAvailable.into()))
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }
    #[inline]
    pub fn as_socket(&self) -> &OwnedSocket {
        &self.0 .0
    }
    #[inline]
    pub fn into_socket(self) -> OwnedSocket {
        self.0 .0
    }
    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.nodelay()
    }
    #[inline]
    pub fn broadcast(&self) -> io::Result<bool> {
        // 0x0020 - SO_BROADCAST
        // 0xFFFF - SOL_SOCKET
        winapi::WSAGetSockOption::<u32>(&self.0 .0, 0xFFFF, 0x20)
            .map(|e| e == 1)
            .map_err(Error::from)
    }
    #[inline]
    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.0.duplicate().map(|s| UdpSocket(s))
    }
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }
    #[inline]
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        // 0x0 - IP_PROTO_IP
        // 0xA - IP_MULTICAST_TTL
        winapi::WSAGetSockOption::<u32>(&self.0 .0, 0, 0xA).map_err(Error::from)
    }
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }
    #[inline]
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        // 0x0 - IP_PROTO_IP
        // 0xB - IP_MULTICAST_LOOP
        winapi::WSAGetSockOption::<u32>(&self.0 .0, 0, 0xB)
            .map(|e| e == 1)
            .map_err(Error::from)
    }
    #[inline]
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        // 0x29 - IP_PROTO_IPV6
        // 0x0B - IP_MULTICAST_LOOP
        winapi::WSAGetSockOption::<u32>(&self.0 .0, 0x29, 0xB)
            .map(|e| e == 1)
            .map_err(Error::from)
    }
    #[inline]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(0, buf)
    }
    #[inline]
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.0.linger()
    }
    #[inline]
    pub fn take_error(&self) -> io::Result<Option<Error>> {
        self.0.take_error()
    }
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }
    #[inline]
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(0, buf)
    }
    #[inline]
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        // 0x1006 - SO_RCVTIMEO
        self.0.timeout(0x1006)
    }
    #[inline]
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        // 0x1005 - SO_SNDTIMEO
        self.0.timeout(0x1005)
    }
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> io::Result<()> {
        self.0.set_nodelay(no_delay)
    }
    #[inline]
    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        // 0x0020 - SO_BROADCAST
        // 0xFFFF - SOL_SOCKET
        winapi::WSASetSockOption(
            &self.0 .0,
            0xFFFF,
            0x20,
            if broadcast { 1u32 } else { 0u32 },
        )
        .map_err(Error::from)
    }
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(non_blocking)
    }
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        self.0.set_linger(linger)
    }
    #[inline]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        // 0x1006 - SO_RCVTIMEO
        self.0.set_timeout(0x1006, dur)
    }
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        // 0x1005 - SO_SNDTIMEO
        self.0.set_timeout(0x1005, dur)
    }
    #[inline]
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.recv_from(0, buf)
    }
    #[inline]
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.peek_from(buf)
    }
    #[inline]
    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        // 0x0 - IP_PROTO_IP
        // 0xA - IP_MULTICAST_TTL
        winapi::WSASetSockOption(&self.0 .0, 0, 0xA, multicast_ttl_v4).map_err(Error::from)
    }
    #[inline]
    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        // 0x29 - IP_PROTO_IPV6
        // 0x0B - IP_MULTICAST_LOOP
        winapi::WSASetSockOption(
            &self.0 .0,
            0x29,
            0xB,
            if multicast_loop_v6 { 1u32 } else { 0u32 },
        )
        .map_err(Error::from)
    }
    #[inline]
    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        // 0x0 - IP_PROTO_IP
        // 0xB - IP_MULTICAST_LOOP
        winapi::WSASetSockOption(
            &self.0 .0,
            0,
            0xB,
            if multicast_loop_v4 { 1u32 } else { 0u32 },
        )
        .map_err(Error::from)
    }
    pub fn send_to(&self, buf: &[u8], addr: impl ToSocketAddrs) -> io::Result<usize> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match self.0.send_to(&a, 0, buf) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        Err(l.unwrap_or_else(|| io::ErrorKind::AddrNotAvailable.into()))
    }
    #[inline]
    pub fn join_multicast_v6(&self, multi_addr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let v = IP6Group {
            interface,
            addr: multi_addr.octets(),
        };
        // 0x29 - IP_PROTO_IPV6
        // 0x0C - IPV6_ADD_MEMBERSHIP
        winapi::WSASetSockOption(&self.0 .0, 0x29, 0xC, v).map_err(Error::from)
    }
    #[inline]
    pub fn leave_multicast_v6(&self, multi_addr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let v = IP6Group {
            interface,
            addr: multi_addr.octets(),
        };
        // 0x29 - IP_PROTO_IPV6
        // 0x0D - IP_DROP_MEMBERSHIP
        winapi::WSASetSockOption(&self.0 .0, 0x29, 0xD, v).map_err(Error::from)
    }
    #[inline]
    pub fn join_multicast_v4(&self, multi_addr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let v = [
            u32::from_ne_bytes(multi_addr.octets()),
            u32::from_ne_bytes(interface.octets()),
        ];
        // 0x0 - IP_PROTO_IPV
        // 0xC - IP_ADD_MEMBERSHIP
        winapi::WSASetSockOption(&self.0 .0, 0, 0xB, v).map_err(Error::from)
    }
    #[inline]
    pub fn leave_multicast_v4(&self, multi_addr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let v = [
            u32::from_ne_bytes(multi_addr.octets()),
            u32::from_ne_bytes(interface.octets()),
        ];
        // 0x0 - IP_PROTO_IPV
        // 0xD - IP_DROP_MEMBERSHIP
        winapi::WSASetSockOption(&self.0 .0, 0, 0xD, v).map_err(Error::from)
    }

    fn try_bind(addr: &SocketAddr) -> io::Result<UdpSocket> {
        // 0x02 - SOCK_DGRAM
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let s = Socket::new(
            match addr {
                SocketAddr::V4(_) => 0x2,
                SocketAddr::V6(_) => 0x17,
            },
            2,
        )?;
        s.bind(addr)?;
        Ok(UdpSocket(s))
    }
    fn try_connect(addr: &SocketAddr) -> io::Result<UdpSocket> {
        // 0x02 - SOCK_DGRAM
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let s = Socket::new(
            match addr {
                SocketAddr::V4(_) => 0x2,
                SocketAddr::V6(_) => 0x17,
            },
            2,
        )?;
        s.connect(addr)?;
        Ok(UdpSocket(s))
    }
}
impl TcpStream {
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<TcpStream> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match TcpStream::try_connect(&a) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        Err(l.unwrap_or_else(|| io::ErrorKind::AddrNotAvailable.into()))
    }
    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        // 0x01 - SOCK_STREAM
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let s = Socket::new(
            match addr {
                SocketAddr::V4(_) => 0x2,
                SocketAddr::V6(_) => 0x17,
            },
            1,
        )?;
        s.connect_timeout(addr, timeout)?;
        Ok(TcpStream(s))
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }
    #[inline]
    pub fn as_socket(&self) -> &OwnedSocket {
        &self.0 .0
    }
    #[inline]
    pub fn into_socket(self) -> OwnedSocket {
        self.0 .0
    }
    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.nodelay()
    }
    #[inline]
    pub fn try_clone(&self) -> io::Result<TcpStream> {
        self.0.duplicate().map(|s| TcpStream(s))
    }
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }
    #[inline]
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.0.linger()
    }
    #[inline]
    pub fn take_error(&self) -> io::Result<Option<Error>> {
        self.0.take_error()
    }
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }
    #[inline]
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        // 0x1006 - SO_RCVTIMEO
        self.0.timeout(0x1006)
    }
    #[inline]
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        // 0x1005 - SO_SNDTIMEO
        self.0.timeout(0x1005)
    }
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> io::Result<()> {
        self.0.set_nodelay(no_delay)
    }
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(non_blocking)
    }
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        self.0.set_linger(linger)
    }
    #[inline]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        // 0x1006 - SO_RCVTIMEO
        self.0.set_timeout(0x1006, dur)
    }
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        // 0x1005 - SO_SNDTIMEO
        self.0.set_timeout(0x1005, dur)
    }

    fn try_connect(addr: &SocketAddr) -> io::Result<TcpStream> {
        // 0x01 - SOCK_STREAM
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let s = Socket::new(
            match addr {
                SocketAddr::V4(_) => 0x02,
                SocketAddr::V6(_) => 0x17,
            },
            0x1,
        )?;
        s.connect(addr)?;
        Ok(TcpStream(s))
    }
}
impl TcpListener {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<TcpListener> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match TcpListener::try_bind(&a) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        Err(l.unwrap_or_else(|| io::ErrorKind::AddrNotAvailable.into()))
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }
    #[inline]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self)
    }
    #[inline]
    pub fn as_socket(&self) -> &OwnedSocket {
        &self.0 .0
    }
    #[inline]
    pub fn into_socket(self) -> OwnedSocket {
        self.0 .0
    }
    #[inline]
    pub fn into_incoming(self) -> IntoIncoming {
        IntoIncoming(self)
    }
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }
    #[inline]
    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.0.duplicate().map(|s| TcpListener(s))
    }
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }
    #[inline]
    pub fn take_error(&self) -> io::Result<Option<Error>> {
        self.0.take_error()
    }
    #[inline]
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.0.accept().map(|v| (TcpStream(v.0), v.1))
    }
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(non_blocking)
    }

    fn try_bind(addr: &SocketAddr) -> io::Result<TcpListener> {
        // 0x01 - SOCK_STREAM
        // 0x02 - AF_INET
        // 0x17 - AF_INET6
        let s = Socket::new(
            match addr {
                SocketAddr::V4(_) => 0x02,
                SocketAddr::V6(_) => 0x17,
            },
            0x1,
        )?;
        s.bind(addr)?;
        s.listen(0x80)?;
        Ok(TcpListener(s))
    }
}

impl From<OwnedSocket> for UdpSocket {
    #[inline]
    fn from(v: OwnedSocket) -> UdpSocket {
        UdpSocket(Socket(v))
    }
}
impl From<OwnedSocket> for TcpStream {
    #[inline]
    fn from(v: OwnedSocket) -> TcpStream {
        TcpStream(Socket(v))
    }
}
impl From<OwnedSocket> for TcpListener {
    #[inline]
    fn from(v: OwnedSocket) -> TcpListener {
        TcpListener(Socket(v))
    }
}

impl Read for TcpStream {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(0, buf)
    }
}
impl Read for &TcpStream {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(0, buf)
    }
}

impl Write for TcpStream {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(0, buf)
    }
}
impl Write for &TcpStream {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(0, buf)
    }
}

impl Iterator for IntoIncoming {
    type Item = io::Result<TcpStream>;

    #[inline]
    fn next(&mut self) -> Option<io::Result<TcpStream>> {
        Some(self.0.accept().map(|p| p.0))
    }
}
impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<TcpStream>;

    #[inline]
    fn next(&mut self) -> Option<io::Result<TcpStream>> {
        Some(self.0.accept().map(|p| p.0))
    }
}

impl ToSocketAddrs for str {
    type Iter = vec::IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        if let Ok(a) = self.parse() {
            return Ok(vec![a].into_iter());
        }
        Resolver::try_from(self)?.resolve()
    }
}
impl ToSocketAddrs for String {
    type Iter = vec::IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        (&**self).to_socket_addrs()
    }
}
impl ToSocketAddrs for SocketAddr {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        Ok(Some(*self).into_iter())
    }
}
impl ToSocketAddrs for (&str, u16) {
    type Iter = vec::IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        let (h, p) = *self;
        if let Ok(a) = h.parse::<Ipv4Addr>() {
            return Ok(vec![SocketAddr::V4(SocketAddrV4::new(a, p))].into_iter());
        }
        if let Ok(a) = h.parse::<Ipv6Addr>() {
            return Ok(vec![SocketAddr::V6(SocketAddrV6::new(a, p, 0, 0))].into_iter());
        }
        Resolver::try_from((h, p))?.resolve()
    }
}
impl ToSocketAddrs for SocketAddrV4 {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        SocketAddr::V4(*self).to_socket_addrs()
    }
}
impl ToSocketAddrs for SocketAddrV6 {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        SocketAddr::V6(*self).to_socket_addrs()
    }
}
impl ToSocketAddrs for (IpAddr, u16) {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        let (i, p) = *self;
        match i {
            IpAddr::V4(a) => (a, p).to_socket_addrs(),
            IpAddr::V6(a) => (a, p).to_socket_addrs(),
        }
    }
}
impl ToSocketAddrs for (String, u16) {
    type Iter = vec::IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        (&*self.0, self.1).to_socket_addrs()
    }
}
impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        let (i, p) = *self;
        SocketAddrV4::new(i, p).to_socket_addrs()
    }
}
impl ToSocketAddrs for (Ipv6Addr, u16) {
    type Iter = IntoIter<SocketAddr>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<IntoIter<SocketAddr>> {
        let (i, p) = *self;
        SocketAddrV6::new(i, p, 0, 0).to_socket_addrs()
    }
}

impl<'a> ToSocketAddrs for &'a [SocketAddr] {
    type Iter = Cloned<Iter<'a, SocketAddr>>;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        Ok(self.iter().cloned())
    }
}
impl<T: ToSocketAddrs + ?Sized> ToSocketAddrs for &T {
    type Iter = T::Iter;

    #[inline]
    fn to_socket_addrs(&self) -> io::Result<T::Iter> {
        (**self).to_socket_addrs()
    }
}

impl Drop for Resolver {
    #[inline]
    fn drop(&mut self) {
        winapi::WSAFreeAddrInfo(self.original)
    }
}
impl Iterator for Resolver {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        loop {
            let n = unsafe { self.current.as_ref()? };
            self.current = n.next;
            if let Some(a) = n.into() {
                return Some(a);
            }
        }
    }
}

impl TryFrom<&str> for Resolver {
    type Error = Error;

    #[inline]
    fn try_from(s: &str) -> io::Result<Resolver> {
        let i = s.find(':').ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        let p: u16 = s[i + 1..].parse().map_err(|_| Error::from(ErrorKind::InvalidInput))?;
        (&s[0..i], p).try_into()
    }
}
impl<'a> TryFrom<(&'a str, u16)> for Resolver {
    type Error = Error;

    #[inline]
    fn try_from((host, port): (&'a str, u16)) -> io::Result<Resolver> {
        winapi::WSAGetAddrInfo(host, 1, 0, 0)
            .map_err(Error::from)
            .map(|i| Resolver { port, current: i, original: i })
    }
}

unsafe impl Sync for Resolver {}
unsafe impl Send for Resolver {}
