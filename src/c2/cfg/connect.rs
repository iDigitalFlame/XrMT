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

use core::alloc::Allocator;

use crate::c2::cfg::OwnedConfig;
use crate::prelude::*;

pub const CONNECT_TCP: u8 = 0xC0u8;
pub const CONNECT_TLS: u8 = 0xC1u8;
pub const CONNECT_UDP: u8 = 0xC2u8;
pub const CONNECT_ICMP: u8 = 0xC3u8;
pub const CONNECT_PIPE: u8 = 0xC4u8;
pub const CONNECT_TLS_NO_VERIFY: u8 = 0xC5u8;

pub(super) const CONNECT_IP: u8 = 0xB0u8;
pub(super) const CONNECT_WC2: u8 = 0xB1u8;
pub(super) const CONNECT_TLS_EX: u8 = 0xB2u8;
pub(super) const CONNECT_MU_TLS: u8 = 0xB3u8;
pub(super) const CONNECT_TLS_CA: u8 = 0xB4u8;
pub(super) const CONNECT_TLS_CERT: u8 = 0xB5u8;

impl<A: Allocator> OwnedConfig<A> {
    pub fn connect_tcp(mut self) -> OwnedConfig<A> {
        self.0.push(CONNECT_TCP);
        self
    }
    pub fn connect_udp(mut self) -> OwnedConfig<A> {
        self.0.push(CONNECT_UDP);
        self
    }
    pub fn connect_tls(mut self) -> OwnedConfig<A> {
        self.0.push(CONNECT_TLS);
        self
    }
    pub fn connect_icmp(mut self) -> OwnedConfig<A> {
        self.0.push(CONNECT_ICMP);
        self
    }
    pub fn connect_pipe(mut self) -> OwnedConfig<A> {
        self.0.push(CONNECT_PIPE);
        self
    }
    pub fn connect_tls_no_verify(mut self) -> OwnedConfig<A> {
        self
    }
    pub fn connect_ip(mut self, proto: u8) -> OwnedConfig<A> {
        self
    }
    pub fn connect_tls_ex(mut self, version: u8) -> OwnedConfig<A> {
        self
    }
    pub fn connect_tls_ex_ca(mut self, version: u8, ca: impl AsRef<[u8]>) -> OwnedConfig<A> {
        self
    }
    pub fn connect_tls_certs<T: AsRef<[u8]>>(mut self, version: u8, pem: T, key: T) -> OwnedConfig<A> {
        self
    }

    pub fn connect_tls_mu<T: AsRef<[u8]>>(mut self, version: u8, ca: T, pem: T, key: T) -> OwnedConfig<A> {
        self
    }
    pub fn connect_wc2<T: AsRef<str>>(mut self, url: T, host: Option<T>, agent: Option<T>, headers: Option<String>) -> OwnedConfig<A> {
        self
    }
}
