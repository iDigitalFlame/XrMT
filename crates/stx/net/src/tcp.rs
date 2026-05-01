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
use core::iter::Iterator;
use core::net::SocketAddr;
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::time::Duration;

use xrmt_io::{ErrorKind, IoError, IoResult, Read, Write};
use xrmt_winapi::structs::OwnedSocket;

use crate::net::socket::Socket;
use crate::{Shutdown, ToSocketAddrs};

/// A TCP stream between a local and a remote socket.
///
/// After creating a `TcpStream` by either [`connect`]ing to a remote host or
/// [`accept`]ing a connection on a [`TcpListener`], data can be transmitted
/// by [reading] and [writing] to it.
///
/// The connection will be closed when the value is dropped. The reading and
/// writing portions of the connection can also be shut down individually with
/// the [`shutdown`] method.
///
/// The Transmission Control Protocol is specified in [IETF RFC 793].
///
/// [`accept`]: TcpListener::accept
/// [`connect`]: TcpStream::connect
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
/// [reading]: Read
/// [`shutdown`]: TcpStream::shutdown
/// [writing]: Write
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::net::TcpStream;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let mut stream = TcpStream::connect("127.0.0.1:34254")?;
///
///     stream.write(&[1])?;
///     stream.read(&mut [0; 128])?;
///     Ok(())
/// } // the stream is closed here
/// ```
pub struct TcpStream(Socket);
/// A TCP socket server, listening for connections.
///
/// After creating a `TcpListener` by [`bind`]ing it to a socket address, it
/// listens for incoming TCP connections. These can be accepted by calling
/// [`accept`] or by iterating over the [`Incoming`] iterator returned by
/// [`incoming`][`TcpListener::incoming`].
///
/// The socket will be closed when the value is dropped.
///
/// The Transmission Control Protocol is specified in [IETF RFC 793].
///
/// [`accept`]: TcpListener::accept
/// [`bind`]: TcpListener::bind
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::net::{TcpListener, TcpStream};
///
/// fn handle_client(stream: TcpStream) {
///     // ...
/// }
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let listener = TcpListener::bind("127.0.0.1:80")?;
///
///     // accept connections and process them serially
///     for stream in listener.incoming() {
///         handle_client(stream?);
///     }
///     Ok(())
/// }
/// ```
pub struct TcpListener(Socket);
/// An iterator that infinitely [`accept`]s connections on a [`TcpListener`].
///
/// This `struct` is created by the [`TcpListener::into_incoming`] method.
/// See its documentation for more.
///
/// [`accept`]: TcpListener::accept
pub struct IntoIncoming(TcpListener);
/// An iterator that infinitely [`accept`]s connections on a [`TcpListener`].
///
/// This `struct` is created by the [`TcpListener::incoming`] method.
/// See its documentation for more.
///
/// [`accept`]: TcpListener::accept
pub struct Incoming<'a>(&'a TcpListener);

impl TcpStream {
    /// Opens a TCP connection to a remote host.
    ///
    /// `addr` is an address of the remote host. Anything which implements
    /// [`ToSocketAddrs`] trait can be supplied for the address; see this trait
    /// documentation for concrete examples.
    ///
    /// If `addr` yields multiple addresses, `connect` will be attempted with
    /// each of the addresses until a connection is successful. If none of
    /// the addresses result in a successful connection, the error returned from
    /// the last connection attempt (the last address) is returned.
    ///
    /// # Examples
    ///
    /// Open a TCP connection to `127.0.0.1:8080`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// if let Ok(stream) = TcpStream::connect("127.0.0.1:8080") {
    ///     println!("Connected to the server!");
    /// } else {
    ///     println!("Couldn't connect to server...");
    /// }
    /// ```
    ///
    /// Open a TCP connection to `127.0.0.1:8080`. If the connection fails, open
    /// a TCP connection to `127.0.0.1:8081`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::{SocketAddr, TcpStream};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 8080)),
    ///     SocketAddr::from(([127, 0, 0, 1], 8081)),
    /// ];
    /// if let Ok(stream) = TcpStream::connect(&addrs[..]) {
    ///     println!("Connected to the server!");
    /// } else {
    ///     println!("Couldn't connect to server...");
    /// }
    /// ```
    pub fn connect(addr: impl ToSocketAddrs) -> IoResult<TcpStream> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            l = match TcpStream::try_connect(&a) {
                Ok(s) => return Ok(s),
                Err(e) => Some(e),
            };
        }
        match l {
            None => Err(IoError::from(ErrorKind::AddrNotAvailable)),
            Some(e) => Err(e),
        }
    }
    /// Opens a TCP connection to a remote host with a timeout.
    ///
    /// Unlike `connect`, `connect_timeout` takes a single [`SocketAddr`] since
    /// timeout must be applied to individual addresses.
    ///
    /// It is an error to pass a zero `Duration` to this function.
    ///
    /// Unlike other methods on `TcpStream`, this does not correspond to a
    /// single system call. It instead calls `connect` in nonblocking mode and
    /// then uses an OS-specific mechanism to await the completion of the
    /// connection request.
    #[inline]
    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> IoResult<TcpStream> {
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

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`TcpStream::set_ttl`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// assert_eq!(stream.ttl().unwrap_or(0), 100);
    /// ```
    #[inline]
    pub fn ttl(&self) -> IoResult<u32> {
        self.0.ttl()
    }
    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`TcpStream::set_nodelay`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// assert_eq!(stream.nodelay().unwrap_or(false), true);
    /// ```
    #[inline]
    pub fn nodelay(&self) -> IoResult<bool> {
        self.0.nodelay()
    }
    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `TcpStream` is a reference to the same stream that this
    /// object references. Both handles will read and write the same stream of
    /// data, and options set on one stream will be propagated to the other
    /// stream.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// let stream_clone = stream.try_clone().expect("clone failed...");
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<TcpStream> {
        self.0.duplicate().map(TcpStream)
    }
    /// Returns the socket address of the remote peer of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
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
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// ```
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> IoResult<()> {
        self.0.set_ttl(ttl)
    }
    /// Returns the socket address of the local half of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{IpAddr, Ipv4Addr, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.local_addr().unwrap().ip(),
    ///            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    /// ```
    #[inline]
    pub fn local_addr(&self) -> IoResult<SocketAddr> {
        self.0.socket_addr()
    }
    /// Gets the value of the `SO_LINGER` option on this socket.
    ///
    /// For more information about this option, see [`TcpStream::set_linger`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(tcp_linger)]
    ///
    /// use xrmt_stx::net::TcpStream;
    /// use xrmt_stx::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_linger(Some(Duration::from_secs(0))).expect("set_linger call failed");
    /// assert_eq!(stream.linger().unwrap(), Some(Duration::from_secs(0)));
    /// ```
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
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.take_error().expect("No error was expected...");
    /// ```
    #[inline]
    pub fn take_error(&self) -> IoResult<Option<IoError>> {
        self.0.take_error()
    }
    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value (see the
    /// documentation of [`Shutdown`]).
    ///
    /// # Platform-specific behavior
    ///
    /// Calling this function multiple times may result in different behavior,
    /// depending on the operating system. On Linux, the second call will
    /// return `Ok(())`, but on macOS, it will return `ErrorKind::NotConnected`.
    /// This may change in the future.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{Shutdown, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.shutdown(Shutdown::Both).expect("shutdown call failed");
    /// ```
    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        self.0.shutdown(how)
    }
    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8000")
    ///                        .expect("Couldn't connect to the server...");
    /// let mut buf = [0; 10];
    /// let len = stream.peek(&mut buf).expect("peek failed");
    /// ```
    #[inline]
    pub fn peek(&self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.peek(buf)
    }
    /// Returns the read timeout of this socket.
    ///
    /// If the timeout is [`None`], then [`read`] calls will block indefinitely.
    ///
    /// # Platform-specific behavior
    ///
    /// Some platforms do not provide access to the current timeout.
    ///
    /// [`read`]: Read::read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_read_timeout(None).expect("set_read_timeout call failed");
    /// assert_eq!(stream.read_timeout().unwrap(), None);
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
    /// # Platform-specific behavior
    ///
    /// Some platforms do not provide access to the current timeout.
    ///
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_write_timeout(None).expect("set_write_timeout call failed");
    /// assert_eq!(stream.write_timeout().unwrap(), None);
    /// ```
    #[inline]
    pub fn write_timeout(&self) -> IoResult<Option<Duration>> {
        // 0x1005 - SO_SNDTIMEO
        self.0.timeout(0x1005)
    }
    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that
    /// segments are always sent as soon as possible, even if there is only a
    /// small amount of data. When not set, data is buffered until there is a
    /// sufficient amount to send out, thereby avoiding the frequent sending of
    /// small packets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// ```
    #[inline]
    pub fn set_nodelay(&self, no_delay: bool) -> IoResult<()> {
        self.0.set_nodelay(no_delay)
    }
    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// This will result in `read`, `write`, `recv` and `send` system operations
    /// becoming nonblocking, i.e., immediately returning from their calls.
    /// If the IO operation is successful, `Ok` is returned and no further
    /// action is required. If the IO operation could not be completed and needs
    /// to be retried, an error with kind [`xrmt_io::ErrorKind::WouldBlock`] is
    /// returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl`
    /// `FIONBIO`. On Windows calling this method corresponds to calling
    /// `ioctlsocket` `FIONBIO`.
    ///
    /// # Examples
    ///
    /// Reading bytes from a TCP stream in non-blocking mode:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, Read};
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut stream = TcpStream::connect("127.0.0.1:7878")
    ///     .expect("Couldn't connect to the server...");
    /// stream.set_nonblocking(true).expect("set_nonblocking call failed");
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// let mut buf = vec![];
    /// loop {
    ///     match stream.read_to_end(&mut buf) {
    ///         Ok(_) => break,
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // wait until network socket is ready, typically implemented
    ///             // via platform-specific APIs such as epoll or IOCP
    ///             wait_for_fd();
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     };
    /// };
    /// println!("bytes: {buf:?}");
    /// ```
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> IoResult<()> {
        self.0.set_nonblocking(non_blocking)
    }
    // Sets the value of the `SO_LINGER` option on this socket.
    ///
    /// This value controls how the socket is closed when data remains
    /// to be sent. If `SO_LINGER` is set, the socket will remain open
    /// for the specified duration as the system attempts to send pending data.
    /// Otherwise, the system may close the socket immediately, or wait for a
    /// default timeout.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(tcp_linger)]
    ///
    /// use xrmt_stx::net::TcpStream;
    /// use xrmt_stx::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_linger(Some(Duration::from_secs(0))).expect("set_linger call failed");
    /// ```
    #[inline]
    pub fn set_linger(&self, linger: Option<Duration>) -> IoResult<()> {
        self.0.set_linger(linger)
    }
    // Sets the read timeout to the timeout specified.
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
    /// [`read`]: Read::read
    /// [`WouldBlock`]: ErrorKind::WouldBlock
    /// [`TimedOut`]: ErrorKind::TimedOut
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_read_timeout(None).expect("set_read_timeout call failed");
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::TcpStream;
    /// use xrmt_stx::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    /// let result = stream.set_read_timeout(Some(Duration::new(0, 0)));
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
    /// [`write`]: Write::write
    /// [`WouldBlock`]: ErrorKind::WouldBlock
    /// [`TimedOut`]: ErrorKind::TimedOut
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_write_timeout(None).expect("set_write_timeout call failed");
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::TcpStream;
    /// use xrmt_stx::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    /// let result = stream.set_write_timeout(Some(Duration::new(0, 0)));
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), xrmt_io::ErrorKind::InvalidInput)
    /// ```
    #[inline]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
        // 0x1005 - SO_SNDTIMEO
        self.0.set_timeout(0x1005, dur)
    }

    #[inline]
    fn try_connect(addr: &SocketAddr) -> IoResult<TcpStream> {
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
    /// Creates a new `TcpListener` which will be bound to the specified
    /// address.
    ///
    /// The returned listener is ready for accepting connections.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port
    /// to this listener. The port allocated can be queried via the
    /// [`TcpListener::local_addr`] method.
    ///
    /// The address type can be any implementor of [`ToSocketAddrs`] trait. See
    /// its documentation for concrete examples.
    ///
    /// If `addr` yields multiple addresses, `bind` will be attempted with
    /// each of the addresses until one succeeds and returns the listener. If
    /// none of the addresses succeed in creating a listener, the error returned
    /// from the last attempt (the last address) is returned.
    ///
    /// # Examples
    ///
    /// Creates a TCP listener bound to `127.0.0.1:80`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// ```
    ///
    /// Creates a TCP listener bound to `127.0.0.1:80`. If that fails, create a
    /// TCP listener bound to `127.0.0.1:443`:
    ///
    /// ```no_run
    /// use xrmt_stx::net::{SocketAddr, TcpListener};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 80)),
    ///     SocketAddr::from(([127, 0, 0, 1], 443)),
    /// ];
    /// let listener = TcpListener::bind(&addrs[..]).unwrap();
    /// ```
    ///
    /// Creates a TCP listener bound to a port assigned by the operating system
    /// at `127.0.0.1`.
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    /// ```
    pub fn bind(addr: impl ToSocketAddrs) -> IoResult<TcpListener> {
        let mut l = None;
        for a in addr.to_socket_addrs()? {
            match TcpListener::try_bind(&a) {
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
    /// For more information about this option, see [`TcpListener::set_ttl`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.set_ttl(100).expect("could not set TTL");
    /// assert_eq!(listener.ttl().unwrap_or(0), 100);
    /// ```
    #[inline]
    pub fn ttl(&self) -> IoResult<u32> {
        self.0.ttl()
    }
    /// Returns an iterator over the connections being received on this
    /// listener.
    ///
    /// The returned iterator will never return [`None`] and will also not yield
    /// the peer's [`SocketAddr`] structure. Iterating over it is equivalent to
    /// calling [`TcpListener::accept`] in a loop.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{TcpListener, TcpStream};
    ///
    /// fn handle_connection(stream: TcpStream) {
    ///    //...
    /// }
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let listener = TcpListener::bind("127.0.0.1:80")?;
    ///
    ///     for stream in listener.incoming() {
    ///         match stream {
    ///             Ok(stream) => {
    ///                 handle_connection(stream);
    ///             }
    ///             Err(e) => { /* connection failed */ }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self)
    }
    /// Turn this into an iterator over the connections being received on this
    /// listener.
    ///
    /// The returned iterator will never return [`None`] and will also not yield
    /// the peer's [`SocketAddr`] structure. Iterating over it is equivalent to
    /// calling [`TcpListener::accept`] in a loop.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(tcplistener_into_incoming)]
    /// use xrmt_stx::net::{TcpListener, TcpStream};
    ///
    /// fn listen_on(port: u16) -> impl Iterator<Item = TcpStream> {
    ///     let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    ///     listener.into_incoming()
    ///         .filter_map(Result::ok) /* Ignore failed connections */
    /// }
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     for stream in listen_on(80) {
    ///         /* handle the connection here */
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn into_incoming(self) -> IntoIncoming {
        IntoIncoming(self)
    }
    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.set_ttl(100).expect("could not set TTL");
    /// ```
    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> IoResult<()> {
        self.0.set_ttl(ttl)
    }
    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned [`TcpListener`] is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// let listener_clone = listener.try_clone().unwrap();
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<TcpListener> {
        self.0.duplicate().map(TcpListener)
    }
    /// Returns the local socket address of this listener.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// assert_eq!(listener.local_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
    /// ```
    #[inline]
    pub fn local_addr(&self) -> IoResult<SocketAddr> {
        self.0.socket_addr()
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
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.take_error().expect("No error was expected");
    /// ```
    #[inline]
    pub fn take_error(&self) -> IoResult<Option<IoError>> {
        self.0.take_error()
    }
    /// Accept a new incoming connection from this listener.
    ///
    /// This function will block the calling thread until a new TCP connection
    /// is established. When established, the corresponding [`TcpStream`] and
    /// the remote peer's address will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// match listener.accept() {
    ///     Ok((_socket, addr)) => println!("new client: {addr:?}"),
    ///     Err(e) => println!("couldn't get client: {e:?}"),
    /// }
    /// ```
    #[inline]
    pub fn accept(&self) -> IoResult<(TcpStream, SocketAddr)> {
        self.0.accept().map(|v| (TcpStream(v.0), v.1))
    }
    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// This will result in the `accept` operation becoming nonblocking,
    /// i.e., immediately returning from their calls. If the IO operation is
    /// successful, `Ok` is returned and no further action is required. If the
    /// IO operation could not be completed and needs to be retried, an error
    /// with kind [`xrmt_io::ErrorKind::WouldBlock`] is returned.
    ///
    /// On Unix platforms, calling this method corresponds to calling `fcntl`
    /// `FIONBIO`. On Windows calling this method corresponds to calling
    /// `ioctlsocket` `FIONBIO`.
    ///
    /// # Examples
    ///
    /// Bind a TCP listener to an address, listen for connections, and read
    /// bytes in nonblocking mode:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    /// listener.set_nonblocking(true).expect("Cannot set non-blocking");
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// # fn handle_connection(stream: xrmt_stx::net::TcpStream) { unimplemented!() }
    /// for stream in listener.incoming() {
    ///     match stream {
    ///         Ok(s) => {
    ///             // do something with the TcpStream
    ///             handle_connection(s);
    ///         }
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // wait until network socket is ready, typically implemented
    ///             // via platform-specific APIs such as epoll or IOCP
    ///             wait_for_fd();
    ///             continue;
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn set_nonblocking(&self, non_blocking: bool) -> IoResult<()> {
        self.0.set_nonblocking(non_blocking)
    }

    fn try_bind(addr: &SocketAddr) -> IoResult<TcpListener> {
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

impl Read for TcpStream {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.recv(0, buf)
    }
}
impl Read for &TcpStream {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0.recv(0, buf)
    }
}
impl Write for TcpStream {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.send(0, buf)
    }
}
impl Write for &TcpStream {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.send(0, buf)
    }
}
impl From<OwnedSocket> for TcpStream {
    #[inline]
    fn from(v: OwnedSocket) -> TcpStream {
        TcpStream(Socket(v))
    }
}
impl AsRef<OwnedSocket> for TcpStream {
    #[inline]
    fn as_ref(&self) -> &OwnedSocket {
        &self.0 .0
    }
}

impl From<OwnedSocket> for TcpListener {
    #[inline]
    fn from(v: OwnedSocket) -> TcpListener {
        TcpListener(Socket(v))
    }
}
impl AsRef<OwnedSocket> for TcpListener {
    #[inline]
    fn as_ref(&self) -> &OwnedSocket {
        &self.0 .0
    }
}

impl Iterator for IntoIncoming {
    type Item = IoResult<TcpStream>;

    #[inline]
    fn next(&mut self) -> Option<IoResult<TcpStream>> {
        Some(self.0.accept().map(|p| p.0))
    }
}
impl<'a> Iterator for Incoming<'a> {
    type Item = IoResult<TcpStream>;

    #[inline]
    fn next(&mut self) -> Option<IoResult<TcpStream>> {
        Some(self.0.accept().map(|p| p.0))
    }
}
