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
#![allow(non_snake_case)]

extern crate core;

use core::convert::{AsRef, From, Into, TryInto};
use core::default::Default;
use core::mem::size_of;
use core::net::SocketAddr;
use core::option::Option;
use core::ptr::{null, null_mut};
use core::result::Result::{Err, Ok};
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

use crate::functions::{len_to_u32, GetCurrentProcessID, SetHandleInformation};
use crate::info::is_min_windows_8;
use crate::structs::{AddressInfo, AddressInfoPointer, Chars, FileSet, Handle, MaybeOverlapped, Overlapped, OverlappedPtr, OwnedSocket, SockAddr, SockBuffer, SockInfo, SockTimeout};
use crate::{winsock, Win32Error, Win32Result, PTR_SIZE};

const WSA_VERSION: u16 = 0x202u16;

static WSA_INIT: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn WSAFreeAddrInfo(info: *const AddressInfo<'_>) {
    syscall!(winsock().freeaddrinfo, (*const AddressInfo) -> (), info)
}
#[inline]
pub fn WSACloseSocket(sock: &OwnedSocket) -> Win32Result<()> {
    if syscall!(winsock().closesocket, (Handle) -> i32, **sock) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSAGetSockName(sock: &OwnedSocket) -> Win32Result<SocketAddr> {
    let (mut n, mut a) = (0x1Cu32, SockAddr::default());
    let r = syscall!(winsock().getsockname, (Handle, *mut SockAddr, *mut u32) -> i32, **sock, &mut a, &mut n) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        a.try_into()
    }
}
#[inline]
pub fn WSAGetPeerName(sock: &OwnedSocket) -> Win32Result<SocketAddr> {
    let (mut n, mut a) = (0x1Cu32, SockAddr::default());
    let r = syscall!(winsock().getpeername, (Handle, *mut SockAddr, *mut u32) -> i32, **sock, &mut a, &mut n) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        a.try_into()
    }
}
#[inline]
pub fn WSAListen(sock: &OwnedSocket, backlog: i32) -> Win32Result<()> {
    if syscall!(winsock().listen, (Handle, u32) -> i32, **sock, backlog as u32) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSAShutdownSock(sock: &OwnedSocket, how: u32) -> Win32Result<()> {
    if syscall!(winsock().shutdown, (Handle, u32) -> i32, **sock, how) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSADuplicateSocket(sock: &OwnedSocket) -> Win32Result<OwnedSocket> {
    let mut i = SockInfo::default();
    if syscall!(winsock().WSADuplicateSocketW, (Handle, u32, *mut SockInfo) -> i32, **sock, GetCurrentProcessID(), &mut i) != 0 {
        Err(Win32Error::last_error())
    } else {
        WSASocket(i.address_family, i.socket_type, i.protocol, true, false)
    }
}
#[inline]
pub fn WSABind(sock: &OwnedSocket, address: &SocketAddr) -> Win32Result<()> {
    let (a, n) = SockAddr::new(address);
    if syscall!(winsock().bind, (Handle, *const SockAddr, u32) -> i32, **sock, &a, n) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSAAccept(sock: &OwnedSocket) -> Win32Result<(OwnedSocket, SocketAddr)> {
    let (mut n, mut a) = (0x1Cu32, SockAddr::default());
    let h = syscall!(winsock().WSAAccept, (Handle, *mut SockAddr, *mut u32, usize, usize)-> isize, **sock, &mut a, &mut n, 0, 0);
    if h == 0 || h == -1 {
        Err(Win32Error::last_error())
    } else {
        Ok((Handle::new(h as usize).into(), a.try_into()?))
    }
}
#[inline]
pub fn WSAConnect(sock: &OwnedSocket, address: &SocketAddr) -> Win32Result<()> {
    let (a, n) = SockAddr::new(address);
    if syscall!(winsock().connect, (Handle, *const SockAddr, u32) -> i32, **sock, &a, n) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
pub fn WSAWaitSock(sock: &OwnedSocket, dur: Option<Duration>) -> Win32Result<()> {
    let (mut f, t) = (sock.into(), SockTimeout::from(dur));
    match syscall!(winsock().select, (u32, *mut FileSet, *mut FileSet, *mut FileSet,  Option<&[u32; 2]>) -> i32, 1, &mut f, &mut f, &mut f, t.as_ref()) {
        -1 => return Err(Win32Error::last_error()),
        0 if dur.is_some() => return Err(Win32Error::TimedOut),
        0 if dur.is_none() => return Err(Win32Error::IoPending),
        _ => (),
    }
    // 0xFFFF - SOL_SOCKET
    // 0x1007 - SO_ERROR
    let r: u32 = WSAGetSockOption(sock, 0xFFFF, 0x1007)?;
    if r > 0 {
        Err(Win32Error::from_code(r))
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSAGetSockOption<T: Default>(sock: &OwnedSocket, level: u32, opt: u32) -> Win32Result<T> {
    let (mut v, mut n) = (T::default(), len_to_u32(size_of::<T>()));
    if syscall!(winsock().getsockopt, (Handle, u32, u32, *mut T, *mut u32) -> i32, **sock, level, opt, &mut v, &mut n) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(v)
    }
}
#[inline]
pub fn WSASetSockOption<T>(sock: &OwnedSocket, level: u32, opt: u32, value: T) -> Win32Result<()> {
    if syscall!(winsock().setsockopt, (Handle, u32, u32, *const T, u32) -> i32, **sock, level, opt, &value, len_to_u32(size_of::<T>())) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn WSAEventSelect(sock: &OwnedSocket, event: impl AsRef<Handle>, events: u32) -> Win32Result<()> {
    if syscall!(winsock().WSAEventSelect, (Handle, Handle, u32) -> u32, **sock, *event.as_ref(), events) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
pub fn WSASend(sock: &OwnedSocket, olp: MaybeOverlapped, flags: u32, buf: &[u8]) -> Win32Result<usize> {
    let (mut n, b) = (0u32, SockBuffer::write(buf));
    let o = OverlappedPtr::new(olp);
    let r = syscall!(
        winsock().WSASend,
        (Handle, *const SockBuffer, u32, *mut u32, u32, *mut Overlapped, usize) -> i32,
        **sock,
        &b,
        1,
        &mut n,
        flags,
        o.apc(),
        0
    ) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n as usize)
    }
}
pub fn WSARecv(sock: &OwnedSocket, olp: MaybeOverlapped, flags: u32, buf: &mut [u8]) -> Win32Result<usize> {
    let (mut n, mut b, mut f) = (0u32, SockBuffer::read(buf), flags);
    let o = OverlappedPtr::new(olp);
    let r = syscall!(
        winsock().WSARecv, (Handle, *mut SockBuffer, u32, *mut u32, *mut u32, *mut Overlapped, usize) -> i32,
        **sock,
        &mut b,
        1,
        &mut n,
        &mut f,
        o.apc(),
        0
    ) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n as usize)
    }
}
pub fn WSAIoctl<T>(sock: &OwnedSocket, olp: MaybeOverlapped, code: u32, input: T, output: &mut T) -> Win32Result<u32> {
    let mut n = 0u32;
    let i = len_to_u32(size_of::<T>());
    let o = OverlappedPtr::new(olp);
    let r = syscall!(
        winsock().WSAIoctl,
        (Handle, u32, *const T, u32, *mut T, u32, *mut u32, *mut Overlapped, usize) -> i32,
        **sock,
        code,
        &input,
        i,
        output,
        i,
        &mut n,
        o.apc(),
        0
    ) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n)
    }
}
pub fn WSASocket(family: u32, socket_type: u32, protocol: u32, blocking: bool, inherit: bool) -> Win32Result<OwnedSocket> {
    wsa_init()?;
    let v = is_min_windows_8();
    let h = syscall!(
        winsock().WSASocketW,
        (u32, u32, u32, *mut usize, u32, u32) -> isize,
        family,
        socket_type,
        protocol,
        null_mut(),
        0,
        // 0x01 - WSA_FLAG_OVERLAPPED
        // 0x80 - WSA_FLAG_NO_HANDLE_INHERIT
        if !blocking { 0x1 } else { 0 } | if v && !inherit { 0x80 } else { 0 }
    );
    if h == 0 || h == -1 {
        Err(Win32Error::last_error())
    } else {
        let x = Handle::new(h as usize);
        if !v && !inherit {
            // If we're older than Windows 8, try to remove the inheritance flag.
            let _ = SetHandleInformation(x, false, false);
        }
        Ok(x.into())
    }
}
pub fn WSARecvFrom(sock: &OwnedSocket, olp: MaybeOverlapped, flags: u32, buf: &mut [u8]) -> Win32Result<(usize, SocketAddr)> {
    let (mut f, mut s) = (flags, 0x1Cu32);
    let (mut n, mut a, mut b) = (0u32, SockAddr::default(), SockBuffer::read(buf));
    let o = OverlappedPtr::new(olp);
    let r = syscall!(
        winsock().WSARecvFrom,
        (Handle, *mut SockBuffer, u32, *mut u32, *mut u32, *mut SockAddr, *mut u32, *mut Overlapped, usize) -> i32,
        **sock,
        &mut b,
        1,
        &mut n,
        &mut f,
        &mut a,
        &mut s,
        o.apc(),
        0
    ) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok((n as usize, a.try_into()?))
    }
}
pub fn WSASendTo(sock: &OwnedSocket, address: &SocketAddr, olp: MaybeOverlapped, flags: u32, buf: &[u8]) -> Win32Result<usize> {
    let (a, s) = SockAddr::new(address);
    let (mut n, b) = (0u32, SockBuffer::write(buf));
    let o = OverlappedPtr::new(olp);
    let r = syscall!(
        winsock().WSASendTo,
        (Handle, *const SockBuffer, u32, *mut u32, u32, *const SockAddr, u32, *mut Overlapped, usize) -> i32,
        **sock,
        &b,
        1,
        &mut n,
        flags,
        &a,
        s,
        o.apc(),
        0
    ) != 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n as usize)
    }
}
pub fn WSAGetAddrInfo<'a>(host: impl AsRef<str>, socktype: u32, family: u32, flags: u32) -> Win32Result<AddressInfoPointer<'a>> {
    wsa_init()?;
    let mut n: Chars = host.as_ref().into();
    n.push(0); // Add NULL ending.
    let h = AddressInfo::new(flags, family, socktype);
    let mut v: *mut AddressInfo = null_mut();
    if syscall!(winsock().getaddrinfo, (*const u8, *const u8, *const AddressInfo, *mut *mut AddressInfo) -> u32, n.as_ptr(), null(), &h, &mut v) != 0 {
        Err(Win32Error::last_error())
    } else if v.is_null() {
        // 0x2AFA - WSATRY_AGAIN
        Err(Win32Error::IoPending)
    } else {
        Ok(v.into())
    }
}

#[inline]
pub(crate) fn wsa_cleanup() {
    if WSA_INIT.load(Ordering::Acquire) {
        let _ = syscall!(winsock().WSAStartup, () -> i32,);
    }
}

#[inline]
fn wsa_init() -> Win32Result<()> {
    if WSA_INIT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        wsa_startup()?;
    }
    Ok(())
}
#[inline]
fn wsa_startup() -> Win32Result<()> {
    let mut d = [0u8; 0x18A + PTR_SIZE];
    if syscall!(winsock().WSAStartup, (u16, *const u8) -> i32, WSA_VERSION, d.as_mut_ptr()) != 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
