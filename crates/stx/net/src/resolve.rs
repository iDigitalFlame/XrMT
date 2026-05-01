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

extern crate alloc;
extern crate core;

extern crate xrmt_data;
extern crate xrmt_io;
extern crate xrmt_winapi;

use alloc::string::String;
use core::alloc::Allocator;
use core::convert::{From, Into, TryFrom, TryInto};
use core::hint::unlikely;
use core::iter::{Copied, FusedIterator, Iterator};
use core::marker::{Send, Sized, Sync};
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::slice::Iter;
use core::str::from_utf8_unchecked;

use xrmt_data::text::parse_u16;
use xrmt_data::Fiber;
use xrmt_io::{ErrorKind, IoError, IoResult};
use xrmt_winapi::functions::WSAGetAddrInfo;
use xrmt_winapi::structs::AddressResolverUnsafe;

pub struct Resolver {
    v:    Type,
    port: u16,
}

/// A trait for objects which can be converted or resolved to one or more
/// [`SocketAddr`] values.
///
/// This trait is used for generic address resolution when constructing network
/// objects. By default it is implemented for the following types:
///
///  * [`SocketAddr`]: [`to_socket_addrs`] is the identity function.
///
///  * [`SocketAddrV4`], [`SocketAddrV6`], <code>([IpAddr], [u16])</code>,
///    <code>([Ipv4Addr], [u16])</code>, <code>([Ipv6Addr], [u16])</code>:
///    [`to_socket_addrs`] constructs a [`SocketAddr`] trivially.
///
///  * <code>(&[str], [u16])</code>: <code>&[str]</code> should be either a
///    string representation of an [`IpAddr`] address as expected by [`FromStr`]
///    implementation or a host name. [`u16`] is the port number.
///
///  * <code>&[str]</code>: the string should be either a string representation
///    of a [`SocketAddr`] as expected by its [`FromStr`] implementation or a
///    string like `<host_name>:<port>` pair where `<port>` is a [`u16`] value.
///
/// This trait allows constructing network objects like [`TcpStream`] or
/// [`UdpSocket`] easily with values of various types for the bind/connection
/// address. It is needed because sometimes one type is more appropriate than
/// the other: for simple uses a string like `"localhost:12345"` is much nicer
/// than manual construction of the corresponding [`SocketAddr`], but sometimes
/// [`SocketAddr`] value is *the* main source of the address, and converting it
/// to some other type (e.g., a string) just for it to be converted back to
/// [`SocketAddr`] in constructor methods is pointless.
///
/// Addresses returned by the operating system that are not IP addresses are
/// silently ignored.
///
/// [`FromStr`]: alloc::str::FromStr "alloc::str::FromStr"
/// [`TcpStream`]: crate::net::TcpStream "net::TcpStream"
/// [`to_socket_addrs`]: ToSocketAddrs::to_socket_addrs
/// [`UdpSocket`]: crate::net::UdpSocket "net::UdpSocket"
///
/// # Examples
///
/// Creating a [`SocketAddr`] iterator that yields one item:
///
/// ```
/// use xrmt_stx::net::{ToSocketAddrs, SocketAddr};
///
/// let addr = SocketAddr::from(([127, 0, 0, 1], 443));
/// let mut addrs_iter = addr.to_socket_addrs().unwrap();
///
/// assert_eq!(Some(addr), addrs_iter.next());
/// assert!(addrs_iter.next().is_none());
/// ```
///
/// Creating a [`SocketAddr`] iterator from a hostname:
///
/// ```no_run
/// use xrmt_stx::net::{SocketAddr, ToSocketAddrs};
///
/// // assuming 'localhost' resolves to 127.0.0.1
/// let mut addrs_iter = "localhost:443".to_socket_addrs().unwrap();
/// assert_eq!(addrs_iter.next(), Some(SocketAddr::from(([127, 0, 0, 1], 443))));
/// assert!(addrs_iter.next().is_none());
///
/// // assuming 'foo' does not resolve
/// assert!("foo:443".to_socket_addrs().is_err());
/// ```
///
/// Creating a [`SocketAddr`] iterator that yields multiple items:
///
/// ```
/// use xrmt_stx::net::{SocketAddr, ToSocketAddrs};
///
/// let addr1 = SocketAddr::from(([0, 0, 0, 0], 80));
/// let addr2 = SocketAddr::from(([127, 0, 0, 1], 443));
/// let addrs = vec![addr1, addr2];
///
/// let mut addrs_iter = (&addrs[..]).to_socket_addrs().unwrap();
///
/// assert_eq!(Some(addr1), addrs_iter.next());
/// assert_eq!(Some(addr2), addrs_iter.next());
/// assert!(addrs_iter.next().is_none());
/// ```
///
/// Attempting to create a [`SocketAddr`] iterator from an improperly formatted
/// socket address `&str` (missing the port):
///
/// ```
/// use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::net::ToSocketAddrs;
///
/// let err = "127.0.0.1".to_socket_addrs().unwrap_err();
/// assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
/// ```
///
/// [`TcpStream::connect`] is an example of an function that utilizes
/// `ToSocketAddrs` as a trait bound on its parameter in order to accept
/// different types:
///
/// ```no_run
/// use xrmt_stx::net::{TcpStream, Ipv4Addr};
///
/// let stream = TcpStream::connect(("127.0.0.1", 443));
/// // or
/// let stream = TcpStream::connect("127.0.0.1:443");
/// // or
/// let stream = TcpStream::connect((Ipv4Addr::new(127, 0, 0, 1), 443));
/// ```
///
/// [`TcpStream::connect`]: crate::net::TcpStream::connect
pub trait ToSocketAddrs {
    /// Returned iterator over socket addresses which this type may correspond
    /// to.
    type Iter: Iterator<Item = SocketAddr>;

    /// Converts this object to an iterator of resolved [`SocketAddr`]s.
    ///
    /// The returned iterator might not actually yield any values depending on
    /// the outcome of any resolution performed.
    ///
    /// Note that this function may block the current thread while resolution is
    /// performed.
    fn to_socket_addrs(&self) -> IoResult<Self::Iter>;
}

enum Type {
    Fixed(Option<SocketAddr>),
    Resolve(AddressResolverUnsafe),
}

impl Iterator for Resolver {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        match &mut self.v {
            Type::Fixed(v) => v.take(),
            Type::Resolve(v) => v.next(),
        }
        .map(|mut a| {
            if self.port > 0 {
                a.set_port(self.port);
            }
            a
        })
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.v {
            Type::Fixed(v) if v.is_some() => (1, Some(1)),
            _ => (0, None),
        }
    }
}
impl FusedIterator for Resolver {}

impl From<SocketAddr> for Resolver {
    #[inline]
    fn from(v: SocketAddr) -> Resolver {
        Resolver {
            v:    Type::Fixed(Some(v)),
            port: 0,
        }
    }
}
impl From<(SocketAddr, u16)> for Resolver {
    #[inline]
    fn from(v: (SocketAddr, u16)) -> Resolver {
        Resolver {
            v:    Type::Fixed(Some(v.0)),
            port: v.1,
        }
    }
}
impl From<&(SocketAddr, u16)> for Resolver {
    #[inline]
    fn from(v: &(SocketAddr, u16)) -> Resolver {
        Resolver {
            v:    Type::Fixed(Some(v.0)),
            port: v.1,
        }
    }
}

impl<'a> TryFrom<&'a str> for Resolver {
    type Error = IoError;

    #[inline]
    fn try_from(s: &'a str) -> IoResult<Resolver> {
        let b = s.as_bytes();
        let i = b.len().saturating_sub(
            b.iter()
                .rev()
                .position(|v| *v == b':')
                .ok_or_else(|| IoError::from(ErrorKind::InvalidInput))?,
        );
        if i <= 1 || unlikely(i >= b.len()) {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        let (n, v) = unsafe { b.split_at_unchecked(i) };
        let p = unsafe { parse_u16(from_utf8_unchecked(v.get_unchecked(1..))).ok_or_else(|| IoError::from(ErrorKind::InvalidInput))? };
        let r = WSAGetAddrInfo(unsafe { from_utf8_unchecked(n) }, 1, 0, 0)?;
        Ok(Resolver {
            v:    Type::Resolve(r.into()),
            port: p,
        })
    }
}
impl<'a> TryFrom<(&'a str, u16)> for Resolver {
    type Error = IoError;

    #[inline]
    fn try_from(v: (&'a str, u16)) -> IoResult<Resolver> {
        Ok(WSAGetAddrInfo(v.0, 1, 0, 0).map(|i| Resolver {
            v:    Type::Resolve(i.into()),
            port: v.1,
        })?)
    }
}
impl<'a> TryFrom<&(&'a str, u16)> for Resolver {
    type Error = IoError;

    #[inline]
    fn try_from(v: &(&'a str, u16)) -> IoResult<Resolver> {
        (*v).try_into()
    }
}

impl ToSocketAddrs for str {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        match self.parse::<SocketAddr>() {
            Ok(v) => Ok(v.into()),
            _ => Resolver::try_from(self),
        }
    }
}
impl ToSocketAddrs for String {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        (&**self).to_socket_addrs()
    }
}
impl ToSocketAddrs for SocketAddr {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        Ok((*self).into())
    }
}
impl ToSocketAddrs for (&str, u16) {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        match self.0.parse::<SocketAddr>() {
            Ok(v) => Ok((v, self.1).into()),
            _ => Resolver::try_from(self),
        }
    }
}
impl ToSocketAddrs for SocketAddrV4 {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        SocketAddr::V4(*self).to_socket_addrs()
    }
}
impl ToSocketAddrs for SocketAddrV6 {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        SocketAddr::V6(*self).to_socket_addrs()
    }
}
impl ToSocketAddrs for (IpAddr, u16) {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        match self.0 {
            IpAddr::V4(a) => (a, self.1).to_socket_addrs(),
            IpAddr::V6(a) => (a, self.1).to_socket_addrs(),
        }
    }
}
impl ToSocketAddrs for (String, u16) {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        (&*self.0, self.1).to_socket_addrs()
    }
}
impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        SocketAddrV4::new(self.0, self.1).to_socket_addrs()
    }
}
impl ToSocketAddrs for (Ipv6Addr, u16) {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        SocketAddrV6::new(self.0, self.1, 0, 0).to_socket_addrs()
    }
}
impl<'a> ToSocketAddrs for &'a [SocketAddr] {
    type Iter = Copied<Iter<'a, SocketAddr>>;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Self::Iter> {
        Ok(self.iter().copied())
    }
}
impl<A: Allocator> ToSocketAddrs for Fiber<A> {
    type Iter = Resolver;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<Resolver> {
        self.as_str().to_socket_addrs()
    }
}
impl<T: ToSocketAddrs + ?Sized> ToSocketAddrs for &T {
    type Iter = T::Iter;

    #[inline]
    fn to_socket_addrs(&self) -> IoResult<T::Iter> {
        (**self).to_socket_addrs()
    }
}

unsafe impl Sync for Resolver {}
unsafe impl Send for Resolver {}
