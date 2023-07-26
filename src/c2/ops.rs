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

use crate::c2::{Transform, Wrapper};
use crate::com::Packet;
use crate::data::Chunk;
use crate::util::stx::io::{self, Read, Write};
use crate::util::stx::prelude::*;

pub(super) fn read_packet(input: &mut impl Read, w: &mut Wrapper, t: &mut Transform) -> io::Result<Packet> {
    if w.is_none() && t.is_none() {
        let mut n = Packet::default();
        n.read_packet(input)?;
        return Ok(n);
    }
    let mut buf = Chunk::new();
    input.read_to_end(buf.as_vec())?;
    if !t.is_none() {
        let mut o = Chunk::new();
        o.as_vec().extend_from_slice(&buf);
        buf.clear();
        t.read(&o, &mut buf)?;
    }
    let mut v = w.unwrap(&mut buf)?;
    let mut n = Packet::default();
    n.read_packet(&mut v)?;
    Ok(n)
}
pub(super) fn write_packet(out: &mut impl Write, w: &mut Wrapper, t: &mut Transform, n: Packet) -> io::Result<()> {
    if w.is_none() && t.is_none() {
        return n.write_packet(out);
    }
    let mut buf = Chunk::new();
    {
        // Inner wrap to drop Box after this.
        let mut v = w.wrap(&mut buf)?;
        n.write_packet(&mut v)?;
    }
    t.write(&buf, out)
}
