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
use alloc::vec::IntoIter;
use core::alloc::Allocator;
use core::cmp::Ordering;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::ops::{Deref, Index};

use crate::data::str::Fiber;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::io;
use crate::prelude::*;

pub struct Address {
    hi:  u64,
    low: u64,
}
pub struct HardwareAddress(u64);
pub struct Interface<A: Allocator = Global> {
    pub name:    Fiber<A>,
    pub address: Vec<Address, A>,
    pub mac:     HardwareAddress,
}
pub struct Network<A: Allocator = Global>(Vec<Interface<A>, A>);

impl Address {
    #[inline]
    pub const fn new() -> Address {
        Address { hi: 0u64, low: 0u64 }
    }

    #[inline]
    pub fn read(r: &mut impl Reader) -> io::Result<Address> {
        let mut a = Address::new();
        a.read_stream(r)?;
        Ok(a)
    }

    #[inline]
    pub fn len(&self) -> usize {
        if self.is_ipv4() {
            32
        } else {
            128
        }
    }
    #[inline]
    pub fn is_ipv4(&self) -> bool {
        self.hi == 0 && (self.low >> 32) as u32 == 0xFFFF
    }
    #[inline]
    pub fn is_ipv6(&self) -> bool {
        self.low > 0 && (self.low >> 32) as u32 != 0xFFFF
    }
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.hi == 0 && self.low == 0
    }
    #[inline]
    pub fn is_loopback(&self) -> bool {
        (self.is_ipv4() && (self.low >> 24) as u8 == 0x7F) || (self.is_ipv6() && self.hi == 0 && self.low == 1)
    }
    #[inline]
    pub fn is_multicast(&self) -> bool {
        (self.is_ipv4() && (self.low >> 24) as u8 == 0xFE) || (self.is_ipv6() && (self.hi >> 56) as u8 == 0xFF)
    }
    #[inline]
    pub fn is_broadcast(&self) -> bool {
        self.is_ipv4() && self.low == 0xFFFFFFFFFFFF
    }
    #[inline]
    pub fn is_global_unicast(&self) -> bool {
        !self.is_zero() && !self.is_broadcast() && !self.is_loopback() && !self.is_multicast() && !self.is_link_local_unicast()
    }
    #[inline]
    pub fn is_link_local_unicast(&self) -> bool {
        (self.is_ipv4() && (self.low >> 16) as u32 == 0xFFFFA9FE) || (self.is_ipv6() && (self.hi >> 48) as u16 == 0xFE80)
    }
    #[inline]
    pub fn is_link_local_multicast(&self) -> bool {
        (self.is_ipv4() && self.low >> 8 == 0xFFFFE00000) || (self.is_ipv6() && (self.hi >> 48) as u16 == 0xFF02)
    }

    #[inline]
    fn from_ipv4(&mut self, d: [u8; 4]) {
        self.hi = 0;
        self.low = 0xFFFF00000000 | (d[0] as u64) << 24 | (d[1] as u64) << 16 | (d[2] as u64) << 8 | d[3] as u64;
    }
    #[inline]
    fn from_ipv6(&mut self, d: [u8; 16]) {
        self.hi = d[7] as u64 | (d[6] as u64) << 8 | (d[5] as u64) << 16 | (d[4] as u64) << 24 | (d[3] as u64) << 32 | (d[2] as u64) << 40 | (d[1] as u64) << 48 | (d[0] as u64) << 56;
        self.low = d[15] as u64 | (d[14] as u64) << 8 | (d[13] as u64) << 16 | (d[12] as u64) << 24 | (d[11] as u64) << 32 | (d[10] as u64) << 40 | (d[9] as u64) << 48 | (d[8] as u64) << 56;
    }
}
impl Network {
    #[inline]
    pub const fn new() -> Network {
        Network::new_in(Global)
    }

    #[inline]
    pub fn local() -> io::Result<Network> {
        Network::local_in(Global)
    }
}
impl Interface {
    #[inline]
    pub fn new() -> Interface {
        Interface::new_in(Global)
    }
}
impl<A: Allocator> Interface<A> {
    #[inline]
    pub fn len(&self) -> usize {
        self.address.len()
    }
}
impl<A: Allocator + Clone> Network<A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Network<A> {
        Network(Vec::new_in(alloc))
    }

    #[inline]
    pub fn local_in(alloc: A) -> io::Result<Network<A>> {
        let mut n = Network::new_in(alloc);
        n.refresh()?;
        Ok(n)
    }

    #[inline]
    pub fn refresh(&mut self) -> io::Result<()> {
        inner::refresh(self)?;
        self.0.shrink_to_fit();
        Ok(())
    }
}
impl<A: Allocator + Clone> Interface<A> {
    #[inline]
    pub fn new_in(alloc: A) -> Interface<A> {
        Interface {
            mac:     HardwareAddress(0u64),
            name:    Fiber::new_in(alloc.clone()),
            address: Vec::new_in(alloc),
        }
    }
}

impl Eq for Address {}
impl Ord for Address {
    #[inline]
    fn cmp(&self, other: &Address) -> Ordering {
        if self.eq(other) {
            Ordering::Equal
        } else if self.is_ipv4() && self.low > other.low {
            Ordering::Greater
        } else if self.is_ipv6() && self.hi > other.hi && self.low > other.low {
            Ordering::Greater
        } else if self.is_ipv4() && other.is_ipv6() {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}
impl Copy for Address {}
impl Clone for Address {
    #[inline]
    fn clone(&self) -> Address {
        Address { hi: self.hi, low: self.low }
    }
}
impl Default for Address {
    #[inline]
    fn default() -> Address {
        Address::new()
    }
}
impl Writable for Address {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u64(self.hi)?;
        w.write_u64(self.low)
    }
}
impl Readable for Address {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u64(&mut self.hi)?;
        r.read_into_u64(&mut self.low)
    }
}
impl PartialEq for Address {
    #[inline]
    fn eq(&self, other: &Address) -> bool {
        self.hi == other.hi && self.low == other.low
    }
}
impl PartialOrd for Address {
    #[inline]
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl From<IpAddr> for Address {
    #[inline]
    fn from(v: IpAddr) -> Address {
        let mut x = Address::new();
        match v {
            IpAddr::V4(a) => x.from_ipv4(a.octets()),
            IpAddr::V6(a) => x.from_ipv6(a.octets()),
        }
        x
    }
}
impl From<[u8; 16]> for Address {
    #[inline]
    fn from(v: [u8; 16]) -> Address {
        let mut a = Address::new();
        for i in 4..16 {
            if v[i] > 0 {
                // BUG(dij): We can make a mistake here as we can mistake '1::'
                //           as an IPv4 address. It won't happen much as this is
                //           only used for cases where the chance of IPv6 is low.
                a.from_ipv6(v);
                return a;
            }
        }
        a.low = 0xFFFF00000000 | (v[0] as u64) << 24 | (v[1] as u64) << 16 | (v[2] as u64) << 8 | v[3] as u64;
        a
    }
}
impl From<SocketAddr> for Address {
    #[inline]
    fn from(v: SocketAddr) -> Address {
        let mut x = Address::new();
        match v.ip() {
            IpAddr::V4(a) => x.from_ipv4(a.octets()),
            IpAddr::V6(a) => x.from_ipv6(a.octets()),
        }
        x
    }
}

impl From<Address> for IpAddr {
    #[inline]
    fn from(v: Address) -> IpAddr {
        if v.is_ipv4() {
            IpAddr::V4(Ipv4Addr::from(v.low as u32))
        } else {
            IpAddr::V6(Ipv6Addr::from((v.hi as u128) << 64 | v.low as u128))
        }
    }
}

impl Default for Network {
    #[inline]
    fn default() -> Network {
        Network::new()
    }
}
impl<A: Allocator> Eq for Network<A> {}
impl<A: Allocator> Ord for Network<A> {
    #[inline]
    fn cmp(&self, other: &Network<A>) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl<A: Allocator> Deref for Network<A> {
    type Target = [Interface<A>];

    #[inline]
    fn deref(&self) -> &[Interface<A>] {
        &self.0
    }
}
impl<A: Allocator> Writable for Network<A> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        let n = self.0.len();
        w.write_u8(n as u8)?;
        for (i, v) in self.0.iter().enumerate() {
            if i > n || i > u8::MAX as usize {
                break;
            }
            v.write_stream(w)?;
        }
        Ok(())
    }
}
impl<A: Allocator> PartialEq for Network<A> {
    #[inline]
    fn eq(&self, other: &Network<A>) -> bool {
        self.0.eq(&other.0)
    }
}
impl<A: Allocator> PartialOrd for Network<A> {
    #[inline]
    fn partial_cmp(&self, other: &Network<A>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<A: Allocator> Index<usize> for Network<A> {
    type Output = Interface<A>;

    #[inline]
    fn index(&self, index: usize) -> &Interface<A> {
        &self.0[index]
    }
}
impl<A: Allocator> IntoIterator for Network<A> {
    type Item = Interface<A>;
    type IntoIter = IntoIter<Interface<A>, A>;

    #[inline]
    fn into_iter(self) -> IntoIter<Interface<A>, A> {
        self.0.into_iter()
    }
}
impl<A: Allocator + Clone> Clone for Network<A> {
    #[inline]
    fn clone(&self) -> Network<A> {
        Network(self.0.clone())
    }
}
impl<A: Allocator + Clone> Readable for Network<A> {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        let n = r.read_u8()?;
        self.0.clear();
        self.0.reserve_exact(n as usize);
        for _ in 0..n {
            let mut a = Interface::new_in(self.0.allocator().clone());
            a.read_stream(r)?;
            self.0.push(a);
        }
        Ok(())
    }
}

impl Default for Interface {
    #[inline]
    fn default() -> Interface {
        Interface::new()
    }
}
impl<A: Allocator> Eq for Interface<A> {}
impl<A: Allocator> Ord for Interface<A> {
    #[inline]
    fn cmp(&self, other: &Interface<A>) -> Ordering {
        self.name.cmp(&other.name)
    }
}
impl<A: Allocator> Deref for Interface<A> {
    type Target = [Address];

    #[inline]
    fn deref(&self) -> &[Address] {
        &self.address
    }
}
impl<A: Allocator> Writable for Interface<A> {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_str(&self.name)?;
        self.mac.write_stream(w)?;
        let n = self.address.len();
        w.write_u8(n as u8)?;
        for (i, v) in self.address.iter().enumerate() {
            if i > n || i > u8::MAX as usize {
                break;
            }
            v.write_stream(w)?;
        }
        Ok(())
    }
}
impl<A: Allocator> Readable for Interface<A> {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_fiber(&mut self.name)?;
        self.mac.read_stream(r)?;
        let n = r.read_u8()?;
        self.address.clear();
        self.address.reserve_exact(n as usize);
        for _ in 0..n {
            self.address.push(Address::read(r)?);
        }
        Ok(())
    }
}
impl<A: Allocator> PartialEq for Interface<A> {
    #[inline]
    fn eq(&self, other: &Interface<A>) -> bool {
        self.name.eq(&other.name) && self.address.eq(&other.address) && self.mac.0 == other.mac.0
    }
}
impl<A: Allocator> PartialOrd for Interface<A> {
    #[inline]
    fn partial_cmp(&self, other: &Interface<A>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<A: Allocator> Index<usize> for Interface<A> {
    type Output = Address;

    #[inline]
    fn index(&self, index: usize) -> &Address {
        &self.address[index]
    }
}
impl<A: Allocator> IntoIterator for Interface<A> {
    type Item = Address;
    type IntoIter = IntoIter<Address, A>;

    #[inline]
    fn into_iter(self) -> IntoIter<Address, A> {
        self.address.into_iter()
    }
}
impl<A: Allocator + Clone> Clone for Interface<A> {
    #[inline]
    fn clone(&self) -> Interface<A> {
        Interface {
            mac:     self.mac,
            name:    self.name.clone(),
            address: self.address.clone(),
        }
    }
}

impl Eq for HardwareAddress {}
impl Ord for HardwareAddress {
    #[inline]
    fn cmp(&self, other: &HardwareAddress) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Copy for HardwareAddress {}
impl Clone for HardwareAddress {
    #[inline]
    fn clone(&self) -> HardwareAddress {
        HardwareAddress(self.0)
    }
}
impl Default for HardwareAddress {
    #[inline]
    fn default() -> HardwareAddress {
        HardwareAddress(0)
    }
}
impl Writable for HardwareAddress {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u64(self.0)
    }
}
impl Readable for HardwareAddress {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u64(&mut self.0)
    }
}
impl PartialEq for HardwareAddress {
    #[inline]
    fn eq(&self, other: &HardwareAddress) -> bool {
        self.0 == other.0
    }
}
impl PartialOrd for HardwareAddress {
    #[inline]
    fn partial_cmp(&self, other: &HardwareAddress) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

#[cfg(target_family = "windows")]
mod inner {
    use core::alloc::Allocator;

    use crate::data::blob::Blob;
    use crate::device::network::{Address, HardwareAddress, Interface, Network};
    use crate::device::winapi;
    use crate::io;
    use crate::prelude::*;

    impl From<[u8; 8]> for HardwareAddress {
        #[inline]
        fn from(v: [u8; 8]) -> HardwareAddress {
            HardwareAddress((v[0] as u64) << 40 | (v[1] as u64) << 32 | (v[2] as u64) << 24 | (v[3] as u64) << 16 | (v[4] as u64) << 8 | v[5] as u64)
        }
    }

    pub fn refresh<A: Allocator + Clone>(v: &mut Network<A>) -> io::Result<()> {
        v.0.clear();
        let x = v.0.allocator().clone();
        let mut buf = Blob::new();
        let a = winapi::GetAdaptersAddresses(0, 0x10, &mut buf)?;
        if a.is_empty() {
            return Ok(());
        }
        v.0.reserve_exact(a.len());
        for i in a {
            if i.operating_status != 1 {
                continue;
            }
            match i.if_type {
                0x17 | 0x83 | 0x1B | 0x25 => continue,
                _ => (),
            }
            let mut e = Interface {
                mac:     HardwareAddress::from(i.physical_address),
                name:    i.friendly_name.into_fiber(x.clone()),
                address: Vec::new_in(x.clone()),
            };
            i.iter()
                .map(Address::from)
                .filter(|a| a.is_global_unicast())
                .collect_into(&mut e.address);
            if e.address.is_empty() {
                continue;
            }
            e.address.shrink_to_fit();
            v.0.push(e);
            if v.0.len() >= 255 {
                break;
            }
        }
        Ok(())
    }
}
#[cfg(any(target_vendor = "fortanix", target_os = "redox"))]
mod inner {
    use core::alloc::Allocator;

    use crate::device::Network;
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[inline]
    pub fn refresh<A: Allocator + Clone>(_v: &mut Network<A>) -> io::Result<()> {
        Err(ErrorKind::Unsupported.into())
    }
}
#[cfg(all(
    not(target_family = "windows"),
    not(target_vendor = "fortanix"),
    not(target_os = "redox")
))]
mod inner {
    extern crate libc;

    use alloc::collections::BTreeMap;
    use core::alloc::Allocator;
    use core::ffi::CStr;
    use core::marker::PhantomData;
    use core::mem::{transmute, zeroed};
    use core::ptr;
    use core::slice::from_raw_parts;

    use libc::{freeifaddrs, getifaddrs, ifaddrs, sockaddr, sockaddr_in, sockaddr_in6};

    use crate::device::network::{Address, HardwareAddress, Interface, Network};
    use crate::io::{self, Error};
    use crate::prelude::*;
    use crate::{ok_or_continue, some_or_return};

    struct InterfaceIter<'a> {
        original: *mut ifaddrs,
        current:  *mut ifaddrs,
        phantom:  PhantomData<&'a mut ifaddrs>,
    }

    impl Interface {
        #[inline]
        fn hash(v: &str) -> u32 {
            let mut h = 0x811C9DC5u32;
            for i in v.as_bytes() {
                h = h.wrapping_mul(0x1000193);
                h ^= *i as u32;
            }
            h
        }
    }
    impl InterfaceIter<'_> {
        #[inline]
        fn new<'a>() -> io::Result<InterfaceIter<'a>> {
            let mut a = unsafe { zeroed() };
            if unsafe { getifaddrs(&mut a) } == 0 {
                Ok(InterfaceIter {
                    original: a,
                    current:  a,
                    phantom:  PhantomData,
                })
            } else {
                Err(Error::last_os_error())
            }
        }
    }
    impl<A: Allocator + Clone> Interface<A> {
        #[inline]
        fn init(n: &str, alloc: A) -> Interface<A> {
            Interface {
                mac:     HardwareAddress(0),
                name:    n.into_alloc(alloc.clone()),
                address: Vec::new_in(alloc),
            }
        }

        #[inline]
        fn add(&mut self, v: *mut sockaddr) {
            let i = some_or_return!(unsafe { v.as_ref() }, ());
            /* AF_PACKET | AF_LINK */
            if i.sa_family == 0x11 || i.sa_family == 0x12 {
                // BUG(dij): Solaris does not give this! How can we get the
                //           mac address?
                self.mac = i.into();
                return;
            }
            if let Ok(a) = Address::try_from(i) {
                self.address.push(a)
            }
        }
    }

    impl Drop for InterfaceIter<'_> {
        #[inline]
        fn drop(&mut self) {
            self.current = ptr::null_mut();
            unsafe { freeifaddrs(self.original) }
        }
    }
    impl TryFrom<&sockaddr> for Address {
        type Error = ();

        #[inline]
        fn try_from(v: &sockaddr) -> Result<Address, ()> {
            /* AF_INET */
            if v.sa_family == 0x2 {
                let x = unsafe { &*transmute::<&sockaddr, *const sockaddr_in>(v) };
                let mut a = Address::new();
                a.from_ipv4([
                    ((x.sin_addr.s_addr & 0xFF) >> 0) as u8,
                    ((x.sin_addr.s_addr & 0xFF00) >> 8) as u8,
                    ((x.sin_addr.s_addr & 0xFF0000) >> 16) as u8,
                    ((x.sin_addr.s_addr & 0xFF000000) >> 24) as u8,
                ]);
                if a.is_global_unicast() {
                    return Ok(a);
                }
            }
            /*
                AF_INET6
                - 0xA  in Linux-like
                - 0x1A in Solaris
                - 0x1C in BSD-like
                - 0x1E in MacOS (why? not documented)
                - 0x18 in NetBSD
            */
            match v.sa_family {
                0xA | 0x1C | 0x1E | 0x1A | 0x18 => (),
                _ => return Err(()),
            }
            let x = unsafe { &*transmute::<&sockaddr, *const sockaddr_in6>(v) };
            let mut a = Address::new();
            a.from_ipv6([
                x.sin6_addr.s6_addr[0],
                x.sin6_addr.s6_addr[1],
                x.sin6_addr.s6_addr[2],
                x.sin6_addr.s6_addr[3],
                x.sin6_addr.s6_addr[4],
                x.sin6_addr.s6_addr[5],
                x.sin6_addr.s6_addr[6],
                x.sin6_addr.s6_addr[7],
                x.sin6_addr.s6_addr[8],
                x.sin6_addr.s6_addr[9],
                x.sin6_addr.s6_addr[10],
                x.sin6_addr.s6_addr[11],
                x.sin6_addr.s6_addr[12],
                x.sin6_addr.s6_addr[13],
                x.sin6_addr.s6_addr[14],
                x.sin6_addr.s6_addr[15],
            ]);
            if a.is_global_unicast() {
                Ok(a)
            } else {
                Err(())
            }
        }
    }
    impl<'a> Iterator for InterfaceIter<'a> {
        type Item = &'a ifaddrs;

        #[inline]
        fn next(&mut self) -> Option<&'a ifaddrs> {
            let i = match unsafe { self.current.as_ref() } {
                Some(v) => v,
                None => return None,
            };
            self.current = i.ifa_next;
            Some(i)
        }
    }
    impl From<&sockaddr> for HardwareAddress {
        #[cfg(any(
            target_os = "ios",
            target_os = "redox",
            target_os = "macos",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "fuchsia",
            target_os = "freebsd",
            target_os = "dragonfly"
        ))]
        #[inline]
        fn from(v: &sockaddr) -> HardwareAddress {
            // BSD has a diff way of handeling MAC addresses.
            // saddr_dl->sdl_nlen
            let n = unsafe { *((v as *const sockaddr) as *const u8).add(5) } as usize;
            // saddr_dl->sdl_data (always 8) + saddr_dl->sdl_nlen (^ above)
            let b = unsafe { from_raw_parts(((v as *const sockaddr) as *const u8).add(8 + n), 6) };
            HardwareAddress((b[0] as u64) << 40 | (b[1] as u64) << 32 | (b[2] as u64) << 24 | (b[3] as u64) << 16 | (b[4] as u64) << 8 | b[5] as u64)
        }
        #[cfg(all(
            not(target_os = "ios"),
            not(target_os = "redox"), // NOTE(dij): Rust-based OS, BSD-Like.
            not(target_os = "macos"),
            not(target_os = "netbsd"),
            not(target_os = "openbsd"),
            not(target_os = "fuchsia"),
            not(target_os = "freebsd"),
            not(target_os = "dragonfly")
        ))]
        #[inline]
        fn from(v: &sockaddr) -> HardwareAddress {
            let b = unsafe { from_raw_parts((v.sa_data.as_ptr() as *const u8).add(10), 6) };
            HardwareAddress((b[0] as u64) << 40 | (b[1] as u64) << 32 | (b[2] as u64) << 24 | (b[3] as u64) << 16 | (b[4] as u64) << 8 | b[5] as u64)
        }
    }

    pub fn refresh<A: Allocator + Clone>(v: &mut Network<A>) -> io::Result<()> {
        v.0.clear();
        let a = v.0.allocator().clone();
        let mut e = BTreeMap::new();
        for i in InterfaceIter::new()? {
            if i.ifa_flags & 0x8 != 0 || i.ifa_flags & 0x1 == 0 || i.ifa_addr.is_null() {
                continue;
            }
            let n = ok_or_continue!(unsafe { CStr::from_ptr(i.ifa_name).to_str() });
            let a = e
                .entry(Interface::hash(n))
                .or_insert_with(|| Interface::init(n, a.clone()));
            if a.address.len() < 255 {
                a.add(i.ifa_addr);
            }
        }
        v.0.reserve_exact(e.len());
        for mut i in e.into_values() {
            if i.address.is_empty() {
                continue;
            }
            i.address.shrink_to_fit();
            v.0.push(i)
        }
        Ok(())
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::alloc::Allocator;
    use core::fmt::{self, Debug, Display, Formatter, Write};

    use crate::device::network::{Address, HardwareAddress, Interface, Network};
    use crate::prelude::*;

    impl Address {
        #[inline]
        fn grab(&self, i: u8) -> u16 {
            (if (i / 4) % 2 == 1 { self.low } else { self.hi } >> ((3 - (i % 4)) * 16)) as u16
        }
    }

    impl Debug for Address {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Address {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.is_zero() {
                return if self.is_ipv4() {
                    f.write_str("0.0.0.0")
                } else {
                    f.write_str("::")
                };
            }
            if self.is_ipv4() {
                return write!(
                    f,
                    "{}.{}.{}.{}",
                    ((self.low >> 24) as u8),
                    ((self.low >> 16) as u8),
                    ((self.low >> 8) as u8),
                    (self.low as u8)
                );
            }
            let (mut s, mut e) = (255u8, 255u8);
            for i in 0..8 {
                let mut j = i;
                while j < 8 && self.grab(j) == 0 {
                    j += 1;
                }
                let l = j - i;
                if l >= 2 && l > e - s {
                    (s, e) = (i, j);
                }
            }
            let mut i = 0u8;
            while i < 8 {
                if i == s {
                    f.write_str("::")?;
                    i = e;
                    if i >= 8 {
                        break;
                    }
                } else if i > 0 {
                    f.write_char(':')?;
                }
                write!(f, "{:X}", self.grab(i))?;
                i += 1;
            }
            Ok(())
        }
    }

    impl Debug for HardwareAddress {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for HardwareAddress {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                (self.0 >> 40) as u8,
                (self.0 >> 32) as u8,
                (self.0 >> 24) as u8,
                (self.0 >> 16) as u8,
                (self.0 >> 8) as u8,
                self.0 as u8,
            )
        }
    }

    impl<A: Allocator> Debug for Network<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl<A: Allocator> Display for Network<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("Network{")?;
            for (i, v) in self.0.iter().enumerate() {
                if i > 0 {
                    f.write_char(' ')?;
                }
                Debug::fmt(v, f)?;
            }
            f.write_char('}')
        }
    }

    impl<A: Allocator> Debug for Interface<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl<A: Allocator> Display for Interface<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&self.name)?;
            f.write_char('(')?;
            Debug::fmt(&self.mac, f)?;
            f.write_str("): [")?;
            for (i, v) in self.address.iter().enumerate() {
                if i > 0 {
                    f.write_char(' ')?;
                }
                Debug::fmt(v, f)?;
            }
            f.write_char(']')
        }
    }
}
