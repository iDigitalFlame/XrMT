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

use core::convert::{AsRef, From};
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::time::Duration;

use xrmt_io::{ErrorKind, IoError, IoResult};
use xrmt_winapi::functions::{WSAGetSockOption, WSASetSockOption};
use xrmt_winapi::structs::OwnedSocket;

use crate::net::socket::Socket;
use crate::{Shutdown, ToSocketAddrs};

/// A UDP socket.
///
/// After creating a `UdpSocket` by [`bind`]ing it to a socket address, data can
/// be [sent to] and [received from] any other socket address.
///
/// Although UDP is a connectionless protocol, this implementation provides an
/// interface to set an address where data should be sent and received from.
/// After setting a remote address with [`connect`], data can be sent to and
/// received from that address with [`send`] and [`recv`].
///
/// As stated in the User Datagram Protocol's specification in [IETF RFC 768],
/// UDP is an unordered, unreliable protocol; refer to [`TcpListener`] and
/// [`TcpStream`] for TCP primitives.
///
/// [`bind`]: UdpSocket::bind
/// [`connect`]: UdpSocket::connect
/// [IETF RFC 768]: https://tools.ietf.org/html/rfc768
/// [`recv`]: UdpSocket::recv
/// [received from]: UdpSocket::recv_from
/// [`send`]: UdpSocket::send
/// [sent to]: UdpSocket::send_to
/// [`TcpListener`]: crate::net::TcpListener
/// [`TcpStream`]: crate::net::TcpStream
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::net::UdpSocket;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     {
///         let socket = UdpSocket::bind("127.0.0.1:34254")?;
///
///         // Receives a single datagram message on the socket. If `buf` is too small to hold
///         // the message, it will be cut off.
///         let mut buf = [0; 10];
///         let (amt, src) = socket.recv_from(&mut buf)?;
///
///         // Redeclare `buf` as slice of the received data and send reverse data back to origin.
///         let buf = &mut buf[..amt];
///         buf.reverse();
///         socket.send_to(buf, &src)?;
///     } // the socket is closed here
///     Ok(())
/// }
/// ```
pub struct UdpSocket(Socket);

#[repr(C)]
struct IP6Group {
    addr:      [u8; 16],
    interface: u32,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    ///
    /// The address type can be any implementor of [`ToSocketAddrs`] trait. See
    /// its documentation for concrete examples.
    ///
    /// If `addr` yields multiple addresses, `bind` will be attempted with
    /// each of the addresses until one succeeds and returns the socket. If none
    /// of the addresses succeed in creating a socket, the error returned from
    /// the last attempt (the last address) is returned.
    ///
    /// # Examples
    ///
    /// Creates a UDP socket bound to `127.0.0.1:3400`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:3400").expect("couldn't bind to address");
    /// ```
    ///
    /// Creates a UDP socket bound to `127.0.0.1:3400`. If the socket cannot be
    /// bound to that address, create a UDP socket bound to `127.0.0.1:3401`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::{SocketAddr, UdpSocket};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 3400)),
    ///     SocketAddr::from(([127, 0, 0, 1], 3401)),
    /// ];
    /// let socket = UdpSocket::bind(&addrs[..]).expect("couldn't bind to address");
    /// ```
    ///
    /// Creates a UDP socket bound to a port assigned by the operating system
    /// at `127.0.0.1`.
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    /// ```
    ///
    /// Note that `bind` declares the scope of your network connection.
    /// You can only receive datagrams from and send datagrams to
    /// participants in that view of the network.
    /// For instance, binding to a loopback address as in the example
    /// above will prevent you from sending datagrams to another device
    /// in your local network.
    ///
    /// In order to limit your view of the network the least, `bind` to
    /// [`Ipv4Addr::UNSPECIFIED`] or [`Ipv6Addr::UNSPECIFIED`].
    pub fn bind(addr: impl ToSocketAddrs) -> IoResult<UdpSocket> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match UdpSocket::try_bind(&a) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`UdpSocket::set_ttl`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_ttl(42).expect("set_ttl call failed");
    /// assert_eq!(socket.ttl().unwrap(), 42);
    /// ```
    #[inline]
    pub fn ttl(&self) -> IoResult<u32> {
        self.0.ttl()
    }
    #[inline]
    pub fn nodelay(&self) -> IoResult<bool> {
        self.0.nodelay()
    }
    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::set_broadcast`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_broadcast(false).expect("set_broadcast call failed");
    /// assert_eq!(socket.broadcast().unwrap(), false);
    /// ```
    #[inline]
    pub fn broadcast(&self) -> IoResult<bool> {
        // 0x0020 - SO_BROADCAST
        // 0xFFFF - SOL_SOCKET
        Ok(WSAGetSockOption::<u32>(&self.0 .0, 0xFFFF, 0x20).map(|e| e == 1)?)
    }
    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UdpSocket` is a reference to the same socket that this
    /// object references. Both handles will read and write the same port, and
    /// options set on one socket will be propagated to the other.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let socket_clone = socket.try_clone().expect("couldn't clone the socket");
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<UdpSocket> {
        self.0.duplicate().map(|s| UdpSocket(s))
    }
    /// Returns the socket address of the remote peer this socket was connected
    /// to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("192.168.0.1:41203").expect("couldn't connect to address");
    /// assert_eq!(socket.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 41203)));
    /// ```
    ///
    /// If the socket isn't connected, it will return a [`NotConnected`] error.
    ///
    /// [`NotConnected`]: ErrorKind::NotConnected
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// assert_eq!(socket.peer_addr().unwrap_err().kind(),
    ///            xrmt_stx::io::ErrorKind::NotConnected);
    /// ```
    #[inline]
    pub fn peer_addr(&self) -> IoResult<SocketAddr> {
        self.0.peer_addr()
    }
    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_ttl(42).expect("set_ttl call failed");
    /// ```
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> IoResult<()> {
        self.0.set_ttl(ttl)
    }
    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::set_multicast_ttl_v4`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_ttl_v4(42).expect("set_multicast_ttl_v4 call failed");
    /// assert_eq!(socket.multicast_ttl_v4().unwrap(), 42);
    /// ```
    #[inline]
    pub fn multicast_ttl_v4(&self) -> IoResult<u32> {
        // 0x0 - IP_PROTO_IP
        // 0xA - IP_MULTICAST_TTL
        Ok(WSAGetSockOption::<u32>(&self.0 .0, 0, 0xA)?)
    }
    // Returns the socket address that this socket was created from.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// assert_eq!(socket.local_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 34254)));
    /// ```
    #[inline]
    pub fn local_addr(&self) -> IoResult<SocketAddr> {
        self.0.socket_addr()
    }
    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::set_multicast_loop_v4`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v4(false).expect("set_multicast_loop_v4 call failed");
    /// assert_eq!(socket.multicast_loop_v4().unwrap(), false);
    /// ```
    #[inline]
    pub fn multicast_loop_v4(&self) -> IoResult<bool> {
        // 0x0 - IP_PROTO_IP
        // 0xB - IP_MULTICAST_LOOP
        Ok(WSAGetSockOption::<u32>(&self.0 .0, 0, 0xB).map(|e| e == 1)?)
    }
    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::set_multicast_loop_v6`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v6(false).expect("set_multicast_loop_v6 call failed");
    /// assert_eq!(socket.multicast_loop_v6().unwrap(), false);
    /// ```
    #[inline]
    pub fn multicast_loop_v6(&self) -> IoResult<bool> {
        // 0x29 - IP_PROTO_IPV6
        // 0x0B - IP_MULTICAST_LOOP
        Ok(WSAGetSockOption::<u32>(&self.0 .0, 0x29, 0xB).map(|e| e == 1)?)
    }
    /// Sends data on the socket to the remote address to which it is connected.
    /// On success, returns the number of bytes written. Note that the operating
    /// system may refuse buffers larger than 65507. However, partial writes are
    /// not possible until buffer sizes above `i32::MAX`.
    ///
    /// [`UdpSocket::connect`] will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// socket.send(&[0, 1, 2]).expect("couldn't send message");
    /// ```
    #[inline]
    pub fn send(&self, buf: &[u8]) -> IoResult<usize> {
        self.0.send(0, buf)
    }
    #[inline]
    pub fn linger(&self) -> IoResult<Option<Duration>> {
        self.0.linger()
    }
    /// Gets the value of the `SO_ERROR` option on this socket.
    ///
    /// This will retrieve the stored error in the underlying socket, clearing
    /// the field in the process. This can be useful for checking errors between
    /// calls.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// match socket.take_error() {
    ///     Ok(Some(error)) => println!("UdpSocket error: {error:?}"),
    ///     Ok(None) => println!("No error"),
    ///     Err(error) => println!("UdpSocket.take_error failed: {error:?}"),
    /// }
    /// ```
    #[inline]
    pub fn take_error(&self) -> IoResult<Option<IoError>> {
        self.0.take_error()
    }
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        self.0.shutdown(how)
    }
    /// Receives single datagram on the socket from the remote address to which
    /// it is connected, without removing the message from input queue. On
    /// success, returns the number of bytes peeked.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in
    /// the supplied buffer, excess bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use
    /// `libc::poll` to synchronize IO events on one or more sockets.
    ///
    /// [`UdpSocket::connect`] will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Errors
    ///
    /// This method will fail if the socket is not connected. The `connect`
    /// method will connect this socket to a remote address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// let mut buf = [0; 10];
    /// match socket.peek(&mut buf) {
    ///     Ok(received) => println!("received {received} bytes"),
    ///     Err(e) => println!("peek function failed: {e:?}"),
    /// }
    /// ```
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.peek(buf)
    }
    /// Receives a single datagram message on the socket from the remote address
    /// to which it is connected. On success, returns the number of bytes
    /// read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in
    /// the supplied buffer, excess bytes may be discarded.
    ///
    /// [`UdpSocket::connect`] will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// let mut buf = [0; 10];
    /// match socket.recv(&mut buf) {
    ///     Ok(received) => println!("received {received} bytes {:?}", &buf[..received]),
    ///     Err(e) => println!("recv function failed: {e:?}"),
    /// }
    /// ```
    #[inline]
    pub fn recv(&self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.recv(0, buf)
    }
    /// Returns the read timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`read`] calls will block indefinitely.
    ///
    /// [`read`]: xrmt_io::Read::read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_read_timeout(None).expect("set_read_timeout call failed");
    /// assert_eq!(socket.read_timeout().unwrap(), None);
    /// ```
    #[inline]
    pub fn read_timeout(&self) -> IoResult<Option<Duration>> {
        // 0x1006 - SO_RCVTIMEO
        self.0.timeout(0x1006)
    }
    /// Returns the write timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`write`] calls will block
    /// indefinitely.
    ///
    /// [`write`]: xrmt_io::Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_write_timeout(None).expect("set_write_timeout call failed");
    /// assert_eq!(socket.write_timeout().unwrap(), None);
    /// ```
    #[inline]
    pub fn write_timeout(&self) -> IoResult<Option<Duration>> {
        // 0x1005 - SO_SNDTIMEO
        self.0.timeout(0x1005)
    }
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> IoResult<()> {
        self.0.set_nodelay(no_delay)
    }
    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_broadcast(false).expect("set_broadcast call failed");
    /// ```
    #[inline]
    pub fn set_broadcast(&self, broadcast: bool) -> IoResult<()> {
        // 0x0020 - SO_BROADCAST
        // 0xFFFF - SOL_SOCKET
        Ok(WSASetSockOption(
            &self.0 .0,
            0xFFFF,
            0x20,
            if broadcast { 1 } else { 0 },
        )?)
    }
    // Connects this UDP socket to a remote address, allowing the `send` and
    /// `recv` syscalls to be used to send data and also applies filters to only
    /// receive data from the specified address.
    ///
    /// If `addr` yields multiple addresses, `connect` will be attempted with
    /// each of the addresses until the underlying OS function returns no
    /// error. Note that usually, a successful `connect` call does not specify
    /// that there is a remote server listening on the port, rather, such an
    /// error would only be detected after the first send. If the OS returns an
    /// error for each of the specified addresses, the error returned from the
    /// last connection attempt (the last address) is returned.
    ///
    /// # Examples
    ///
    /// Creates a UDP socket bound to `127.0.0.1:3400` and connect the socket to
    /// `127.0.0.1:8080`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:3400").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// ```
    ///
    /// Unlike in the TCP case, passing an array of addresses to the `connect`
    /// function of a UDP socket is not a useful thing to do: The OS will be
    /// unable to determine whether something is listening on the remote
    /// address without the application sending data.
    ///
    /// If your first `connect` is to a loopback address, subsequent
    /// `connect`s to non-loopback addresses might fail, depending
    /// on the platform.
    pub fn connect(&self, addr: impl ToSocketAddrs) -> IoResult<()> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match self.0.connect(&a) {
                Ok(_) => return Ok(()),
                Err(e) => l = Some(e),
            }
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }
    #[inline]
    /// Moves this UDP socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to` system
    /// operations becoming nonblocking, i.e., immediately returning from their
    /// calls. If the IO operation is successful, `Ok` is returned and no
    /// further action is required. If the IO operation could not be completed
    /// and needs to be retried, an error with kind
    /// [`xrmt_io::ErrorKind::WouldBlock`] is returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl`
    /// `FIONBIO`. On Windows calling this method corresponds to calling
    /// `ioctlsocket` `FIONBIO`.
    ///
    /// # Examples
    ///
    /// Creates a UDP socket bound to `127.0.0.1:7878` and read bytes in
    /// nonblocking mode:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:7878").unwrap();
    /// socket.set_nonblocking(true).unwrap();
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// let mut buf = [0; 10];
    /// let (num_bytes_read, _) = loop {
    ///     match socket.recv_from(&mut buf) {
    ///         Ok(n) => break n,
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // wait until network socket is ready, typically implemented
    ///             // via platform-specific APIs such as epoll or IOCP
    ///             wait_for_fd();
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     }
    /// };
    /// println!("bytes: {:?}", &buf[..num_bytes_read]);
    /// ```
    pub fn set_nonblocking(&self, non_blocking: bool) -> IoResult<()> {
        self.0.set_nonblocking(non_blocking)
    }
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> IoResult<()> {
        self.0.set_linger(linger)
    }
    /// Sets the read timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`read`] calls will block
    /// indefinitely. An [`Err`] is returned if the zero [`Duration`] is
    /// passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a read times out as
    /// a result of setting this option. For example Unix typically returns an
    /// error of the kind [`WouldBlock`], but Windows may return [`TimedOut`].
    ///
    /// [`read`]: xrmt_io::Read::read
    /// [`WouldBlock`]: ErrorKind::WouldBlock
    /// [`TimedOut`]: ErrorKind::TimedOut
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_read_timeout(None).expect("set_read_timeout call failed");
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::UdpSocket;
    /// use xrmt_stx::time::Duration;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    /// let result = socket.set_read_timeout(Some(Duration::new(0, 0)));
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), io::ErrorKind::InvalidInput)
    /// ```
    #[inline]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
        // 0x1006 - SO_RCVTIMEO
        self.0.set_timeout(0x1006, dur)
    }
    /// Sets the write timeout to the timeout specified.
    ///
    /// If the value specified is [`None`], then [`write`] calls will block
    /// indefinitely. An [`Err`] is returned if the zero [`Duration`] is
    /// passed to this method.
    ///
    /// # Platform-specific behavior
    ///
    /// Platforms may return a different error code whenever a write times out
    /// as a result of setting this option. For example Unix typically returns
    /// an error of the kind [`WouldBlock`], but Windows may return
    /// [`TimedOut`].
    ///
    /// [`write`]: xrmt_io::Write::write
    /// [`WouldBlock`]: ErrorKind::WouldBlock
    /// [`TimedOut`]: ErrorKind::TimedOut
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_write_timeout(None).expect("set_write_timeout call failed");
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::UdpSocket;
    /// use xrmt_stx::time::Duration;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    /// let result = socket.set_write_timeout(Some(Duration::new(0, 0)));
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), io::ErrorKind::InvalidInput)
    /// ```
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
        // 0x1005 - SO_SNDTIMEO
        self.0.set_timeout(0x1005, dur)
    }
    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in
    /// the supplied buffer, excess bytes may be discarded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let mut buf = [0; 10];
    /// let (number_of_bytes, src_addr) = socket.recv_from(&mut buf)
    ///                                         .expect("Didn't receive data");
    /// let filled_buf = &mut buf[..number_of_bytes];
    /// ```
    #[inline]
    pub fn recv_from(&self, buf: &mut [u8]) -> IoResult<(usize, SocketAddr)> {
        self.0.recv_from(0, buf)
    }
    /// Receives a single datagram message on the socket, without removing it
    /// from the queue. On success, returns the number of bytes read and the
    /// origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in
    /// the supplied buffer, excess bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recvfrom` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use
    /// `libc::poll` to synchronize IO events on one or more sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let mut buf = [0; 10];
    /// let (number_of_bytes, src_addr) = socket.peek_from(&mut buf)
    ///                                         .expect("Didn't receive data");
    /// let filled_buf = &mut buf[..number_of_bytes];
    /// ```
    #[inline]
    pub fn peek_from(&self, buf: &mut [u8]) -> IoResult<(usize, SocketAddr)> {
        self.0.peek_from(buf)
    }
    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// Note that this might not have any effect on IPv6 sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_ttl_v4(42).expect("set_multicast_ttl_v4 call failed");
    /// ```
    #[inline]
    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> IoResult<()> {
        // 0x0 - IP_PROTO_IP
        // 0xA - IP_MULTICAST_TTL
        Ok(WSASetSockOption(&self.0 .0, 0, 0xA, multicast_ttl_v4)?)
    }
    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    /// Note that this might not have any affect on IPv4 sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v6(false).expect("set_multicast_loop_v6 call failed");
    /// ```
    #[inline]
    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> IoResult<()> {
        // 0x29 - IP_PROTO_IPV6
        // 0x0B - IP_MULTICAST_LOOP
        Ok(WSASetSockOption(
            &self.0 .0,
            0x29,
            0xB,
            if multicast_loop_v6 { 1 } else { 0 },
        )?)
    }
    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    /// Note that this might not have any effect on IPv6 sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v4(false).expect("set_multicast_loop_v4 call failed");
    /// ```
    #[inline]
    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> IoResult<()> {
        // 0x0 - IP_PROTO_IP
        // 0xB - IP_MULTICAST_LOOP
        Ok(WSASetSockOption(
            &self.0 .0,
            0,
            0xB,
            if multicast_loop_v4 { 1 } else { 0 },
        )?)
    }
    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written. Note that the operating system may refuse
    /// buffers larger than 65507. However, partial writes are not possible
    /// until buffer sizes above `i32::MAX`.
    ///
    /// Address type can be any implementor of [`ToSocketAddrs`] trait. See its
    /// documentation for concrete examples.
    ///
    /// It is possible for `addr` to yield multiple addresses, but `send_to`
    /// will only send data to the first address yielded by `addr`.
    ///
    /// This will return an error when the IP version of the local socket
    /// does not match that returned from [`ToSocketAddrs`].
    ///
    /// See [Issue #34202] for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.send_to(&[0; 10], "127.0.0.1:4242").expect("couldn't send data");
    /// ```
    ///
    /// [Issue #34202]: https://github.com/rust-lang/rust/issues/34202
    pub fn send_to(&self, buf: &[u8], addr: impl ToSocketAddrs) -> IoResult<usize> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match self.0.send_to(&a, 0, buf) {
                Ok(s) => return Ok(s),
                Err(e) => l = Some(e),
            }
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }
    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    #[inline]
    pub fn join_multicast_v6(&self, multi_addr: &Ipv6Addr, interface: u32) -> IoResult<()> {
        let v = IP6Group {
            interface,
            addr: multi_addr.octets(),
        };
        // 0x29 - IP_PROTO_IPV6
        // 0x0C - IPV6_ADD_MEMBERSHIP
        Ok(WSASetSockOption(&self.0 .0, 0x29, 0xC, v)?)
    }
    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::join_multicast_v6`].
    #[inline]
    pub fn leave_multicast_v6(&self, multi_addr: &Ipv6Addr, interface: u32) -> IoResult<()> {
        let v = IP6Group {
            interface,
            addr: multi_addr.octets(),
        };
        // 0x29 - IP_PROTO_IPV6
        // 0x0D - IP_DROP_MEMBERSHIP
        Ok(WSASetSockOption(&self.0 .0, 0x29, 0xD, v)?)
    }
    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to [`UNSPECIFIED`](Ipv4Addr::UNSPECIFIED)
    /// then an appropriate interface is chosen by the system.
    #[inline]
    pub fn join_multicast_v4(&self, multi_addr: &Ipv4Addr, interface: &Ipv4Addr) -> IoResult<()> {
        let v = [
            u32::from_ne_bytes(multi_addr.octets()),
            u32::from_ne_bytes(interface.octets()),
        ];
        // 0x0 - IP_PROTO_IPV
        // 0xC - IP_ADD_MEMBERSHIP
        Ok(WSASetSockOption(&self.0 .0, 0, 0xB, v)?)
    }
    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see
    /// [`UdpSocket::join_multicast_v4`].
    #[inline]
    pub fn leave_multicast_v4(&self, multi_addr: &Ipv4Addr, interface: &Ipv4Addr) -> IoResult<()> {
        let v = [
            u32::from_ne_bytes(multi_addr.octets()),
            u32::from_ne_bytes(interface.octets()),
        ];
        // 0x0 - IP_PROTO_IPV
        // 0xD - IP_DROP_MEMBERSHIP
        Ok(WSASetSockOption(&self.0 .0, 0, 0xD, v)?)
    }

    fn try_bind(addr: &SocketAddr) -> IoResult<UdpSocket> {
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
}

impl AsRef<OwnedSocket> for UdpSocket {
    #[inline]
    fn as_ref(&self) -> &OwnedSocket {
        &self.0 .0
    }
}
impl From<OwnedSocket> for UdpSocket {
    #[inline]
    fn from(v: OwnedSocket) -> UdpSocket {
        UdpSocket(Socket(v))
    }
}
