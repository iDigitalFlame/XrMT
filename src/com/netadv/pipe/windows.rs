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

use core::net::SocketAddr;
use core::time::Duration;

use crate::com::netadv::{foreach_addr, Conn, Listener};
use crate::device::winapi::{self, OwnedHandle};
use crate::io;
use crate::net::{TcpListener, TcpStream};
use crate::prelude::*;

pub struct PipeListener {
    handle: OwnedHandle,
    name:   String,
}

impl PipeListener {
    pub fn new() {}
}
