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
use core::cmp;
use core::ops::{Deref, DerefMut};
use core::str::from_utf8_unchecked;

use crate::com::Packet;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::io::{self, Error, ErrorKind, Read, Seek, SeekFrom, Write};
use crate::prelude::*;
use crate::util::crypt;

pub struct Chunk<A: Allocator = Global> {
    pub limit: usize,
    buf:       Vec<u8, A>,
    rpos:      usize,
}

impl Chunk {
    #[inline]
    pub const fn new() -> Chunk {
        Chunk {
            buf:   Vec::new(),
            rpos:  0usize,
            limit: 0usize,
        }
    }
}
impl<A: Allocator> Chunk<A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Chunk<A> {
        Chunk {
            buf:   Vec::new_in(alloc),
            rpos:  0usize,
            limit: 0usize,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.buf.clear();
        self.rpos = 0;
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len() - self.rpos
    }
    #[inline]
    pub fn size(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    pub fn space(&self) -> usize {
        if self.limit == 0 {
            return usize::MAX;
        }
        let r = self.limit - self.len() as usize;
        if r > 0 {
            r
        } else {
            0
        }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[self.rpos..]
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.buf.shrink_to_fit()
    }
    #[inline]
    pub fn remaining(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            self.len() - self.rpos
        }
    }
    #[inline]
    pub fn push(&mut self, value: u8) {
        self.buf.push(value)
    }
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }
    #[inline]
    pub fn grow(&mut self, len: usize) {
        self.buf.reserve(len - self.len())
    }
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.buf.truncate(len)
    }
    #[inline]
    pub fn extend(&mut self, other: &[u8]) {
        self.buf.extend_from_slice(other)
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buf.as_mut_ptr()
    }
    #[inline]
    pub fn avaliable(&self, n: usize) -> bool {
        if self.limit == 0 {
            true
        } else {
            self.buf.len() + n <= self.limit as usize
        }
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf[self.rpos..]
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional)
    }
    #[inline]
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8, A> {
        &mut self.buf
    }
    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.buf.shrink_to(min_capacity)
    }
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.buf.reserve_exact(additional)
    }
    #[inline]
    pub fn try_extend_slice(&mut self, other: &[u8]) -> Option<usize> {
        let r = self.space();
        if r == 0 {
            return None;
        }
        let t = cmp::max(other.len(), r);
        self.buf.extend_from_slice(&other[0..t]);
        Some(t)
    }
    #[inline]
    pub fn write_u8_at(&mut self, pos: usize, v: u8) -> io::Result<()> {
        if pos >= self.size() || pos + 1 >= self.size() {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.buf[pos] = v;
        Ok(())
    }
    #[inline]
    pub fn extend_from_slice(&mut self, other: &[u8]) -> io::Result<()> {
        self.check_avaliable(other.len())?;
        self.buf.extend_from_slice(other);
        Ok(())
    }
    #[inline]
    pub fn write_u16_at(&mut self, pos: usize, v: u16) -> io::Result<()> {
        if pos >= self.size() || pos + 2 >= self.size() {
            return Err(ErrorKind::InvalidInput.into());
        }
        let b = v.to_be_bytes();
        self.buf[pos..pos + 2].copy_from_slice(&b);
        Ok(())
    }
    #[inline]
    pub fn write_u32_at(&mut self, pos: usize, v: u32) -> io::Result<()> {
        if pos >= self.size() || pos + 4 >= self.size() {
            return Err(ErrorKind::InvalidInput.into());
        }
        let b = v.to_be_bytes();
        self.buf[pos..pos + 4].copy_from_slice(&b);
        Ok(())
    }
    #[inline]
    pub fn write_u64_at(&mut self, pos: usize, v: u64) -> io::Result<()> {
        if pos >= self.size() || pos + 8 >= self.size() {
            return Err(ErrorKind::InvalidInput.into());
        }
        let b = v.to_be_bytes();
        self.buf[pos..pos + 8].copy_from_slice(&b);
        Ok(())
    }
    #[inline]
    pub fn resize(&mut self, new_len: usize, value: u8) -> io::Result<()> {
        self.check_avaliable(new_len)?;
        self.buf.resize(new_len, value);
        Ok(())
    }
    #[inline]
    pub fn append(&mut self, other: &mut impl AsMut<Vec<u8, A>>) -> io::Result<()> {
        let v = other.as_mut();
        self.check_avaliable(v.len())?;
        self.buf.append(v);
        Ok(())
    }

    pub fn read_str_ptr(&mut self) -> io::Result<Option<&str>> {
        let n = match self.read_u8()? {
            0 => return Ok(None),
            1 | 2 => self.read_u8()? as usize,
            3 | 4 => self.read_u16()? as usize,
            5 | 6 => self.read_u32()? as usize,
            7 | 8 => self.read_u64()? as usize,
            _ => return Err(ErrorKind::InvalidData.into()),
        };
        if (n as isize) <= 0 || (n as isize) >= isize::MAX {
            return Err(ErrorKind::FileTooLarge.into());
        }
        if self.len() < n {
            return Err(ErrorKind::WouldBlock.into());
        }
        let b = &self.buf[self.rpos..self.rpos + n];
        self.rpos += n;
        Ok(Some(unsafe { from_utf8_unchecked(b) }))
    }

    #[inline]
    fn check_avaliable(&self, n: usize) -> io::Result<()> {
        if self.avaliable(n) {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedEof,
                crypt::get_or(0, "limit reached"),
            ))
        }
    }
}

impl Default for Chunk {
    #[inline]
    fn default() -> Chunk {
        Chunk {
            buf:   Vec::new(),
            rpos:  0usize,
            limit: 0usize,
        }
    }
}
impl From<&[u8]> for Chunk {
    #[inline]
    fn from(v: &[u8]) -> Chunk {
        let mut c = Chunk::new();
        c.extend(v);
        c
    }
}
impl<A: Allocator> Drop for Chunk<A> {
    #[inline]
    fn drop(&mut self) {
        self.buf.shrink_to(0)
    }
}
impl<A: Allocator> Read for Chunk<A> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buf.is_empty() {
            return Ok(0);
        }
        let n = cmp::min(buf.len(), self.len());
        buf[0..n].copy_from_slice(&self.buf[self.rpos..self.rpos + n]);
        self.rpos += n;
        Ok(n)
    }
}
impl<A: Allocator> Seek for Chunk<A> {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(self.buf.len() as u64)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.rpos as u64)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let n = match pos {
            SeekFrom::End(v) => (self.buf.len() as i64 + v) as u64,
            SeekFrom::Start(v) => v,
            SeekFrom::Current(v) => (self.rpos as i64 + v) as u64,
        };
        if n > self.buf.len() as u64 {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.rpos = n as usize;
        Ok(n)
    }
}
impl<A: Allocator> Write for Chunk<A> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len();
        if n == 0 {
            return Ok(0);
        }
        self.check_avaliable(n)?;
        self.extend_from_slice(buf)?;
        Ok(n)
    }
}
impl<A: Allocator> Deref for Chunk<A> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.buf
    }
}
impl<A: Allocator> Reader for Chunk<A> {
    #[inline]
    fn read_i8(&mut self) -> io::Result<i8> {
        if self.buf.is_empty() || self.len() <= 1 {
            return Ok(0);
        }
        let v = self.buf[self.rpos];
        self.rpos += 1;
        Ok(v as i8)
    }
    #[inline]
    fn read_u8(&mut self) -> io::Result<u8> {
        if self.buf.is_empty() || self.len() <= 1 {
            return Ok(0);
        }
        let v = self.buf[self.rpos];
        self.rpos += 1;
        Ok(v)
    }
}
impl<A: Allocator> Writer for Chunk<A> {
    #[inline]
    fn write_i8(&mut self, v: i8) -> io::Result<()> {
        self.buf.push(v as u8);
        Ok(())
    }
    fn write_u8(&mut self, v: u8) -> io::Result<()> {
        self.buf.push(v);
        Ok(())
    }
}
impl<A: Allocator> Writable for Chunk<A> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_bytes(&self.buf[self.rpos..])
    }
}
impl<A: Allocator> Readable for Chunk<A> {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_vec(&mut self.buf)?;
        self.rpos = 0;
        Ok(())
    }
}
impl<A: Allocator> DerefMut for Chunk<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}
impl<A: Allocator> AsRef<[u8]> for Chunk<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.buf
    }
}
impl<A: Allocator> From<Packet<A>> for Chunk<A> {
    #[inline]
    fn from(v: Packet<A>) -> Chunk<A> {
        v.data
    }
}
impl<A: Allocator> From<Vec<u8, A>> for Chunk<A> {
    #[inline]
    fn from(v: Vec<u8, A>) -> Chunk<A> {
        Chunk {
            buf:   v,
            rpos:  0usize,
            limit: 0usize,
        }
    }
}
impl<A: Allocator> AsRef<Vec<u8, A>> for Chunk<A> {
    #[inline]
    fn as_ref(&self) -> &Vec<u8, A> {
        &self.buf
    }
}
impl<A: Allocator> AsMut<Vec<u8, A>> for Chunk<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut Vec<u8, A> {
        &mut self.buf
    }
}
impl<A: Allocator + Copy + Clone> Clone for Chunk<A> {
    #[inline]
    fn clone(&self) -> Chunk<A> {
        Chunk {
            buf:   self.buf.clone(),
            rpos:  0,
            limit: self.limit,
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::alloc::Allocator;
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::data::Chunk;
    use crate::prelude::*;

    impl<A: Allocator> Debug for Chunk<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Chunk")
                .field("limit", &self.limit)
                .field("buf", &self.buf)
                .field("rpos", &self.rpos)
                .finish()
        }
    }
    impl<A: Allocator> Display for Chunk<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(&self.buf, f)
        }
    }
}
