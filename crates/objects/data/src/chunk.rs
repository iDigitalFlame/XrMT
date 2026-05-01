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

extern crate alloc;
extern crate core;

extern crate xrmt_io;

use alloc::alloc::Global;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::clone::Clone;
use core::cmp::Ord;
use core::convert::{AsMut, AsRef, From};
use core::default::Default;
use core::hint::{likely, unlikely};
use core::mem::transmute;
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::copy_nonoverlapping;
use core::result::Result::{Err, Ok};
use core::usize;

use xrmt_io::{ErrorKind, IoError, IoResult, Read, Seek, SeekFrom, Write};

use crate::{read_size, Readable, Reader, Writable, Writer};

pub struct Chunk<A: Allocator = Global> {
    buf:   Vec<u8, A>,
    pos:   usize,
    limit: usize,
}

impl Chunk {
    #[inline]
    pub const fn new() -> Chunk {
        Chunk {
            buf:   Vec::new(),
            pos:   0usize,
            limit: usize::MAX,
        }
    }
    #[inline]
    pub const fn with_limit(limit: usize) -> Chunk {
        Chunk::with_limit_in(limit, Global)
    }
}
impl<A: Allocator> Chunk<A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Chunk<A> {
        Chunk {
            buf:   Vec::new_in(alloc),
            pos:   0usize,
            limit: usize::MAX,
        }
    }
    #[inline]
    pub const fn with_limit_in(limit: usize, alloc: A) -> Chunk<A> {
        Chunk {
            buf: Vec::new_in(alloc),
            pos: 0usize,
            limit,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.buf.clear();
        self.pos = 0;
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    pub fn space(&self) -> usize {
        if self.limit == usize::MAX {
            usize::MAX
        } else {
            self.limit.saturating_sub(self.buf.len() as usize)
        }
    }
    #[inline]
    pub fn push(&mut self, v: u8) {
        self.buf.push(v)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: 'pos' cannot be larger than buf.len()
        unsafe { self.buf.get_unchecked(self.pos..) }
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
            self.buf.len().saturating_sub(self.pos)
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        if self.limit == usize::MAX {
            None
        } else {
            Some(self.limit)
        }
    }
    #[inline]
    pub fn reserve(&mut self, len: usize) {
        self.buf.reserve(len)
    }
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        if len > self.pos {
            self.pos = len;
        }
        self.buf.truncate(len);
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buf.as_mut_ptr()
    }
    #[inline]
    pub fn shrink_to(&mut self, len: usize) {
        self.buf.shrink_to(len)
    }
    #[inline]
    pub fn avaliable(&self, n: usize) -> bool {
        if self.limit == usize::MAX {
            true
        } else {
            self.buf.len() + n <= self.limit as usize
        }
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: 'pos' cannot be larger than buf.len()
        unsafe { self.buf.get_unchecked_mut(self.pos..) }
    }
    #[inline]
    pub fn reserve_exact(&mut self, len: usize) {
        self.buf.reserve_exact(len)
    }
    #[inline]
    pub fn set_limit(&mut self, limit: Option<usize>) {
        self.limit = limit.unwrap_or(usize::MAX)
    }
    #[inline]
    pub fn resize(&mut self, len: usize, v: u8) -> IoResult<()> {
        let _ = self.check(len)?;
        self.buf.resize(len, v);
        Ok(())
    }
    #[inline]
    pub fn extend_from_slice(&mut self, other: &[u8]) -> IoResult<()> {
        let _ = self.check(other.len())?;
        self.buf.extend_from_slice(other);
        Ok(())
    }
    #[inline]
    pub fn try_extend_slice(&mut self, other: &[u8]) -> Option<usize> {
        let n = match self.space() {
            0 => return None,
            usize::MAX => other.len(),
            v => other.len().min(v),
        };
        // Bounds checked above
        self.buf.extend_from_slice(unsafe { other.get_unchecked(0..n) });
        Some(n)
    }
    #[inline]
    pub fn append(&mut self, other: &mut impl AsMut<[u8]>) -> IoResult<()> {
        let v = other.as_mut();
        if self.try_extend_slice(v).is_none() {
            return Err(IoError::from(ErrorKind::QuotaExceeded));
        }
        Ok(())
    }

    pub fn read_str_ptr(&mut self) -> IoResult<&str> {
        let n = match read_size(self)? {
            Some(v) => v,
            None => return Ok(unsafe { transmute(self.buf.get_unchecked(0..0)) }),
        };
        if self.len() < n {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        // Bounds checked already.
        let b = unsafe { self.buf.get_unchecked(self.pos..self.pos + n) };
        self.pos += n;
        Ok(unsafe { transmute(b) })
    }
    pub fn read_maybe_str_ptr(&mut self) -> IoResult<Option<&str>> {
        let n = match read_size(self)? {
            Some(v) => v,
            None => return Ok(None),
        };
        if self.len() < n {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        // Bounds checked already.
        let b = unsafe { self.buf.get_unchecked(self.pos..self.pos + n) };
        self.pos += n;
        Ok(Some(unsafe { transmute(b) }))
    }

    #[inline]
    pub fn write_u8_at(&mut self, pos: usize, v: u8) -> IoResult<()> {
        if pos + 1 >= self.len() {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        // Bounds checked above.
        unsafe { *self.buf.get_unchecked_mut(pos) = v };
        Ok(())
    }
    #[inline]
    pub fn write_u16_at(&mut self, pos: usize, v: u16) -> IoResult<()> {
        if pos + 2 >= self.len() {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        let b = v.to_be_bytes();
        // Bounds checked above.
        unsafe { copy_nonoverlapping(b.as_ptr(), self.buf.as_mut_ptr().add(pos), 2) };
        Ok(())
    }
    #[inline]
    pub fn write_u32_at(&mut self, pos: usize, v: u32) -> IoResult<()> {
        if pos + 4 >= self.len() {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        let b = v.to_be_bytes();
        // Bounds checked above.
        unsafe { copy_nonoverlapping(b.as_ptr(), self.buf.as_mut_ptr().add(pos), 4) };
        Ok(())
    }
    #[inline]
    pub fn write_u64_at(&mut self, pos: usize, v: u64) -> IoResult<()> {
        if pos + 8 >= self.len() {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        let b = v.to_be_bytes();
        // Bounds checked above.
        unsafe { copy_nonoverlapping(b.as_ptr(), self.buf.as_mut_ptr().add(pos), 8) };
        Ok(())
    }

    // #[inline] TODO(dij): Do we need this?
    // pub unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8, A> {
    //    // This could be modified to change the pos pointer.
    //     &mut self.buf
    // }

    #[inline]
    fn check(&self, n: usize) -> IoResult<()> {
        if likely(self.avaliable(n)) {
            Ok(())
        } else {
            Err(IoError::from(ErrorKind::QuotaExceeded))
        }
    }
}

impl Default for Chunk {
    #[inline]
    fn default() -> Chunk {
        Chunk::new()
    }
}
impl From<&str> for Chunk {
    #[inline]
    fn from(v: &str) -> Chunk {
        Chunk::from(v.as_bytes())
    }
}
impl From<&[u8]> for Chunk {
    #[inline]
    fn from(v: &[u8]) -> Chunk {
        let mut c = Chunk::new();
        c.buf.extend_from_slice(v);
        c
    }
}
impl From<String> for Chunk {
    #[inline]
    fn from(v: String) -> Chunk {
        Chunk {
            buf:   v.into_bytes(),
            pos:   0,
            limit: usize::MAX,
        }
    }
}
impl From<&String> for Chunk {
    #[inline]
    fn from(v: &String) -> Chunk {
        Chunk::from(v.as_bytes())
    }
}
impl<A: Allocator> Drop for Chunk<A> {
    #[inline]
    fn drop(&mut self) {
        self.buf.clear()
    }
}
impl<A: Allocator> Read for Chunk<A> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.buf.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(self.remaining());
        unsafe { copy_nonoverlapping(self.buf.as_ptr().add(self.pos), buf.as_mut_ptr(), n) };
        self.pos += n;
        Ok(n)
    }
}
impl<A: Allocator> Seek for Chunk<A> {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        Ok(self.buf.len() as u64)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        Ok(self.pos as u64)
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let n = match pos {
            SeekFrom::End(v) => (self.buf.len() as i64 + v) as usize,
            SeekFrom::Start(v) => v as usize,
            SeekFrom::Current(v) => (self.pos as i64 + v) as usize,
        };
        if n > self.buf.len() {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        self.pos = n;
        Ok(n as u64)
    }
}
impl<A: Allocator> Write for Chunk<A> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = buf.len();
        if n == 0 {
            return Ok(0);
        }
        if self.limit != usize::MAX {
            return match self.try_extend_slice(buf) {
                Some(v) => Ok(v),
                None => Err(IoError::from(ErrorKind::QuotaExceeded)),
            };
        }
        self.buf.extend_from_slice(buf);
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
    fn read_u8(&mut self) -> IoResult<u8> {
        if self.buf.is_empty() {
            return Ok(0);
        }
        // Bounds checked above
        let v = unsafe { *self.buf.get_unchecked(self.pos) };
        self.pos += 1;
        Ok(v)
    }
    #[inline]
    fn read_u16(&mut self) -> IoResult<u16> {
        if self.buf.is_empty() || self.remaining() < 2 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        // Bounds checked above
        let v = u16::from_be_bytes(unsafe { *(self.buf.get_unchecked(self.pos..self.pos + 2).as_ptr().cast::<[u8; 2]>()) });
        self.pos += 2;
        Ok(v)
    }
    #[inline]
    fn read_u32(&mut self) -> IoResult<u32> {
        if self.buf.is_empty() || self.remaining() < 4 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        // Bounds checked above
        let v = u32::from_be_bytes(unsafe { *(self.buf.get_unchecked(self.pos..self.pos + 4).as_ptr().cast::<[u8; 4]>()) });
        self.pos += 4;
        Ok(v)
    }
    #[inline]
    fn read_u64(&mut self) -> IoResult<u64> {
        if self.buf.is_empty() || self.remaining() < 8 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        // Bounds checked above
        let v = u64::from_be_bytes(unsafe { *(self.buf.get_unchecked(self.pos..self.pos + 8).as_ptr().cast::<[u8; 8]>()) });
        self.pos += 8;
        Ok(v)
    }
}
impl<A: Allocator> Writer for Chunk<A> {
    #[inline]
    fn write_u8(&mut self, v: u8) -> IoResult<()> {
        if unlikely(!self.avaliable(1)) {
            return Err(IoError::from(ErrorKind::QuotaExceeded));
        }
        self.buf.push(v as u8);
        Ok(())
    }
}
impl<A: Allocator> Writable for Chunk<A> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> IoResult<()> {
        w.write_bytes(&self.buf[self.pos..])
    }
}
impl<A: Allocator> Readable for Chunk<A> {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> IoResult<()> {
        r.read_into_vec(&mut self.buf)?;
        self.pos = 0;
        Ok(())
    }
}
impl<A: Allocator> DerefMut for Chunk<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}
impl<A: Allocator> AsMut<[u8]> for Chunk<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}
impl<A: Allocator> AsRef<[u8]> for Chunk<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.buf
    }
}
impl<A: Allocator> From<Vec<u8, A>> for Chunk<A> {
    #[inline]
    fn from(v: Vec<u8, A>) -> Chunk<A> {
        Chunk {
            buf:   v,
            pos:   0usize,
            limit: 0usize,
        }
    }
}
impl<A: Allocator + Clone> Clone for Chunk<A> {
    #[inline]
    fn clone(&self) -> Chunk<A> {
        Chunk {
            buf:   self.buf.clone(),
            pos:   0,
            limit: self.limit,
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::alloc::Allocator;
    use core::fmt::{Debug, Display, Formatter, Result};

    use crate::Chunk;

    impl<A: Allocator> Debug for Chunk<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("Chunk")
                .field("limit", &self.limit)
                .field("buf", &self.buf)
                .field("pos", &self.pos)
                .finish()
        }
    }
    impl<A: Allocator> Display for Chunk<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.buf, f)
        }
    }
}
