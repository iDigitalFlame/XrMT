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
#![cfg(target_family = "windows")]

extern crate core;

use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Into, TryFrom};
use core::default::Default;
use core::iter::{FusedIterator, IntoIterator, Iterator};
use core::marker::PhantomData;
use core::mem::{replace, transmute};
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::num::NonZeroUsize;
use core::ops::{Deref, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::drop_in_place;
use core::time::Duration;

use crate::functions::{len_to_u32, WSACloseSocket, WSAFreeAddrInfo};
use crate::structs::{CharPtr, Handle, WCharPtr};
use crate::{Win32Error, Win32Result};

#[repr(C)]
pub struct FileSet {
    pub count: u32,
    pub array: [usize; 64],
}
#[repr(C)]
pub struct SockAddr {
    pub family: u16,
    pub port:   u16,
    pub addr4:  u32,      // IPv4 Address or IPv6 FlowInfo
    pub addr6:  [u8; 16], // IPv6 Address
    pub scope:  u32,
}
#[repr(C)]
pub struct SockInfo {
    pad1:               [u32; 19],
    pub address_family: u32,
    pad2:               [u32; 2],
    pub socket_type:    u32,
    pub protocol:       u32,
    pad3:               [u32; 5],
    pad4:               [u16; 256],
}
#[repr(C)]
pub struct Adapter<'a> {
    pub length:                  u32,
    pub index:                   u32,
    pub next:                    Option<&'a Adapter<'a>>,
    pub name:                    CharPtr<'a>,
    pub first_unicast:           Option<&'a UnicastAddress<'a>>,
    pub first_anycast:           Option<&'a MulticastAddress<'a>>,
    pub first_multicast:         Option<&'a MulticastAddress<'a>>,
    pub first_dns:               Option<&'a MulticastAddress<'a>>,
    pub dns_suffix:              WCharPtr<'a>,
    pub description:             WCharPtr<'a>,
    pub friendly_name:           WCharPtr<'a>,
    pub physical_address:        [u8; 8],
    pub physical_address_length: u32,
    pub flags:                   u32,
    pub mtu:                     u32,
    pub if_type:                 u32,
    pub operating_status:        u32,
    pub index_v6:                u32,
    pub zone_indices:            [u16; 16],
    pub first_prefix:            Option<&'a MulticastAddress<'a>>,
}
#[repr(C)]
pub struct SockBuffer<'a> {
    pub len: u32,
    buf:     SockPointer,
    _p:      PhantomData<&'a [u8]>,
}
#[repr(C)]
pub struct AddressInfo<'a> {
    pub flags:        u32,
    pub family:       u32,
    pub socktype:     u32,
    pub protocol:     u32,
    pub address_size: usize,
    pub canon_name:   CharPtr<'a>,
    pub addr:         Option<&'a SocketAddress<'a>>,
    pub next:         Option<&'a AddressInfo<'a>>,
}
#[repr(C)]
pub struct SocketAddress<'a> {
    pub address: Option<&'a SockAddr>,
    pub length:  u32,
}
#[repr(C)]
pub struct UnicastAddress<'a> {
    pub length:             u32,
    pub flags:              u32,
    pub next:               Option<&'a UnicastAddress<'a>>,
    pub addr:               SocketAddress<'a>,
    pub prefix_origin:      u32,
    pub suffix_origin:      u32,
    pub dad_state:          u32,
    pub valid_lifetime:     u32,
    pub preferred_lifetime: u32,
    pub lease_lifetime:     u32,
    pub link_prefix_length: u8,
}
#[repr(transparent)]
pub struct OwnedSocket(Handle);
#[repr(C)]
pub struct AddressResolver<'a> {
    cur: Option<&'a AddressInfo<'a>>,
    _o:  AddressInfoPointer<'a>,
}
#[repr(C)]
pub struct MulticastAddress<'a> {
    pub length: u32,
    pub flags:  u32,
    pub next:   Option<&'a MulticastAddress<'a>>,
    pub addr:   SocketAddress<'a>,
}
#[repr(C)]
pub struct AddressResolverUnsafe {
    _cur: Option<NonZeroUsize>,
    _o:   usize,
}
#[repr(transparent)]
pub struct SockTimeout(Option<[u32; 2]>);
#[repr(transparent)]
pub struct AddressInfoPointer<'a>(*const AddressInfo<'a>);

#[repr(C)]
union SockPointer {
    read:  *mut u8,
    write: *const u8,
}

impl SockAddr {
    #[inline]
    pub fn new(v: &SocketAddr) -> (SockAddr, u32) {
        match v {
            SocketAddr::V4(a) => (
                SockAddr {
                    family: 0x2,
                    port:   u16::from_be(a.port()),
                    addr4:  u32::from_le_bytes(a.ip().octets()),
                    addr6:  [0u8; 16],
                    scope:  0u32,
                },
                0x10,
            ),
            SocketAddr::V6(a) => (
                SockAddr {
                    family: 0x17,
                    port:   u16::from_be(a.port()),
                    addr4:  a.flowinfo(),
                    addr6:  a.ip().octets(),
                    scope:  a.scope_id(),
                },
                0x1C,
            ),
        }
    }

    #[inline]
    pub fn addr(&self) -> Option<SocketAddr> {
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
impl OwnedSocket {
    #[inline]
    pub unsafe fn take(v: &mut OwnedSocket) -> OwnedSocket {
        OwnedSocket(replace(&mut v.0, Handle::EMPTY))
    }
}
impl SockTimeout {
    #[inline]
    pub fn as_ptr(&self) -> Option<&[u32; 2]> {
        self.0.as_ref()
    }
}
impl<'a> SockBuffer<'a> {
    #[inline]
    pub fn write(v: &'a [u8]) -> SockBuffer<'a> {
        SockBuffer {
            len: len_to_u32(v.len()),
            buf: SockPointer { write: v.as_ptr() },
            _p:  PhantomData,
        }
    }
    #[inline]
    pub fn read(v: &'a mut [u8]) -> SockBuffer<'a> {
        SockBuffer {
            len: len_to_u32(v.len()),
            buf: SockPointer { read: v.as_mut_ptr() },
            _p:  PhantomData,
        }
    }
}
impl UnicastAddress<'_> {
    #[inline]
    pub fn address(&self) -> Option<SocketAddr> {
        self.addr.address?.addr()
    }
}
impl<'a> AddressInfo<'a> {
    #[inline]
    pub fn new(flags: u32, family: u32, socktype: u32) -> AddressInfo<'a> {
        AddressInfo {
            flags,
            family,
            socktype,
            addr: None,
            next: None,
            protocol: 0u32,
            canon_name: CharPtr::null(),
            address_size: 0usize,
        }
    }
}
impl MulticastAddress<'_> {
    #[inline]
    pub fn address(&self) -> Option<SocketAddr> {
        self.addr.address.as_ref()?.addr()
    }
}
impl AddressResolverUnsafe {
    #[inline]
    fn resolver<'a>(&'a mut self) -> &'a mut AddressResolver<'a> {
        unsafe { transmute::<_, &mut AddressResolver<'_>>(self) }
    }
}

impl Eq for OwnedSocket {}
impl Drop for OwnedSocket {
    #[inline]
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = WSACloseSocket(self);
        }
    }
}
impl Deref for OwnedSocket {
    type Target = Handle;

    fn deref(&self) -> &Handle {
        &self.0
    }
}
impl PartialEq for OwnedSocket {
    #[inline]
    fn eq(&self, other: &OwnedSocket) -> bool {
        self.0 == other.0
    }
}
impl From<Handle> for OwnedSocket {
    #[inline]
    fn from(v: Handle) -> OwnedSocket {
        OwnedSocket(v)
    }
}
impl AsRef<Handle> for OwnedSocket {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

impl From<usize> for FileSet {
    #[inline]
    fn from(v: usize) -> FileSet {
        let mut f = FileSet { count: 1, array: [0; 64] };
        unsafe { *f.array.get_unchecked_mut(0) = v };
        f
    }
}
impl From<Handle> for FileSet {
    #[inline]
    fn from(v: Handle) -> FileSet {
        FileSet::from(*v)
    }
}
impl From<&Handle> for FileSet {
    #[inline]
    fn from(v: &Handle) -> FileSet {
        FileSet::from(*v)
    }
}
impl From<&OwnedSocket> for FileSet {
    #[inline]
    fn from(v: &OwnedSocket) -> FileSet {
        FileSet::from(**v)
    }
}

impl From<Option<Duration>> for SockTimeout {
    #[inline]
    fn from(v: Option<Duration>) -> SockTimeout {
        match v {
            None => SockTimeout(None),
            Some(d) => {
                let mut i = [
                    len_to_u32(d.as_secs() as usize),
                    len_to_u32(((d.as_micros() as u64).checked_sub(d.as_secs() * 1_000_000).unwrap_or(0)) as usize),
                ];
                unsafe {
                    if *i.get_unchecked_mut(0) == 0 && *i.get_unchecked(1) == 0 {
                        *i.get_unchecked_mut(1) = 1;
                    }
                }
                SockTimeout(Some(i))
            },
        }
    }
}

impl Default for SockInfo {
    #[inline]
    fn default() -> SockInfo {
        SockInfo {
            pad1:           [0u32; 19],
            pad2:           [0u32; 2],
            pad3:           [0u32; 5],
            pad4:           [0u16; 256],
            protocol:       0u32,
            socket_type:    0u32,
            address_family: 0u32,
        }
    }
}
impl<'a> Default for AddressInfo<'a> {
    #[inline]
    fn default() -> AddressInfo<'a> {
        AddressInfo {
            addr:         None,
            next:         None,
            flags:        0u32,
            family:       0u32,
            socktype:     0u32,
            protocol:     0u32,
            address_size: 0usize,
            canon_name:   CharPtr::null(),
        }
    }
}

impl Deref for SockTimeout {
    type Target = Option<[u32; 2]>;

    #[inline]
    fn deref(&self) -> &Option<[u32; 2]> {
        &self.0
    }
}

impl Drop for AddressInfoPointer<'_> {
    #[inline]
    fn drop(&mut self) {
        WSAFreeAddrInfo(self.0);
    }
}
impl<'a> IntoIterator for AddressInfoPointer<'a> {
    type Item = SocketAddr;
    type IntoIter = AddressResolver<'a>;

    #[inline]
    fn into_iter(self) -> AddressResolver<'a> {
        AddressResolver {
            cur: Some(unsafe { &*self.0 }),
            _o:  self,
        }
    }
}

impl Iterator for AddressResolver<'_> {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<SocketAddr> {
        loop {
            let v = match self.cur {
                Some(v) => v,
                None => return None,
            };
            self.cur = v.next;
            if let Some(i) = v.into() {
                return Some(i);
            }
        }
    }
}
impl FusedIterator for AddressResolver<'_> {}
impl<'a> From<AddressInfoPointer<'a>> for AddressResolver<'a> {
    #[inline]
    fn from(v: AddressInfoPointer<'a>) -> AddressResolver<'a> {
        AddressResolver {
            cur: Some(unsafe { &*v.0 }),
            _o:  v,
        }
    }
}

impl Drop for AddressResolverUnsafe {
    #[inline]
    fn drop(&mut self) {
        unsafe { drop_in_place(self.resolver()) };
    }
}
impl Iterator for AddressResolverUnsafe {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.resolver().next()
    }
}
impl FusedIterator for AddressResolverUnsafe {}
impl From<AddressInfoPointer<'_>> for AddressResolverUnsafe {
    #[inline]
    fn from(v: AddressInfoPointer<'_>) -> AddressResolverUnsafe {
        // Prevent it from being dropped as we elude the lifetime.
        unsafe { transmute::<AddressResolver<'_>, _>(v.into()) }
    }
}

impl Default for SockAddr {
    #[inline]
    fn default() -> SockAddr {
        SockAddr {
            port:   0u16,
            addr4:  0u32,
            addr6:  [0u8; 16],
            scope:  0u32,
            family: 0u16,
        }
    }
}
impl TryFrom<SockAddr> for SocketAddr {
    type Error = Win32Error;

    #[inline]
    fn try_from(v: SockAddr) -> Win32Result<SocketAddr> {
        v.addr().ok_or(Win32Error::InvalidObject)
    }
}
impl From<&SockAddr> for Option<SocketAddr> {
    #[inline]
    fn from(v: &SockAddr) -> Option<SocketAddr> {
        v.addr()
    }
}
impl<'a> From<&'a AddressInfo<'a>> for Option<SocketAddr> {
    #[inline]
    fn from(v: &AddressInfo) -> Option<SocketAddr> {
        unsafe { transmute::<_, &SockAddr>(v.addr) }.addr()
    }
}

impl<'a> From<*mut AddressInfo<'a>> for AddressInfoPointer<'a> {
    #[inline]
    fn from(v: *mut AddressInfo<'a>) -> AddressInfoPointer<'a> {
        AddressInfoPointer(v)
    }
}
impl<'a> From<*const AddressInfo<'a>> for AddressInfoPointer<'a> {
    #[inline]
    fn from(v: *const AddressInfo<'a>) -> AddressInfoPointer<'a> {
        AddressInfoPointer(v)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::structs::OwnedSocket;

    impl Debug for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "OwnedSocket: 0x{:X}", self.0)
        }
    }
    impl LowerHex for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for OwnedSocket {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
