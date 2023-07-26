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

use core::cmp;
use core::error::Error;
use core::fmt::{Arguments, Debug, Display};
use core::ops::{Deref, DerefMut};

use crate::com::Flag;
use crate::data::{self, Chunk, Readable, Reader, Writable, Writer};
use crate::device::ID;
use crate::util::stx::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use crate::util::stx::prelude::*;

pub struct Packet {
    pub id:     u8,
    pub job:    u16,
    pub tags:   Vec<u32>,
    pub flags:  Flag,
    pub device: ID,
    data:       Chunk,
}
pub enum PacketAddError {
    Mismatch,
    LimitError,
}

impl Packet {
    pub const MAX_TAGS: usize = 2 << 14;
    pub const HEADER_SIZE: usize = 46;

    #[inline]
    pub fn new() -> Packet {
        Packet {
            id:     0,
            job:    0,
            tags:   Vec::new(),
            data:   Chunk::new(),
            flags:  Flag::new(),
            device: ID::default(),
        }
    }

    #[inline]
    pub fn size(&self) -> usize {
        if self.data.is_empty() {
            return Packet::HEADER_SIZE;
        }
        let s = self.data.size() + Packet::HEADER_SIZE + (4 * self.tags.len());
        s + match s {
            0x0 => 0,
            0x1..=0xFF => 1,
            0x100..=0xFFFF => 2,
            0x10000..=0xFFFFFFFF => 4,
            _ => 8,
        }
    }
    #[inline]
    pub fn belongs(&self, other: &Packet) -> bool {
        self.flags.has(Flag::FRAG) && other.flags.has(Flag::FRAG) && self.id == other.id && self.job == other.job && self.flags.group() == other.flags.group()
    }
    #[inline]
    pub fn add(&mut self, new: Packet) -> Result<(), PacketAddError> {
        if new.data.is_empty() {
            return Ok(());
        }
        if self.id != new.id {
            return Err(PacketAddError::Mismatch);
        }
        if self.data.extend_from_slice(&new.data).is_err() {
            return Err(PacketAddError::LimitError);
        }
        self.flags.0 |= (new.flags.0 as u16) as u64;
        Ok(())
    }
    #[inline]
    pub fn write_packet(&self, w: &mut impl Write) -> io::Result<()> {
        self.write_header(w)?;
        self.write_body(w)
    }
    #[inline]
    pub fn read_packet(&mut self, r: &mut impl Read) -> io::Result<()> {
        let n = self.read_header(r)?;
        self.read_body(n, r)
    }

    fn write_body(&self, w: &mut impl Write) -> io::Result<()> {
        if !self.tags.is_empty() {
            crate::bugtrack!("com::Packet.write_body(): p.tags.len()={}", self.tags.len());
            for (i, t) in self.tags.iter().enumerate() {
                if i > Packet::MAX_TAGS {
                    break;
                }
                if *t == 0 {
                    return Err(ErrorKind::InvalidFilename.into());
                }
                data::write_full(w, &t.to_be_bytes())?;
            }
        }
        if self.data.is_empty() {
            return Ok(());
        }
        let n = io::copy(&mut self.data.deref(), w)? as usize;
        if n != self.data.size() {
            return Err(ErrorKind::WriteZero.into());
        }
        crate::bugtrack!(
            "com::Packet.write_body(): p.Chunk.Size()={}, n={n}",
            self.data.size()
        );
        Ok(())
    }
    fn write_header(&self, w: &mut impl Write) -> io::Result<()> {
        let t = self.tags.len();
        if t > Packet::MAX_TAGS {
            return Err(io::ErrorKind::TooManyLinks.into());
        }
        self.device.write(w)?;
        let mut b = [0u8; 22];
        b[0] = self.id;
        b[1..3].copy_from_slice(&self.job.to_be_bytes());
        b[3..11].copy_from_slice(&self.flags.0.to_be_bytes());
        b[11..13].copy_from_slice(&(t as u16).to_be_bytes());
        let n = self.data.size();
        let c = match n {
            0x0 => {
                b[13] = 0;
                0usize
            },
            0x1..=0xFF => {
                (b[13], b[14]) = (1, n as u8);
                1usize
            },
            0x100..=0xFFFF => {
                b[13] = 3;
                b[14..16].copy_from_slice(&(n as u16).to_be_bytes());
                2usize
            },
            0x10000..=0xFFFFFFFF => {
                b[13] = 5;
                b[14..18].copy_from_slice(&(n as u32).to_be_bytes());
                4usize
            },
            _ => {
                b[13] = 7;
                b[13..22].copy_from_slice(&(n as u64).to_be_bytes());
                8usize
            },
        };
        data::write_full(w, &b[0..14])?;
        data::write_full(w, &b[14..14 + c])?;
        crate::bugtrack!(
            "com::Packet.write_header(): p.id={}, p.len={}, n={}",
            self.id,
            self.data.len(),
            c + 14usize + ID::SIZE as usize
        );
        Ok(())
    }
    fn read_header(&mut self, r: &mut impl Read) -> io::Result<usize> {
        self.device.read(r)?;
        let mut b = [0u8; 14];
        data::read_full(r, &mut b)?;
        crate::bugtrack!("com::Packet.readHeader(): b={:?}", b[0..14]);
        self.id = b[0];
        // SAFETY: All these are safe as we own the buffer and it is larger than
        // the sizes requested, so we'll be fine.
        self.job = u16::from_be_bytes(unsafe { *(b[1..3].as_ptr() as *const [u8; 2]) });
        self.flags.0 = u64::from_be_bytes(unsafe { *(b[3..11].as_ptr() as *const [u8; 8]) });
        let t = u16::from_be_bytes(unsafe { *(b[11..13].as_ptr() as *const [u8; 2]) }) as usize;
        if t > 0 {
            self.tags.resize(t, 0);
        }
        let n = match b[13] {
            0 => 0usize,
            1 => {
                data::read_full(r, &mut b[0..1])?;
                crate::bugtrack!("com::Packet.read_header(): 1, n=1, b=[{:?}]", b[0]);
                b[0] as usize
            },
            3 => {
                data::read_full(r, &mut b[0..2])?;
                crate::bugtrack!("com::Packet.read_header(): 3, n=2, b={:?}", b[0..2]);
                u16::from_be_bytes(unsafe { *(b[0..2].as_ptr() as *const [u8; 2]) }) as usize
            },
            5 => {
                data::read_full(r, &mut b[0..4])?;
                crate::bugtrack!("com::Packet.read_header(): 5, n=4, b={:?}", b[0..4]);
                u32::from_be_bytes(unsafe { *(b[0..4].as_ptr() as *const [u8; 4]) }) as usize
            },
            7 => {
                data::read_full(r, &mut b[0..8])?;
                crate::bugtrack!("com::Packet.read_header(): 7, n=8, b={:?}", b[0..8]);
                u64::from_be_bytes(unsafe { *(b[0..8].as_ptr() as *const [u8; 8]) }) as usize
            },
            _ => return Err(ErrorKind::InvalidData.into()),
        };
        crate::bugtrack!("com::Packet.read_header(): p.ID={}, p.len={n}", self.id);
        Ok(n)
    }
    fn read_body(&mut self, len: usize, r: &mut impl Read) -> io::Result<()> {
        if !self.tags.is_empty() {
            crate::bugtrack!("com::Packet.read_body(): p.tags.len()={}", self.tags.len());
            let mut b = [0u8; 4];
            for i in 0..self.tags.len() {
                data::read_full(r, &mut b)?;
                let v = u32::from_be_bytes(b);
                if v == 0 {
                    return Err(ErrorKind::InvalidFilename.into());
                }
                self.tags[i] = v;
            }
        }
        crate::bugtrack!("com::Packet.read_body(): p.len={len}");
        if len == 0 {
            return Ok(());
        }
        self.data.limit = len;
        let mut n = 0usize;
        while n < len {
            let c = io::copy(r, &mut self.data)? as usize;
            n += c;
            if c == 0 {
                break;
            }
        }
        self.data.limit = 0;
        crate::bugtrack!("com::Packet.read_body(): p.len={len} t={n}");
        if n < len {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }
}

impl Error for PacketAddError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for PacketAddError {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&ErrorKind::InvalidInput.to_string())
    }
}
impl Display for PacketAddError {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&ErrorKind::InvalidInput.to_string())
    }
}

impl Read for Packet {
    #[inline]
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.data.read(b)
    }
}
impl Seek for Packet {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        self.data.stream_len()
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        self.data.stream_position()
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.data.seek(pos)
    }
}
impl Write for Packet {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.write(buf)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.data.write_all(buf)
    }
    #[inline]
    fn write_fmt(&mut self, fmt: Arguments<'_>) -> io::Result<()> {
        self.data.write_fmt(fmt)
    }
}
impl Deref for Packet {
    type Target = Chunk;

    #[inline]
    fn deref(&self) -> &Chunk {
        &self.data
    }
}
impl Clone for Packet {
    #[inline]
    fn clone(&self) -> Packet {
        Packet {
            id:     self.id,
            job:    self.job,
            data:   self.data.clone(),
            tags:   self.tags.clone(),
            flags:  self.flags,
            device: self.device,
        }
    }
}
impl Reader for Packet {}
impl Writer for Packet {}
impl Default for Packet {
    #[inline]
    fn default() -> Packet {
        Packet::new()
    }
}
impl DerefMut for Packet {
    #[inline]
    fn deref_mut(&mut self) -> &mut Chunk {
        &mut self.data
    }
}
impl Readable for Packet {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u8(&mut self.id)?;
        r.read_into_u16(&mut self.job)?;
        let n = r.read_u16()? as usize;
        self.flags.read_stream(r)?;
        self.device.read_stream(r)?;
        if n > 0 {
            self.tags.reserve_exact(n);
            for _ in 0..cmp::min(n, Packet::MAX_TAGS) {
                let v = r.read_u32()?;
                if v == 0 {
                    return Err(ErrorKind::InvalidFilename.into());
                }
                self.tags.push(v);
            }
        }
        self.data.read_stream(r)
    }
}
impl Writable for Packet {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u8(self.id)?;
        w.write_u16(self.job)?;
        w.write_u16(self.tags.len() as u16)?;
        self.flags.write_stream(w)?;
        self.device.write_stream(w)?;
        for (i, t) in self.tags.iter().enumerate() {
            if i > Packet::MAX_TAGS {
                break;
            }
            w.write_u32(*t)?;
        }
        self.data.write_stream(w)
    }
}
impl AsRef<[u8]> for Packet {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.data.as_ref()
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex, Write};

    use crate::com::Packet;
    use crate::util::stx::prelude::*;
    use crate::util::{ToStr, ToStrHex};

    impl Debug for Packet {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Packet {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 21];
            match 0 {
                _ if self.is_empty() && self.flags.is_zero() && self.job == 0 && self.id == 0 => f.write_str("NoP"),
                _ if self.is_empty() && self.flags.is_zero() && self.job == 0 => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))
                },
                _ if self.is_empty() && self.flags.is_zero() && self.id == 0 => {
                    f.write_str("<invalid>NoP/")?;
                    f.write_str(self.job.into_str(&mut b))
                },
                _ if self.is_empty() && self.flags.is_zero() => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char('/')?;
                    f.write_str(self.job.into_str(&mut b))
                },
                _ if self.is_empty() && self.job == 0 && self.id == 0 => Display::fmt(&self.flags, f),
                _ if self.is_empty() && self.job == 0 => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)
                },
                _ if self.is_empty() && self.id == 0 => {
                    f.write_str("NoP/")?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)
                },
                _ if self.is_empty() => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char('/')?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)
                },
                _ if self.flags.is_zero() && self.job == 0 && self.id == 0 => {
                    f.write_str("<invalid>NoP: ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ if self.flags.is_zero() && self.job == 0 => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ if self.flags.is_zero() && self.id == 0 => {
                    f.write_str("<invalid>NoP/")?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ if self.flags.is_zero() => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char('/')?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ if self.job == 0 && self.id == 0 => {
                    Display::fmt(&self.flags, f)?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_str("B")
                },
                _ if self.job == 0 => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ if self.id == 0 => {
                    f.write_str("<invalid>NoP/")?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
                _ => {
                    f.write_str("0x")?;
                    f.write_str(self.id.into_hex_str(&mut b))?;
                    f.write_char('/')?;
                    f.write_str(self.job.into_str(&mut b))?;
                    f.write_char(' ')?;
                    Display::fmt(&self.flags, f)?;
                    f.write_str(": ")?;
                    f.write_str(self.size().into_str(&mut b))?;
                    f.write_char('B')
                },
            }
        }
    }
    impl UpperHex for Packet {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 20];
            f.write_str(self.id.into_hex_str(&mut b))
        }
    }
    impl LowerHex for Packet {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(self, f)
        }
    }
}
