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

use alloc::alloc::Allocator;
use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::{AsRef, From};
use core::marker::Sized;
use core::ops::{AsyncFnOnce, Drop};
use core::option::Option::{None, Some};
use core::result::Result::{Err, Ok};
use core::slice::memchr::memchr;
use core::str::from_utf8;

use crate::link;
use crate::stxa::io::{AsyncSeek, BorrowedBuf, BorrowedCursor, BufRead, Cursor, Empty, ErrorKind, IoError, IoResult, Read, Repeat, Seek, SeekFrom};

pub struct BlockingRead<T: AsyncRead>(T);
pub struct BlockingReadRef<'a, T: AsyncRead>(&'a mut T);

pub trait AsyncRead {
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize>;

    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }
    #[inline]
    fn blocking(self) -> BlockingRead<Self>
    where Self: Sized {
        BlockingRead(self)
    }
    #[inline]
    fn by_ref_blocking(&mut self) -> BlockingReadRef<'_, Self>
    where Self: Sized {
        BlockingReadRef(self)
    }

    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        read_exact(self, buf).await
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        read_to_end(self, buf).await
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        Guard::new(buf).exec(async |v| read_to_end(self, v).await).await
    }
    #[inline]
    async fn async_read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> IoResult<()> {
        let n = self.async_read(buf.ensure_init().init_mut()).await?;
        buf.advance(n);
        Ok(())
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        read_buf_exact(self, cur).await
    }
}
pub trait AsyncBufRead: AsyncRead {
    async fn async_consume(&mut self, n: usize);
    async fn async_fill_buf(&mut self) -> IoResult<&[u8]>;

    #[inline]
    async fn async_has_data_left(&mut self) -> IoResult<bool> {
        self.async_fill_buf().await.map(|v| !v.is_empty())
    }
    #[inline]
    async fn async_skip_until(&mut self, byte: u8) -> IoResult<usize> {
        skip(self, byte).await
    }
    #[inline]
    async fn async_read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        Guard::new(buf).exec(async |v| read(self, 0xA, v).await).await
    }
    #[inline]
    async fn async_read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        read(self, byte, buf).await
    }
}

struct Guard<'a> {
    b: &'a mut Vec<u8>,
    n: usize,
}

impl<'a> Guard<'a> {
    #[inline]
    fn new(b: &'a mut String) -> Guard<'a> {
        Guard {
            n: b.len(),
            b: unsafe { b.as_mut_vec() },
        }
    }

    #[inline]
    async fn exec<F: AsyncFnOnce(&mut Vec<u8>) -> IoResult<usize>>(mut self, f: F) -> IoResult<usize> {
        let r = f(self.b).await;
        if from_utf8(unsafe { self.b.get_unchecked(self.n..) }).is_err() {
            r.and_then(|_| Err(IoError::from(ErrorKind::InvalidData)))
        } else {
            self.n = self.b.len();
            r
        }
    }
}

impl Drop for Guard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.b.set_len(self.n) }
    }
}

impl AsyncRead for &[u8] {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read(buf)
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        self.read_exact(buf)
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        self.read_to_end(buf)
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf(cur)
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        self.read_to_string(buf)
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf_exact(cur)
    }
}
impl AsyncRead for Empty {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read(buf)
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        self.read_exact(buf)
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        self.read_to_end(buf)
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf(cur)
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        self.read_to_string(buf)
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf_exact(cur)
    }
}
impl AsyncRead for Repeat {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read(buf)
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        self.read_exact(buf)
    }
    #[inline]
    async fn async_read_to_end(&mut self, _: &mut Vec<u8>) -> IoResult<usize> {
        Err(IoError::from(ErrorKind::OutOfMemory))
    }
    #[inline]
    async fn async_read_buf(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf(buf)
    }
    #[inline]
    async fn async_read_to_string(&mut self, _: &mut String) -> IoResult<usize> {
        Err(IoError::from(ErrorKind::OutOfMemory))
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf_exact(buf)
    }
}

impl<T: AsRef<[u8]>> AsyncSeek for Cursor<T> {
    #[inline]
    async fn async_stream_len(&mut self) -> IoResult<u64> {
        self.stream_len()
    }
    #[inline]
    async fn async_stream_position(&mut self) -> IoResult<u64> {
        self.stream_position()
    }
    #[inline]
    async fn async_seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        self.seek(pos)
    }
}
impl<T: AsRef<[u8]>> AsyncRead for Cursor<T> {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read(buf)
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        self.read_exact(buf)
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        self.read_to_end(buf)
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf(cur)
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        self.read_to_string(buf)
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf_exact(cur)
    }
}

impl<A: Allocator> AsyncRead for VecDeque<u8, A> {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read(buf)
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        self.read_exact(buf)
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        self.read_to_end(buf)
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf(cur)
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        self.read_to_string(buf)
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        self.read_buf_exact(cur)
    }
}
impl<A: Allocator> AsyncBufRead for VecDeque<u8, A> {
    #[inline]
    async fn async_consume(&mut self, n: usize) {
        self.drain(0..n);
    }
    #[inline]
    async fn async_fill_buf(&mut self) -> IoResult<&[u8]> {
        let (f, _) = self.as_slices();
        Ok(f)
    }
}
impl<A: Allocator, T: ?Sized + AsyncRead> AsyncRead for Box<T, A> {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (**self).async_read(buf).await
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        (**self).async_read_exact(buf).await
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).async_read_to_end(buf).await
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).async_read_buf(cur).await
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).async_read_to_string(buf).await
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).async_read_buf_exact(cur).await
    }
}
impl<A: Allocator, T: ?Sized + AsyncSeek> AsyncSeek for Box<T, A> {
    #[inline]
    async fn async_rewind(&mut self) -> IoResult<()> {
        (**self).async_rewind().await
    }
    #[inline]
    async fn async_stream_len(&mut self) -> IoResult<u64> {
        (**self).async_stream_len().await
    }
    #[inline]
    async fn async_stream_position(&mut self) -> IoResult<u64> {
        (**self).async_stream_position().await
    }
    #[inline]
    async fn async_seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        (**self).async_seek(pos).await
    }
    #[inline]
    async fn async_seek_relative(&mut self, offset: i64) -> IoResult<()> {
        (**self).async_seek_relative(offset).await
    }
}
impl<A: Allocator, T: ?Sized + AsyncBufRead> AsyncBufRead for Box<T, A> {
    #[inline]
    async fn async_consume(&mut self, amt: usize) {
        (**self).async_consume(amt).await
    }
    #[inline]
    async fn async_fill_buf(&mut self) -> IoResult<&[u8]> {
        (**self).async_fill_buf().await
    }
    #[inline]
    async fn async_has_data_left(&mut self) -> IoResult<bool> {
        (**self).async_has_data_left().await
    }
    #[inline]
    async fn async_skip_until(&mut self, byte: u8) -> IoResult<usize> {
        (**self).async_skip_until(byte).await
    }
    #[inline]
    async fn async_read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).async_read_line(buf).await
    }
    #[inline]
    async fn async_read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).async_read_until(byte, buf).await
    }
}

impl<T: ?Sized + AsyncRead> AsyncRead for &mut T {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (**self).async_read(buf).await
    }
    #[inline]
    async fn async_read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        (**self).async_read_exact(buf).await
    }
    #[inline]
    async fn async_read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).async_read_to_end(buf).await
    }
    #[inline]
    async fn async_read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).async_read_buf(cur).await
    }
    #[inline]
    async fn async_read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).async_read_to_string(buf).await
    }
    #[inline]
    async fn async_read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).async_read_buf_exact(cur).await
    }
}

impl<T: AsyncRead> Read for BlockingRead<T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        link(self.0.async_read(buf))
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        link(self.0.async_read_exact(buf))
    }
    #[inline]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        link(self.0.async_read_buf(buf))
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        link(self.0.async_read_to_end(buf))
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        link(self.0.async_read_to_string(buf))
    }
    #[inline]
    fn read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        link(self.0.async_read_buf_exact(cur))
    }
}
impl<T: AsyncBufRead> BufRead for BlockingRead<T> {
    #[inline]
    fn consume(&mut self, n: usize) {
        link(self.0.async_consume(n))
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        link(self.0.async_fill_buf())
    }
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        link(self.0.async_has_data_left())
    }
    #[inline]
    fn skip_until(&mut self, byte: u8) -> IoResult<usize> {
        link(self.0.async_skip_until(byte))
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        link(self.0.async_read_line(buf))
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        link(self.0.async_read_until(byte, buf))
    }
}
impl<T: AsyncRead + AsyncSeek> Seek for BlockingRead<T> {
    #[inline]
    fn rewind(&mut self) -> IoResult<()> {
        link(self.0.async_rewind())
    }
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        link(self.0.async_stream_len())
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        link(self.0.async_stream_position())
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        link(self.0.async_seek(pos))
    }
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        link(self.0.async_seek_relative(offset))
    }
}

impl<T: AsyncRead> Read for BlockingReadRef<'_, T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        link(self.0.async_read(buf))
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        link(self.0.async_read_exact(buf))
    }
    #[inline]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> IoResult<()> {
        link(self.0.async_read_buf(buf))
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        link(self.0.async_read_to_end(buf))
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        link(self.0.async_read_to_string(buf))
    }
    #[inline]
    fn read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        link(self.0.async_read_buf_exact(cur))
    }
}
impl<T: AsyncBufRead> BufRead for BlockingReadRef<'_, T> {
    #[inline]
    fn consume(&mut self, n: usize) {
        link(self.0.async_consume(n))
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        link(self.0.async_fill_buf())
    }
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        link(self.0.async_has_data_left())
    }
    #[inline]
    fn skip_until(&mut self, byte: u8) -> IoResult<usize> {
        link(self.0.async_skip_until(byte))
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        link(self.0.async_read_line(buf))
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        link(self.0.async_read_until(byte, buf))
    }
}
impl<T: AsyncRead + AsyncSeek> Seek for BlockingReadRef<'_, T> {
    #[inline]
    fn rewind(&mut self) -> IoResult<()> {
        link(self.0.async_rewind())
    }
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        link(self.0.async_stream_len())
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        link(self.0.async_stream_position())
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        link(self.0.async_seek(pos))
    }
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        link(self.0.async_seek_relative(offset))
    }
}

async fn skip<T: ?Sized + AsyncBufRead>(r: &mut T, delim: u8) -> IoResult<usize> {
    let mut v = 0usize;
    loop {
        let (d, c) = {
            let a = match r.async_fill_buf().await {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr(delim, a) {
                Some(i) => (true, i + 1),
                None => (false, a.len()),
            }
        };
        r.async_consume(c).await;
        v += c;
        if d || c == 0 {
            return Ok(v);
        }
    }
}
async fn read_exact<T: ?Sized + AsyncRead>(r: &mut T, mut b: &mut [u8]) -> IoResult<()> {
    while !b.is_empty() {
        match r.async_read(b).await {
            Ok(0) => break,
            Ok(n) => b = &mut b[n..],
            Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }
    if !b.is_empty() {
        return Err(IoError::from(ErrorKind::UnexpectedEof));
    }
    Ok(())
}
async fn read_to_end<T: ?Sized + AsyncRead>(r: &mut T, b: &mut Vec<u8>) -> IoResult<usize> {
    let (s, c, mut i) = (b.len(), b.capacity(), 0usize);
    loop {
        if b.len() == b.capacity() {
            b.reserve(0x20)
        }
        let mut z = BorrowedBuf::from(b.spare_capacity_mut());
        unsafe { z.set_init(i) };
        let mut v = z.unfilled();
        match r.async_read_buf(v.reborrow()).await {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if v.written() == 0 {
            return Ok(unsafe { b.len().unchecked_sub(s) });
        }
        i = v.init_mut().len();
        let n = z.filled().len();
        unsafe { b.set_len(n + b.len()) };
        if b.len() == b.capacity() && b.capacity() == c {
            let mut t = [0u8; 32];
            loop {
                match r.async_read(&mut t).await {
                    Ok(0) => return Ok(unsafe { b.len().unchecked_sub(s) }),
                    Ok(n) => {
                        b.extend_from_slice(unsafe { t.get_unchecked(0..n) });
                        break;
                    },
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
    }
}
async fn read<T: ?Sized + AsyncBufRead>(r: &mut T, delim: u8, b: &mut Vec<u8>) -> IoResult<usize> {
    let mut v = 0usize;
    loop {
        let (d, c) = {
            let a = match r.async_fill_buf().await {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr(delim, a) {
                Some(i) => {
                    b.extend_from_slice(unsafe { a.get_unchecked(0..=i) });
                    (true, i + 1)
                },
                None => {
                    b.extend_from_slice(a);
                    (false, a.len())
                },
            }
        };
        r.async_consume(c).await;
        v += c;
        if d || c == 0 {
            return Ok(v);
        }
    }
}
async fn read_buf_exact<T: ?Sized + AsyncRead>(r: &mut T, mut cur: BorrowedCursor<'_>) -> IoResult<()> {
    while cur.capacity() > 0 {
        let p = cur.written();
        match r.async_read_buf(cur.reborrow()).await {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if cur.written() == p {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
    }
    Ok(())
}
