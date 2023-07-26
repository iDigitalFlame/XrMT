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

use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::{mem, ptr};

use crate::device::winapi::functions::SockAddr;
use crate::device::winapi::{self, CharPtr, WCharPtr, Win32Error, Win32Result};
use crate::util::stx::prelude::*;

#[repr(C)]
pub struct Adapter {
    pub length:                  u32,
    pub index:                   u32,
    pub next:                    *mut Adapter,
    pub name:                    CharPtr,
    pub first_unicast:           *mut UnicastAddress,
    pub first_anycast:           *mut MulticastAddress,
    pub first_multicast:         *mut MulticastAddress,
    pub first_dns:               *mut MulticastAddress,
    pub dns_suffix:              WCharPtr,
    pub description:             WCharPtr,
    pub friendly_name:           WCharPtr,
    pub physical_address:        [u8; 8],
    pub physical_address_length: u32,
    pub flags:                   u32,
    pub mtu:                     u32,
    pub if_type:                 u32,
    pub operating_status:        u32,
    pub index_v6:                u32,
    pub zone_indices:            [u16; 16],
    pub first_prefix:            *mut MulticastAddress,
}
#[repr(C)]
pub struct AddressInfo {
    pub flags:        u32,
    pub family:       u32,
    pub socktype:     u32,
    pub protocol:     u32,
    pub address_size: usize,
    pub canon_name:   CharPtr,
    pub addr:         *mut SocketAddress,
    pub next:         *mut AddressInfo,
}
pub struct DnsIter<'a> {
    base: &'a Adapter,
    cur:  Option<&'a MulticastAddress>,
}
#[repr(C)]
pub struct SocketAddress {
    pub address: *mut SockAddr,
    pub length:  u32,
}
#[repr(C)]
pub struct UnicastAddress {
    pub length:             u32,
    pub flags:              u32,
    pub next:               *mut UnicastAddress,
    pub addr:               SocketAddress,
    pub prefix_origin:      u32,
    pub suffix_origin:      u32,
    pub dad_state:          u32,
    pub valid_lifetime:     u32,
    pub preferred_lifetime: u32,
    pub lease_lifetime:     u32,
    pub link_prefix_length: u8,
}
pub struct UnicastIter<'a> {
    base: &'a Adapter,
    cur:  Option<&'a UnicastAddress>,
}
pub struct AdapterIter<'a> {
    base: &'a Adapter,
    cur:  Option<&'a Adapter>,
}
#[repr(C)]
pub struct MulticastAddress {
    pub length: u32,
    pub flags:  u32,
    pub next:   *mut MulticastAddress,
    pub addr:   SocketAddress,
}
#[repr(transparent)]
pub struct OwnedSocket(pub usize);
pub struct AddressIter<'a>(UnicastIter<'a>);

impl Adapter {
    #[inline]
    pub fn dns(&self) -> DnsIter<'_> {
        DnsIter { base: self, cur: None }
    }
    #[inline]
    pub fn iter(&self) -> AddressIter<'_> {
        AddressIter(self.unicast())
    }
    #[inline]
    pub fn unicast(&self) -> UnicastIter<'_> {
        UnicastIter { base: self, cur: None }
    }

    #[inline]
    pub(crate) fn enumerate(&self) -> AdapterIter<'_> {
        AdapterIter { base: self, cur: None }
    }
}
impl SockAddr {
    #[inline]
    fn into_outer(&self) -> Option<SocketAddr> {
        match self.family {
            /* AF_INET */ 0x2 => Some(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(self.addr4.to_le_bytes()),
                u16::from_be(self.port),
            ))),
            /* AF_INET6 */ 0x17 => Some(SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(self.addr6),
                u16::from_be(self.port),
                self.addr4,
                self.scope,
            ))),
            _ => None,
        }
    }
}
impl AddressInfo {
    #[inline]
    pub fn new(flags: u32, family: u32, socktype: u32) -> AddressInfo {
        AddressInfo {
            flags,
            family,
            socktype,
            addr: ptr::null_mut(),
            next: ptr::null_mut(),
            protocol: 0,
            canon_name: CharPtr::null(),
            address_size: 0,
        }
    }
}
impl UnicastAddress {
    #[inline]
    pub fn address(&self) -> Option<SocketAddr> {
        unsafe { self.addr.address.as_ref()? }.into_outer()
    }
}
impl MulticastAddress {
    #[inline]
    pub fn address(&self) -> Option<SocketAddr> {
        unsafe { self.addr.address.as_ref()? }.into_outer()
    }
}

impl Eq for OwnedSocket {}
impl Drop for OwnedSocket {
    #[inline]
    fn drop(&mut self) {
        let _ = winapi::WSACloseSocket(self); // IGNORE ERROR
    }
}
impl PartialEq for OwnedSocket {
    #[inline]
    fn eq(&self, other: &OwnedSocket) -> bool {
        self.0 == other.0
    }
}

impl Default for SockAddr {
    #[inline]
    fn default() -> SockAddr {
        SockAddr {
            port:   0,
            addr4:  0,
            addr6:  [0; 16],
            scope:  0,
            family: 0,
        }
    }
}
impl Default for AddressInfo {
    #[inline]
    fn default() -> AddressInfo {
        AddressInfo {
            addr:         ptr::null_mut(),
            next:         ptr::null_mut(),
            flags:        0,
            family:       0,
            socktype:     0,
            protocol:     0,
            address_size: 0,
            canon_name:   CharPtr::null(),
        }
    }
}

impl TryFrom<SockAddr> for SocketAddr {
    type Error = Win32Error;

    #[inline]
    fn try_from(v: SockAddr) -> Win32Result<SocketAddr> {
        v.into_outer().ok_or(Win32Error::InvalidAddress)
    }
}
impl From<&AddressInfo> for Option<SocketAddr> {
    #[inline]
    fn from(v: &AddressInfo) -> Option<SocketAddr> {
        unsafe { mem::transmute::<*mut SocketAddress, &SockAddr>(v.addr) }.into_outer()
    }
}

impl<'a> IntoIterator for &'a Adapter {
    type Item = SocketAddr;
    type IntoIter = AddressIter<'a>;

    #[inline]
    fn into_iter(self) -> AddressIter<'a> {
        self.iter()
    }
}

impl<'a> Iterator for DnsIter<'a> {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        match self.cur {
            Some(v) => self.cur = unsafe { v.next.as_ref() },
            None => self.cur = unsafe { self.base.first_dns.as_ref() },
        }
        self.cur.and_then(|v| unsafe { v.addr.address.as_ref()? }.into_outer())
    }
}
impl<'a> Iterator for UnicastIter<'a> {
    type Item = &'a UnicastAddress;

    #[inline]
    fn next(&mut self) -> Option<&'a UnicastAddress> {
        match self.cur {
            Some(v) => self.cur = unsafe { v.next.as_ref() },
            None => self.cur = unsafe { self.base.first_unicast.as_ref() },
        }
        self.cur
    }
}
impl<'a> Iterator for AddressIter<'a> {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        loop {
            if let Some(a) = self.0.next()?.address() {
                return Some(a);
            }
        }
    }
}
impl<'a> Iterator for AdapterIter<'a> {
    type Item = &'a Adapter;

    #[inline]
    fn next(&mut self) -> Option<&'a Adapter> {
        match self.cur {
            Some(v) => self.cur = unsafe { v.next.as_ref() },
            None => self.cur = Some(self.base),
        }
        self.cur
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::OwnedSocket;
    use crate::util::stx::prelude::*;

    impl Debug for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "OwnedSocket: 0x{:X}", self.0)
        }
    }
    impl LowerHex for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
