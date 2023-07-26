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
#![allow(non_snake_case)]

use core::net::SocketAddr;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;
use core::{cmp, mem, ptr};

use crate::device::winapi::functions::{FDSet, SockAddr, SockBuffer, SockInfo, SockPointer};
use crate::device::winapi::loader::winsock;
use crate::device::winapi::{self, AddressInfo, Chars, Handle, OwnedSocket, Win32Error, Win32Result};
use crate::util::stx;
use crate::util::stx::prelude::*;

static WSA_INIT: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn WSAFreeAddrInfo(info: *const AddressInfo) {
    if info.is_null() {
        return;
    }
    winapi::init_winsock();
    unsafe {
        winapi::syscall!(
            *winsock::FreeAddrInfo,
            extern "stdcall" fn(*const AddressInfo),
            info
        )
    };
}
pub fn WSACloseSocket(sock: &OwnedSocket) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        wsa_init();
        winapi::syscall!(
            *winsock::CloseSocket,
            extern "stdcall" fn(usize) -> i32,
            sock.0
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSAGetSockName(sock: &OwnedSocket) -> Win32Result<SocketAddr> {
    winapi::init_winsock();
    let mut a = SockAddr::default();
    let e = unsafe {
        let mut n = 0x1Cu32;
        wsa_init();
        winapi::syscall!(
            *winsock::GetSockName,
            extern "stdcall" fn(usize, *mut SockAddr, *mut u32) -> i32,
            sock.0,
            &mut a,
            &mut n
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        a.try_into()
    }
}
pub fn WSAGetPeerName(sock: &OwnedSocket) -> Win32Result<SocketAddr> {
    winapi::init_winsock();
    let mut a = SockAddr::default();
    let e = unsafe {
        let mut n = 0x1Cu32;
        wsa_init();
        winapi::syscall!(
            *winsock::GetPeerName,
            extern "stdcall" fn(usize, *mut SockAddr, *mut u32) -> i32,
            sock.0,
            &mut a,
            &mut n
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        a.try_into()
    }
}
pub fn WSAListen(sock: &OwnedSocket, backlog: i32) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        wsa_init();
        winapi::syscall!(
            *winsock::Listen,
            extern "stdcall" fn(usize, u32) -> i32,
            sock.0,
            backlog as u32
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSAShutdownSock(sock: &OwnedSocket, how: u32) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        wsa_init();
        winapi::syscall!(
            *winsock::SetSockOpt,
            extern "stdcall" fn(usize, u32) -> i32,
            sock.0,
            how
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSAWaitSock(sock: &OwnedSocket, dur: Duration) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        let mut f = sock.0.into();
        let mut t = [
            cmp::min(dur.as_secs(), u32::MAX as u64) as u32,
            (dur.subsec_nanos() / 1000),
        ];
        if t[0] == 0 && t[1] == 0 {
            t[1] = 1;
        }
        wsa_init();
        winapi::syscall!(
            *winsock::Select,
            extern "stdcall" fn(u32, *mut FDSet, *mut FDSet, *mut FDSet, *const u32) -> i32,
            1,
            ptr::null_mut(),
            &mut f,
            &mut f,
            t.as_ptr()
        ) == -1
    };
    if e {
        return Err(winapi::last_error());
    }
    // 0xFFFF - SOL_SOCKET
    // 0x1007 - SO_ERROR
    let r: u32 = WSAGetSockOption(sock, 0xFFFF, 0x1007)?;
    if r > 0 {
        Err(Win32Error::Code(r))
    } else {
        Ok(())
    }
}
pub fn WSADuplicateSocket(sock: &OwnedSocket) -> Win32Result<OwnedSocket> {
    winapi::init_winsock();
    let mut i = SockInfo::default();
    let e = unsafe {
        wsa_init();
        winapi::syscall!(
            *winsock::WSADuplicateSocket,
            extern "stdcall" fn(usize, u32, *mut SockInfo) -> i32,
            sock.0,
            winapi::GetCurrentProcessID(),
            &mut i
        ) != 0
    };
    if e {
        return Err(winapi::last_error());
    }
    WSASocket(i.address_family, i.socket_type, i.protocol, true, false)
}
pub fn WSABind(sock: &OwnedSocket, address: &SocketAddr) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        let (a, n) = SockAddr::convert(address);
        wsa_init();
        winapi::syscall!(
            *winsock::Bind,
            extern "stdcall" fn(usize, *const SockAddr, u32) -> i32,
            sock.0,
            &a,
            n
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSAAccept(sock: &OwnedSocket) -> Win32Result<(OwnedSocket, SocketAddr)> {
    winapi::init_winsock();
    let mut a = SockAddr::default();
    let h = unsafe {
        let mut n = 0x1Cu32;
        wsa_init();
        winapi::syscall!(
            *winsock::WSAAccept,
            extern "stdcall" fn(usize, *mut SockAddr, *mut u32, usize, usize) -> isize,
            sock.0,
            &mut a,
            &mut n,
            0,
            0
        )
    };
    if h == 0 || h == -1 {
        Err(winapi::last_error())
    } else {
        Ok((OwnedSocket(h as usize), a.try_into()?))
    }
}
pub fn WSAConnect(sock: &OwnedSocket, address: &SocketAddr) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        let (a, n) = SockAddr::convert(address);
        wsa_init();
        winapi::syscall!(
            *winsock::Connect,
            extern "stdcall" fn(usize, *const SockAddr, u32) -> i32,
            sock.0,
            &a,
            n
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSASend(sock: &OwnedSocket, flags: u32, buf: &[u8]) -> Win32Result<usize> {
    winapi::init_winsock();
    let mut n = 0u32;
    let e = unsafe {
        let b = SockBuffer {
            len: cmp::min(buf.len(), u32::MAX as usize) as u32,
            buf: SockPointer { write: buf.as_ptr() },
        };
        wsa_init();
        winapi::syscall!(
            *winsock::WSASend,
            extern "stdcall" fn(usize, *const SockBuffer, u32, *mut u32, u32, usize, usize) -> i32,
            sock.0,
            &b,
            1,
            &mut n,
            flags,
            0,
            0
        ) == 0
    };
    let c = winapi::GetLastError();
    match c {
        _ if e => Ok(n as usize),
        /* WSAESHUTDOWN */ 0x274A => Ok(0),
        _ => Err(Win32Error::Code(c)),
    }
}
pub fn WSARecv(sock: &OwnedSocket, flags: u32, buf: &mut [u8]) -> Win32Result<usize> {
    winapi::init_winsock();
    let mut n = 0;
    let e = unsafe {
        let mut f = flags;
        let mut b = SockBuffer {
            len: cmp::min(buf.len(), u32::MAX as usize) as u32,
            buf: SockPointer { read: buf.as_mut_ptr() },
        };
        wsa_init();
        winapi::syscall!(
            *winsock::WSARecv,
            extern "stdcall" fn(usize, *mut SockBuffer, u32, *mut u32, *mut u32, usize, usize) -> i32,
            sock.0,
            &mut b,
            1,
            &mut n,
            &mut f,
            0,
            0
        ) == 0
    };
    let c = winapi::GetLastError();
    match c {
        _ if e => Ok(n as usize),
        /* WSAESHUTDOWN */ 0x274A => Ok(0),
        _ => Err(Win32Error::Code(c)),
    }
}
pub fn WSAGetSockOption<T>(sock: &OwnedSocket, level: u32, opt: u32) -> Win32Result<T> {
    winapi::init_winsock();
    let mut v: T = unsafe { mem::zeroed() };
    let e = unsafe {
        let mut n = cmp::min(mem::size_of::<T>(), u32::MAX as usize) as u32;
        wsa_init();
        winapi::syscall!(
            *winsock::GetSockOpt,
            extern "stdcall" fn(usize, u32, u32, *mut T, *mut u32) -> i32,
            sock.0,
            level,
            opt,
            &mut v,
            &mut n
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(v)
    }
}
pub fn WSAIoctl<T>(sock: &OwnedSocket, code: u32, input: T, output: *mut T) -> Win32Result<u32> {
    winapi::init_winsock();
    let mut n = 0u32;
    let e = unsafe {
        let x = cmp::min(mem::size_of::<T>(), u32::MAX as usize) as u32;
        wsa_init();
        winapi::syscall!(
            *winsock::WSAIoctl,
            extern "stdcall" fn(usize, u32, *const T, u32, *mut T, u32, *mut u32, *mut usize, usize) -> i32,
            sock.0,
            code,
            &input,
            x,
            output,
            if output.is_null() { 0 } else { x },
            &mut n,
            ptr::null_mut(),
            0
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(n)
    }
}
pub fn WSASetSockOption<T>(sock: &OwnedSocket, level: u32, opt: u32, value: T) -> Win32Result<()> {
    winapi::init_winsock();
    let e = unsafe {
        wsa_init();
        winapi::syscall!(
            *winsock::SetSockOpt,
            extern "stdcall" fn(usize, u32, u32, *const T, u32) -> i32,
            sock.0,
            level,
            opt,
            &value,
            cmp::min(mem::size_of::<T>(), u32::MAX as usize) as u32
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WSARecvFrom(sock: &OwnedSocket, flags: u32, buf: &mut [u8]) -> Win32Result<(usize, SocketAddr)> {
    winapi::init_winsock();
    let mut n = 0u32;
    let mut a = SockAddr::default();
    let e = unsafe {
        let mut f = flags;
        let mut b = SockBuffer {
            len: cmp::min(buf.len(), u32::MAX as usize) as u32,
            buf: SockPointer { read: buf.as_mut_ptr() },
        };
        let mut s = 0x1Cu32;
        wsa_init();
        winapi::syscall!(
            *winsock::WSARecvFrom,
            extern "stdcall" fn(usize, *mut SockBuffer, u32, *mut u32, *mut u32, *mut SockAddr, *mut u32, usize, usize) -> i32,
            sock.0,
            &mut b,
            1,
            &mut n,
            &mut f,
            &mut a,
            &mut s,
            0,
            0
        ) == 0
    };
    let c = winapi::GetLastError();
    match c {
        _ if e => Ok((n as usize, a.try_into()?)),
        /* WSAESHUTDOWN */ 0x274A => Ok((0, a.try_into()?)),
        _ => Err(Win32Error::Code(c)),
    }
}
pub fn WSASendTo(sock: &OwnedSocket, address: &SocketAddr, flags: u32, buf: &[u8]) -> Win32Result<usize> {
    winapi::init_winsock();
    let mut n = 0u32;
    let e = unsafe {
        let b = SockBuffer {
            len: cmp::min(buf.len(), u32::MAX as usize) as u32,
            buf: SockPointer { write: buf.as_ptr() },
        };
        let (a, s) = SockAddr::convert(address);
        wsa_init();
        winapi::syscall!(
            *winsock::WSASendTo,
            extern "stdcall" fn(usize, *const SockBuffer, u32, *mut u32, u32, *const SockAddr, u32, usize, usize) -> i32,
            sock.0,
            &b,
            1,
            &mut n,
            flags,
            &a,
            s,
            0,
            0
        ) == 0
    };
    let c = winapi::GetLastError();
    match c {
        _ if e => Ok(n as usize),
        /* WSAESHUTDOWN */ 0x274A => Ok(0),
        _ => Err(Win32Error::Code(c)),
    }
}
pub fn WSAGetAddrInfo(host: impl AsRef<str>, socktype: u32, family: u32, flags: u32) -> Win32Result<*const AddressInfo> {
    winapi::init_winsock();
    let mut r: *mut AddressInfo = ptr::null_mut();
    let e = unsafe {
        let n: Chars = host.as_ref().into();
        let h = AddressInfo::new(flags, family, socktype);
        wsa_init();
        winapi::syscall!(
            *winsock::GetAddrInfo,
            extern "stdcall" fn(*const u8, *const u8, *const AddressInfo, *mut *mut AddressInfo) -> i32,
            n.as_ptr(),
            ptr::null(),
            &h,
            &mut r
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn WSASocket(family: u32, socket_type: u32, protocol: u32, blocking: bool, inherit: bool) -> Win32Result<OwnedSocket> {
    winapi::init_winsock();
    let v = winapi::is_min_windows_8();
    let h = unsafe {
        wsa_init();
        // 0x01 - WSA_FLAG_OVERLAPPED
        // 0x80 - WSA_FLAG_NO_HANDLE_INHERIT
        winapi::syscall!(
            *winsock::WSASocketW,
            extern "stdcall" fn(u32, u32, u32, *mut usize, u32, u32) -> isize,
            family,
            socket_type,
            protocol,
            ptr::null_mut(),
            0,
            if !blocking { 0x1 } else { 0 } | if v && !inherit { 0x80 } else { 0 }
        )
    };
    if h == 0 || h == -1 {
        Err(winapi::last_error())
    } else {
        let x = Handle(h as usize);
        if !v && !inherit {
            // If we're older than Windows 8, try to remove the inheritance flag.
            let _ = winapi::SetHandleInformation(x, false, false); // IGNORE ERROR
        }
        Ok(OwnedSocket(x.0))
    }
}

#[inline]
fn wsa_init() {
    if WSA_INIT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        // NOTE(dij): If this fails, we have bigger problems.
        stx::unwrap(wsa_startup(0x202))
    }
}
fn wsa_startup(version: u16) -> Win32Result<()> {
    let mut d = [0u8; 0x18A + winapi::PTR_SIZE];
    let e = unsafe {
        winapi::syscall!(
            *winsock::WSAStartup,
            extern "stdcall" fn(u16, *const u8) -> i32,
            version,
            d.as_mut_ptr()
        ) != 0
    };
    if e {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
