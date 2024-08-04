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
#![cfg(target_family = "windows")]
#![allow(non_snake_case, non_upper_case_globals)]

extern crate core;

use crate::device::winapi;
use crate::device::winapi::loader::{Function, Loader};

pub(crate) static Bind: Function = Function::new();
pub(crate) static Listen: Function = Function::new();
pub(crate) static Select: Function = Function::new();
pub(crate) static Connect: Function = Function::new();
pub(crate) static Shutdown: Function = Function::new();
pub(crate) static CloseSocket: Function = Function::new();

pub(crate) static SetSockOpt: Function = Function::new();
pub(crate) static GetSockOpt: Function = Function::new();
pub(crate) static GetPeerName: Function = Function::new();
pub(crate) static GetSockName: Function = Function::new();

pub(crate) static GetAddrInfo: Function = Function::new();
pub(crate) static FreeAddrInfo: Function = Function::new();

pub(crate) static WSAStartup: Function = Function::new();
pub(crate) static WSACleanup: Function = Function::new();

pub(crate) static WSAIoctl: Function = Function::new();
pub(crate) static WSAAccept: Function = Function::new();
pub(crate) static WSASocketW: Function = Function::new();
pub(crate) static WSADuplicateSocket: Function = Function::new();

pub(crate) static WSASend: Function = Function::new();
pub(crate) static WSASendTo: Function = Function::new();

pub(crate) static WSARecv: Function = Function::new();
pub(crate) static WSARecvFrom: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|winsock| {
    winsock.proc(&Bind, 0x480D35A8);
    winsock.proc(&Listen, 0xC8DA78C8);
    winsock.proc(&Select, 0x556391B5);
    winsock.proc(&Connect, 0xDA57C9F1);
    winsock.proc(&Shutdown, 0xBD27B67);
    winsock.proc(&CloseSocket, 0x53D900A4);

    winsock.proc(&SetSockOpt, 0x6EEB99EE);
    winsock.proc(&GetSockOpt, 0x3BF2F0AA);
    winsock.proc(&GetPeerName, 0xC8540FEA);
    winsock.proc(&GetSockName, 0x5ADEAC8E);

    winsock.proc(&GetAddrInfo, 0x708FB562);
    winsock.proc(&FreeAddrInfo, 0xBF712706);

    winsock.proc(&WSAStartup, 0xAB5C89EB);
    winsock.proc(&WSACleanup, 0xE25E6CC4);

    winsock.proc(&WSAIoctl, 0xD21DC857);
    winsock.proc(&WSAAccept, 0x72486D38);
    winsock.proc(&WSASocketW, 0xF3FBFD8E);
    winsock.proc(&WSADuplicateSocket, 0x8D4B7867);

    winsock.proc(&WSASend, 0xF40FE60A);
    winsock.proc(&WSASendTo, 0x223A6331);

    winsock.proc(&WSARecv, 0x8C1B7B78);
    winsock.proc(&WSARecvFrom, 0x3BC5B208);
});

#[inline]
pub(super) fn wsa_cleanup() {
    if !WSACleanup.is_loaded() {
        return;
    }
    unsafe { winapi::syscall!(*WSACleanup, extern "stdcall" fn() -> i32,) };
}
