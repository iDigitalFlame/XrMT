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
use core::time::Duration;

use crate::c2::{write_packet, Transform, Wrapper};
use crate::com::{Flag, Packet};
use crate::device::local_id;
use crate::net::TcpStream;
use crate::prelude::*;
use crate::{ignore_error, io};

pub fn shoot(host: impl AsRef<str>, data: Packet) -> io::Result<()> {
    // TODO(dij): Profiles support.
    let mut c = TcpStream::connect(host.as_ref())?;
    let mut n = data;
    if n.device.is_empty() {
        n.device = local_id();
    }
    n.flags |= Flag::ONESHOT;
    ignore_error!(c.set_write_timeout(Some(Duration::from_secs(10))));
    write_packet(
        &mut c,
        &mut Wrapper::None::<Global>,
        &mut Transform::None,
        n,
    )
}
