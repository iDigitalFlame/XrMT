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

use alloc::vec::IntoIter;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::ops::{Deref, Index};

use crate::data::{Readable, Reader, Writable, Writer};
use crate::util::stx::io;
use crate::util::stx::prelude::*;

pub struct Address {
    hi:  u64,
    low: u64,
}
pub struct Interface {
    pub name:    String,
    pub address: Vec<Address>,
    pub mac:     HardwareAddress,
}
pub struct HardwareAddress(u64);
pub struct Network(Vec<Interface>);

impl Address {
    #[inline]
    pub const fn new() -> Address {
        Address { hi: 0, low: 0 }
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
        Network(Vec::new())
    }

    #[inline]
    pub fn local() -> io::Result<Network> {
        let mut n = Network::new();
        n.refresh()?;
        Ok(n)
    }

    #[inline]
    pub fn refresh(&mut self) -> io::Result<()> {
        inner::refresh(self)
    }
}
impl Interface {
    #[inline]
    pub fn new() -> Interface {
        Interface {
            mac:     HardwareAddress(0),
            name:    String::new(),
            address: Vec::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.address.len()
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

impl Clone for Network {
    #[inline]
    fn clone(&self) -> Network {
        Network(self.0.clone())
    }
}
impl Deref for Network {
    type Target = [Interface];

    #[inline]
    fn deref(&self) -> &[Interface] {
        &self.0
    }
}
impl Default for Network {
    #[inline]
    fn default() -> Network {
        Network::new()
    }
}
impl Writable for Network {
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
impl Readable for Network {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        let n = r.read_u8()?;
        self.0.clear();
        self.0.reserve_exact(n as usize);
        for _ in 0..n {
            let mut a = Interface::new();
            a.read_stream(r)?;
            self.0.push(a);
        }
        Ok(())
    }
}
impl Index<usize> for Network {
    type Output = Interface;

    #[inline]
    fn index(&self, index: usize) -> &Interface {
        &self.0[index]
    }
}
impl IntoIterator for Network {
    type Item = Interface;
    type IntoIter = IntoIter<Interface>;

    #[inline]
    fn into_iter(self) -> IntoIter<Interface> {
        self.0.into_iter()
    }
}

impl Clone for Interface {
    #[inline]
    fn clone(&self) -> Interface {
        Interface {
            mac:     self.mac,
            name:    self.name.clone(),
            address: self.address.clone(),
        }
    }
}
impl Deref for Interface {
    type Target = [Address];

    #[inline]
    fn deref(&self) -> &[Address] {
        &self.address
    }
}
impl Default for Interface {
    #[inline]
    fn default() -> Interface {
        Interface::new()
    }
}
impl Writable for Interface {
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
impl Readable for Interface {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_str(&mut self.name)?;
        self.mac.read_stream(r)?;
        let n = r.read_u8()?;
        self.address.clear();
        self.address.reserve_exact(n as usize);
        for _ in 0..n {
            let mut a = Address::new();
            a.read_stream(r)?;
            self.address.push(a);
        }
        Ok(())
    }
}
impl Index<usize> for Interface {
    type Output = Address;

    #[inline]
    fn index(&self, index: usize) -> &Address {
        &self.address[index]
    }
}
impl IntoIterator for Interface {
    type Item = Address;
    type IntoIter = IntoIter<Address>;

    #[inline]
    fn into_iter(self) -> IntoIter<Address> {
        self.address.into_iter()
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

#[cfg(unix)]
mod inner {
    extern crate interfaces;

    use interfaces::{InterfaceFlags, InterfacesError, Kind};

    use super::{Address, HardwareAddress, Interface, Network};
    use crate::util::stx::io::{self, Error, ErrorKind};
    use crate::util::stx::prelude::*;

    impl From<&[u8]> for HardwareAddress {
        #[inline]
        fn from(v: &[u8]) -> HardwareAddress {
            if v.len() < 6 {
                HardwareAddress::default()
            } else {
                HardwareAddress((v[0] as u64) << 40 | (v[1] as u64) << 32 | (v[2] as u64) << 24 | (v[3] as u64) << 16 | (v[4] as u64) << 8 | v[5] as u64)
            }
        }
    }

    pub fn refresh(v: &mut Network) -> io::Result<()> {
        let a = interfaces::Interface::get_all().map_err(|e| match e {
            InterfacesError::Errno(r) => Error::from_raw_os_error((r as i32).into()),
            _ => Error::new(ErrorKind::Unsupported, e),
        })?;
        v.0.clear();
        v.0.reserve_exact(a.len());
        for i in a {
            if !i.flags.contains(InterfaceFlags::IFF_UP) || i.flags.contains(InterfaceFlags::IFF_LOOPBACK) {
                continue;
            }
            let mut e = Interface {
                mac:     HardwareAddress::from(
                    i.hardware_addr()
                        .unwrap_or_else(|_| interfaces::HardwareAddr::zero())
                        .as_bytes(),
                ),
                name:    i.name.clone(),
                address: Vec::new(),
            };
            e.address.reserve(i.addresses.len());
            for v in &i.addresses {
                match v.kind {
                    Kind::Link | Kind::Packet | Kind::Unknown(_) => continue,
                    _ => (),
                }
                if let Some(x) = v.addr {
                    let q = Address::from(x);
                    if !q.is_zero() && q.is_global_unicast() {
                        e.address.push(q);
                    }
                }
            }
            if e.address.is_empty() {
                continue;
            }
            v.0.push(e);
        }
        Ok(())
    }
}
#[cfg(windows)]
mod inner {
    use super::{Address, HardwareAddress, Interface, Network};
    use crate::data::blob::Blob;
    use crate::device::winapi;
    use crate::util::stx::io::{self, Error};
    use crate::util::stx::prelude::*;

    impl From<[u8; 16]> for Address {
        #[inline]
        fn from(v: [u8; 16]) -> Address {
            let mut a = Address::new();
            for i in 4..16 {
                if v[i] > 0 {
                    a.from_ipv6(v);
                    return a;
                }
            }
            a.low = 0xFFFF00000000 | (v[0] as u64) << 24 | (v[1] as u64) << 16 | (v[2] as u64) << 8 | v[3] as u64;
            a
        }
    }
    impl From<[u8; 8]> for HardwareAddress {
        #[inline]
        fn from(v: [u8; 8]) -> HardwareAddress {
            HardwareAddress((v[0] as u64) << 40 | (v[1] as u64) << 32 | (v[2] as u64) << 24 | (v[3] as u64) << 16 | (v[4] as u64) << 8 | v[5] as u64)
        }
    }

    pub fn refresh(v: &mut Network) -> io::Result<()> {
        v.0.clear();
        let mut buf = Blob::new();
        let a = winapi::GetAdaptersAddresses(0, 0x10, &mut buf).map_err(Error::from)?;
        if a.len() == 0 {
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
            let e = Interface {
                mac:     HardwareAddress::from(i.physical_address),
                name:    i.friendly_name.into_string(),
                address: i
                    .iter()
                    .map(|a| Address::from(a))
                    .filter(|a| a.is_global_unicast())
                    .collect(),
            };
            if e.address.len() == 0 {
                continue;
            }
            v.0.push(e);
            if v.0.len() >= 255 {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, Write};

    use super::{Address, HardwareAddress, Interface, Network};
    use crate::util::stx::prelude::*;

    impl Address {
        #[inline]
        fn grab(&self, i: u8) -> u16 {
            (if (i / 4) % 2 == 1 { self.low } else { self.hi } >> ((3 - (i % 4)) * 16)) as u16
        }
    }

    impl Debug for Network {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Network").field(&self.0).finish()
        }
    }
    impl Debug for Interface {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Interface")
                .field("name", &self.name)
                .field("address", &self.address)
                .field("mac", &self.mac)
                .finish()
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
            let (mut s, mut e) = (255, 255);
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
            let mut i = 0;
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
}
