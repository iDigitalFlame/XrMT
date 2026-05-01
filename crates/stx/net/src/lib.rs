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

//! Networking primitives for TCP/UDP communication.
//!
//! This module provides networking functionality for the Transmission Control
//! and User Datagram Protocols, as well as types for IP and socket addresses.
//!
//! # Organization
//!
//! * [`TcpListener`] and [`TcpStream`] provide functionality for communication
//!   over TCP
//! * [`UdpSocket`] provides functionality for communication over UDP
//! * [`IpAddr`] represents IP addresses of either IPv4 or IPv6; [`Ipv4Addr`]
//!   and [`Ipv6Addr`] are respectively IPv4 and IPv6 addresses
//! * [`SocketAddr`] represents socket addresses of either IPv4 or IPv6;
//!   [`SocketAddrV4`] and [`SocketAddrV6`] are respectively IPv4 and IPv6
//!   socket addresses
//! * [`ToSocketAddrs`] is a trait that is used for generic address resolution
//!   when interacting with networking objects like [`TcpListener`],
//!   [`TcpStream`] or [`UdpSocket`]
//! * Other types are return or parameter types for various methods in this
//!   module
//!
//! Rust disables inheritance of socket objects to child processes by default
//! when possible.  For example, through the use of the `CLOEXEC` flag in UNIX
//! systems or the `HANDLE_FLAG_INHERIT` flag on Windows.

#![no_implicit_prelude]
#![no_std]
#![cfg_attr(
    all(target_family = "windows", not(feature = "std")),
    feature(allocator_api, likely_unlikely)
)]

pub use self::net::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "."]
mod net {
    extern crate core;

    mod resolve;
    mod socket;
    mod tcp;
    mod udp;

    pub use self::resolve::*;
    pub use self::socket::Shutdown;
    pub use self::tcp::*;
    pub use self::udp::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use core::net::*;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod net {
    extern crate std;
    pub use std::net::*;
}
