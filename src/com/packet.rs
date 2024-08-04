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
use core::alloc::Allocator;
use core::error::Error;
use core::fmt::{Arguments, Debug, Display, Formatter};
use core::ops::{Deref, DerefMut};
use core::{cmp, fmt};

use crate::com::Flag;
use crate::data::{self, Chunk, Readable, Reader, Writable, Writer};
use crate::device::ID;
use crate::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use crate::prelude::*;

pub enum PacketAddError {
    Mismatch,
    LimitError,
    InvalidCount,
}

pub struct Packet<A: Allocator = Global> {
    pub id:          u8,
    pub job:         u16,
    pub tags:        Vec<u32, A>,
    pub flags:       Flag,
    pub device:      ID,
    pub(crate) data: Chunk<A>,
}

impl Packet {
    pub const MAX_TAGS: usize = 2 << 14;
    pub const HEADER_SIZE: usize = 46usize;

    #[inline]
    pub fn new() -> Packet {
        Packet::new_in(Global)
    }
    #[inline]
    pub fn new_dev(dev: ID) -> Packet {
        Packet::new_id_in(0, dev, Global)
    }
    #[inline]
    pub fn new_id(id: u8, dev: ID) -> Packet {
        Packet::new_id_in(id, dev, Global)
    }
    #[inline]
    pub fn new_job(id: u8, job: u16) -> Packet {
        Packet::new_job_in(id, job, Global)
    }
    #[inline]
    pub fn new_with(id: u8, job: u16, dev: ID) -> Packet {
        Packet::new_with_in(id, job, dev, Global)
    }
    #[inline]
    pub fn from_reader(r: &mut impl Read) -> io::Result<Packet> {
        Packet::from_reader_in(r, Global)
    }
    #[inline]
    pub fn from_stream(r: &mut impl Reader) -> io::Result<Packet> {
        Packet::from_stream_in(r, Global)
    }
}
impl<A: Allocator> Packet<A> {
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
    pub fn belongs(&self, other: &Packet<A>) -> bool {
        self.flags.has(Flag::FRAG) && other.flags.has(Flag::FRAG) && self.id == other.id && self.job == other.job && self.flags.group() == other.flags.group()
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
    #[inline]
    pub fn add(&mut self, new: Packet<A>) -> Result<(), PacketAddError> {
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
    pub fn with_job(mut self, job: u16) -> Packet<A> {
        self.job = job;
        self
    }
    #[inline]
    pub fn with_tags(mut self, tags: &[u32]) -> Packet<A> {
        if tags.len() > 0 {
            self.tags.extend_from_slice(tags);
            self.tags.dedup();
        }
        self
    }
    #[inline]
    pub fn with_flags(mut self, flags: Flag) -> Packet<A> {
        self.flags = flags;
        self
    }

    fn write_body(&self, w: &mut impl Write) -> io::Result<()> {
        if !self.tags.is_empty() {
            bugtrack!("com::Packet.write_body(): p.tags.len()={}", self.tags.len());
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
        bugtrack!(
            "com::Packet.write_body(): p.Chunk.Size()={}, n={n}",
            self.data.size()
        );
        Ok(())
    }
    fn write_header(&self, w: &mut impl Write) -> io::Result<()> {
        let t = self.tags.len();
        if t > Packet::MAX_TAGS {
            return Err(ErrorKind::TooManyLinks.into());
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
        bugtrack!(
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
        bugtrack!("com::Packet.readHeader(): b={:?}", &b[0..14]);
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
                bugtrack!("com::Packet.read_header(): 1, n=1, b=[{:?}]", b[0]);
                b[0] as usize
            },
            3 => {
                data::read_full(r, &mut b[0..2])?;
                bugtrack!("com::Packet.read_header(): 3, n=2, b={:?}", &b[0..2]);
                u16::from_be_bytes(unsafe { *(b[0..2].as_ptr() as *const [u8; 2]) }) as usize
            },
            5 => {
                data::read_full(r, &mut b[0..4])?;
                bugtrack!("com::Packet.read_header(): 5, n=4, b={:?}", &b[0..4]);
                u32::from_be_bytes(unsafe { *(b[0..4].as_ptr() as *const [u8; 4]) }) as usize
            },
            7 => {
                data::read_full(r, &mut b[0..8])?;
                bugtrack!("com::Packet.read_header(): 7, n=8, b={:?}", &b[0..8]);
                u64::from_be_bytes(unsafe { *(b[0..8].as_ptr() as *const [u8; 8]) }) as usize
            },
            _ => return Err(ErrorKind::InvalidData.into()),
        };
        bugtrack!("com::Packet.read_header(): p.ID={}, p.len={n}", self.id);
        Ok(n)
    }
    fn read_body(&mut self, len: usize, r: &mut impl Read) -> io::Result<()> {
        if !self.tags.is_empty() {
            bugtrack!("com::Packet.read_body(): p.tags.len()={}", self.tags.len());
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
        bugtrack!("com::Packet.read_body(): p.len={len}");
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
        bugtrack!("com::Packet.read_body(): p.len={len} t={n}");
        if n < len {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }
}
impl<A: Allocator + Clone> Packet<A> {
    #[inline]
    pub fn new_in(alloc: A) -> Packet<A> {
        Packet {
            id:     0u8,
            job:    0u16,
            tags:   Vec::new_in(alloc.clone()),
            data:   Chunk::new_in(alloc),
            flags:  Flag::new(),
            device: ID::new(),
        }
    }
    #[inline]
    pub fn new_dev_in(dev: ID, alloc: A) -> Packet<A> {
        Packet::new_id_in(0, dev, alloc)
    }
    #[inline]
    pub fn new_id_in(id: u8, dev: ID, alloc: A) -> Packet<A> {
        Packet {
            id,
            job: 0u16,
            tags: Vec::new_in(alloc.clone()),
            data: Chunk::new_in(alloc),
            flags: Flag::new(),
            device: dev,
        }
    }
    #[inline]
    pub fn new_job_in(id: u8, job: u16, alloc: A) -> Packet<A> {
        Packet {
            id,
            job,
            tags: Vec::new_in(alloc.clone()),
            data: Chunk::new_in(alloc),
            flags: Flag::new(),
            device: ID::new(),
        }
    }
    #[inline]
    pub fn new_with_in(id: u8, job: u16, dev: ID, alloc: A) -> Packet<A> {
        Packet {
            id,
            job,
            tags: Vec::new_in(alloc.clone()),
            data: Chunk::new_in(alloc),
            flags: Flag::new(),
            device: dev,
        }
    }
    #[inline]
    pub fn from_reader_in(r: &mut impl Read, alloc: A) -> io::Result<Packet<A>> {
        let mut n = Packet::new_in(alloc);
        n.read_packet(r)?;
        Ok(n)
    }
    #[inline]
    pub fn from_stream_in(r: &mut impl Reader, alloc: A) -> io::Result<Packet<A>> {
        let mut n = Packet::new_in(alloc);
        n.read_stream(r)?;
        Ok(n)
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&ErrorKind::InvalidInput.to_string())
    }
}
impl Display for PacketAddError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&ErrorKind::InvalidInput.to_string())
    }
}

impl Default for Packet {
    #[inline]
    fn default() -> Packet {
        Packet::new()
    }
}
impl<A: Allocator> Read for Packet<A> {
    #[inline]
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.data.read(b)
    }
}
impl<A: Allocator> Seek for Packet<A> {
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
impl<A: Allocator> Write for Packet<A> {
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
impl<A: Allocator> Deref for Packet<A> {
    type Target = Chunk<A>;

    #[inline]
    fn deref(&self) -> &Chunk<A> {
        &self.data
    }
}
impl<A: Allocator> Reader for Packet<A> {}
impl<A: Allocator> Writer for Packet<A> {}
impl<A: Allocator> DerefMut for Packet<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Chunk<A> {
        &mut self.data
    }
}
impl<A: Allocator> Readable for Packet<A> {
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
impl<A: Allocator> Writable for Packet<A> {
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
impl<A: Allocator> AsRef<[u8]> for Packet<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.data.as_ref()
    }
}
impl<A: Allocator + Copy + Clone> Clone for Packet<A> {
    #[inline]
    fn clone(&self) -> Packet<A> {
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

#[cfg(not(feature = "strip"))]
mod display {
    use core::alloc::Allocator;
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex, Write};

    use crate::com::Packet;
    use crate::prelude::*;
    use crate::util::{ToStr, ToStrHex};

    impl<A: Allocator> Debug for Packet<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl<A: Allocator> Display for Packet<A> {
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
    impl<A: Allocator> UpperHex for Packet<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0u8; 20];
            f.write_str(self.id.into_hex_str(&mut b))
        }
    }
    impl<A: Allocator> LowerHex for Packet<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(self, f)
        }
    }
}
