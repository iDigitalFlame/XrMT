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
use alloc::vec::Vec;
use core::convert::From;
use core::marker::Sized;
use core::result::Result::{Err, Ok};

use crate::link;
use crate::stxa::io::{AsyncSeek, BorrowedCursor, Cursor, ErrorKind, IoError, IoResult, Seek, SeekFrom, Sink, Write};

pub struct BlockingWrite<T: AsyncWrite>(T);
pub struct BlockingWriteRef<'a, T: AsyncWrite>(&'a mut T);

pub trait AsyncWrite {
    async fn async_flush(&mut self) -> IoResult<()>;
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize>;

    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }
    #[inline]
    fn blocking(self) -> BlockingWrite<Self>
    where Self: Sized {
        BlockingWrite(self)
    }
    #[inline]
    fn by_ref_blocking(&mut self) -> BlockingWriteRef<'_, Self>
    where Self: Sized {
        BlockingWriteRef(self)
    }

    #[inline]
    async fn async_write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.async_write(buf).await {
                Ok(0) => return Err(IoError::from(ErrorKind::WriteZero)),
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl AsyncWrite for Sink {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }
}
impl AsyncWrite for &mut [u8] {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl AsyncWrite for Cursor<&mut [u8]> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl<const N: usize> AsyncWrite for Cursor<[u8; N]> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}

impl<'a> AsyncWrite for BorrowedCursor<'a> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}

impl<A: Allocator> AsyncWrite for Cursor<Vec<u8, A>> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl<A: Allocator> AsyncWrite for Cursor<Box<[u8], A>> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl<A: Allocator> AsyncWrite for Cursor<&mut Vec<u8, A>> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}

impl<A: Allocator> AsyncWrite for Vec<u8, A> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl<A: Allocator> AsyncWrite for VecDeque<u8, A> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write(buf)
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.write_all(buf)
    }
}
impl<A: Allocator, T: ?Sized + AsyncWrite> AsyncWrite for Box<T, A> {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        (**self).async_flush().await
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (**self).async_write(buf).await
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        (**self).async_write_all(buf).await
    }
}

impl<T: ?Sized + AsyncWrite> AsyncWrite for &mut T {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        (**self).async_flush().await
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (**self).async_write(buf).await
    }
    #[inline]
    async fn async_write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        (**self).async_write_all(buf).await
    }
}

impl<T: AsyncWrite> Write for BlockingWrite<T> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        link(self.0.async_flush())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        link(self.0.async_write(buf))
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        link(self.0.async_write_all(buf))
    }
}
impl<T: AsyncWrite + AsyncSeek> Seek for BlockingWrite<T> {
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

impl<T: AsyncWrite> Write for BlockingWriteRef<'_, T> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        link(self.0.async_flush())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        link(self.0.async_write(buf))
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        link(self.0.async_write_all(buf))
    }
}
impl<T: AsyncWrite + AsyncSeek> Seek for BlockingWriteRef<'_, T> {
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
