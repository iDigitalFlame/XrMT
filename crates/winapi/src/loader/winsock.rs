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

extern crate xrmt_bugtrack;
extern crate xrmt_crypt;
extern crate xrmt_winapi_fnv;

use xrmt_crypt::crypt;

crate::dll!(
    Winsock,
    WINSOCK,
    winsock,
    || crypt!(0, "ws2_32.dll"),
    bind,
    listen,
    select,
    connect,
    shutdown,
    closesocket,
    setsockopt,
    getsockopt,
    getpeername,
    getaddrinfo,
    freeaddrinfo,
    getsockname,
    WSAConnect, // TODO(dij): Should we use this instead? It DOES exist on Xp
    WSAStartup,
    WSACleanup,
    WSAIoctl,
    WSAEventSelect,
    WSAAccept,
    WSASocketW,
    WSADuplicateSocketW,
    WSASend,
    WSASendTo,
    WSARecv,
    WSARecvFrom,
    WSAIsBlocking // TODO(dij): Support
);
