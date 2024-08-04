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

//
// Module assistance with help from the Rust Team std/io code!
//

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::clone::Clone;
use core::cmp;
use core::cmp::{Eq, Ord, PartialEq};
use core::convert::{AsRef, From, Into, TryInto};
use core::default::Default;
use core::iter::Iterator;
use core::marker::{Copy, Sized};
use core::mem::{drop, MaybeUninit};
use core::ops::{Drop, FnMut, FnOnce};
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::slice::from_mut;

use crate::io::{self, BorrowedBuf, BorrowedCursor, Error, ErrorKind, Seek, SeekFrom, Write};

pub struct Empty;
pub struct Repeat {
    byte: u8,
}
pub struct Take<T> {
    inner: T,
    limit: u64,
}
pub struct Bytes<T> {
    inner: T,
}
pub struct Split<T> {
    buf:   T,
    delim: u8,
}
pub struct Lines<T> {
    buf: T,
}
pub struct Cursor<T> {
    inner: T,
    pos:   u64,
}
pub struct Chain<T, U> {
    first:  T,
    second: U,
    done:   bool,
}
pub struct BufReader<T> {
    inner: T,
    buf:   Buffer,
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    #[inline]
    fn bytes(self) -> Bytes<Self>
    where Self: Sized {
        Bytes { inner: self }
    }
    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }
    #[inline]
    fn take(self, limit: u64) -> Take<Self>
    where Self: Sized {
        Take { inner: self, limit }
    }
    #[inline]
    fn chain<T: Read>(self, next: T) -> Chain<Self, T>
    where Self: Sized {
        Chain {
            done:   false,
            first:  self,
            second: next,
        }
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        read_exact(self, buf)
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        read_to_end(self, buf)
    }
    #[inline]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        read_buf(|b| self.read(b), buf)
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        read_string(self, buf)
    }
    fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        while cursor.capacity() > 0 {
            let p = cursor.written();
            match self.read_buf(cursor.reborrow()) {
                Ok(()) => (),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
            if cursor.written() == p {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "failed to fill buffer",
                ));
            }
        }
        Ok(())
    }
}
pub trait BufRead: Read {
    fn consume(&mut self, amt: usize);
    fn fill_buf(&mut self) -> io::Result<&[u8]>;

    #[inline]
    fn lines(self) -> Lines<Self>
    where Self: Sized {
        Lines { buf: self }
    }
    #[inline]
    fn split(self, byte: u8) -> Split<Self>
    where Self: Sized {
        Split { buf: self, delim: byte }
    }

    #[inline]
    fn has_data_left(&mut self) -> io::Result<bool> {
        self.fill_buf().map(|b| !b.is_empty())
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        Guard::new(buf).append(|b| read_until(self, b'\n', b))
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        read_until(self, byte, buf)
    }
}

struct Buffer {
    buf:    Box<[MaybeUninit<u8>]>,
    pos:    usize,
    filled: usize,
    init:   usize,
}
struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Buffer {
    #[inline]
    fn with_capacity(capacity: usize) -> Buffer {
        let buf = Box::new_uninit_slice(capacity);
        Buffer {
            buf,
            pos: 0usize,
            init: 0usize,
            filled: 0usize,
        }
    }

    #[inline]
    fn discard(&mut self) {
        self.pos = 0;
        self.filled = 0;
    }
    #[inline]
    fn pos(&self) -> usize {
        self.pos
    }
    #[inline]
    fn buffer(&self) -> &[u8] {
        unsafe { MaybeUninit::slice_assume_init_ref(self.buf.get_unchecked(self.pos..self.filled)) }
    }
    #[inline]
    fn filled(&self) -> usize {
        self.filled
    }
    #[inline]
    fn undo(&mut self, amt: usize) {
        self.pos = self.pos.saturating_sub(amt);
    }
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.filled);
    }
    #[inline]
    fn fill_buf(&mut self, mut reader: impl Read) -> io::Result<&[u8]> {
        if self.pos >= self.filled {
            let mut buf = BorrowedBuf::from(&mut *self.buf);
            unsafe {
                buf.set_init(self.init);
            }
            reader.read_buf(buf.unfilled())?;
            self.pos = 0;
            self.filled = buf.len();
            self.init = buf.init_len();
        }
        Ok(self.buffer())
    }
    #[inline]
    fn consume_with(&mut self, amt: usize, mut visitor: impl FnMut(&[u8])) -> bool {
        if let Some(claimed) = self.buffer().get(..amt) {
            visitor(claimed);
            self.pos += amt;
            true
        } else {
            false
        }
    }
}
impl<T> Take<T> {
    #[inline]
    pub fn limit(&self) -> u64 {
        self.limit
    }
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    #[inline]
    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit;
    }
}
impl<T> Cursor<T> {
    #[inline]
    pub const fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0u64, inner }
    }

    #[inline]
    pub const fn get_ref(&self) -> &T {
        &self.inner
    }
    #[inline]
    pub const fn position(&self) -> u64 {
        self.pos
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    #[inline]
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}
impl<'a> Guard<'a> {
    #[inline]
    fn new(buf: &'a mut String) -> Guard<'a> {
        Guard {
            len: buf.len(),
            buf: unsafe { buf.as_mut_vec() },
        }
    }

    #[inline]
    fn append<F: FnOnce(&mut Vec<u8>) -> io::Result<usize>>(mut self, f: F) -> io::Result<usize> {
        let r = f(self.buf);
        if core::str::from_utf8(&self.buf[self.len..]).is_err() {
            r.and_then(|_| Err(ErrorKind::InvalidData.into()))
        } else {
            self.len = self.buf.len();
            r
        }
    }
}
impl<T> BufReader<T> {
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        self.buf.buffer()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.buf.len()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    #[inline]
    fn discard(&mut self) {
        self.buf.discard()
    }
}
impl<T, U> Chain<T, U> {
    #[inline]
    pub fn into_inner(self) -> (T, U) {
        (self.first, self.second)
    }
    #[inline]
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.first, &self.second)
    }
    #[inline]
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.first, &mut self.second)
    }
}
impl<T: Read> BufReader<T> {
    #[inline]
    pub fn new(inner: T) -> BufReader<T> {
        io::BufReader {
            inner,
            buf: Buffer::with_capacity(super::BUF_SIZE),
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize, inner: T) -> BufReader<T> {
        BufReader {
            inner,
            buf: Buffer::with_capacity(capacity),
        }
    }
}
impl<T: Seek> BufReader<T> {
    pub fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        let p = self.buf.pos() as u64;
        if offset < 0 {
            if p.checked_sub((-offset) as u64).is_some() {
                self.buf.undo((-offset) as usize);
                return Ok(());
            }
        } else if let Some(n) = p.checked_add(offset as u64) {
            if n <= self.buf.filled() as u64 {
                self.buf.consume(offset as usize);
                return Ok(());
            }
        }
        self.seek(SeekFrom::Current(offset)).map(drop)
    }
}
impl<T: AsRef<[u8]>> Cursor<T> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pos >= self.inner.as_ref().len() as u64
    }
    #[inline]
    pub fn remaining_slice(&self) -> &[u8] {
        &self.inner.as_ref()[(self.pos.min(self.inner.as_ref().len() as u64) as usize)..]
    }
}

impl Read for &[u8] {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(n);
        if n == 1 {
            buf[0] = a[0];
        } else {
            buf[..n].copy_from_slice(a);
        }
        *self = b;
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if buf.len() > self.len() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ));
        }
        let (a, b) = self.split_at(buf.len());
        if buf.len() == 1 {
            buf[0] = a[0];
        } else {
            buf.copy_from_slice(a);
        }
        *self = b;
        Ok(())
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        buf.extend_from_slice(*self);
        let n = self.len();
        *self = &self[n..];
        Ok(n)
    }
    #[inline]
    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let n = cmp::min(cursor.capacity(), self.len());
        let (a, b) = self.split_at(n);
        cursor.append(a);
        *self = b;
        Ok(())
    }
}
impl BufRead for &[u8] {
    #[inline]
    fn consume(&mut self, amt: usize) {
        *self = &self[amt..];
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(*self)
    }
}
impl Read for VecDeque<u8> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (ref mut f, _) = self.as_slices();
        let n = Read::read(f, buf)?;
        self.drain(..n);
        Ok(n)
    }
    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let (ref mut f, _) = self.as_slices();
        let n = cmp::min(cursor.capacity(), f.len());
        Read::read_buf(f, cursor)?;
        self.drain(..n);
        Ok(())
    }
}

impl<T: Read + ?Sized> Read for &mut T {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        (**self).is_read_vectored()
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_to_end(buf)
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_to_string(buf)
    }
    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf(cursor)
    }
}
impl<T: Read + ?Sized> Read for Box<T> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        (**self).is_read_vectored()
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_to_end(buf)
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_to_string(buf)
    }
    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf(cursor)
    }
}
impl<T: BufRead + ?Sized> BufRead for &mut T {
    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        (**self).fill_buf()
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_line(buf)
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_until(byte, buf)
    }
}
impl<T: BufRead + ?Sized> BufRead for Box<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        (**self).fill_buf()
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_line(buf)
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_until(byte, buf)
    }
}

impl Read for Empty {
    #[inline]
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
    #[inline]
    fn read_buf(&mut self, _cursor: BorrowedCursor<'_>) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Empty {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(0)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(0)
    }
    #[inline]
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Ok(0)
    }
}
impl Copy for Empty {}
impl Clone for Empty {
    #[inline]
    fn clone(&self) -> Empty {
        Empty {}
    }
}
impl BufRead for Empty {
    #[inline]
    fn consume(&mut self, _n: usize) {}
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(&[])
    }
}
impl Default for Empty {
    #[inline]
    fn default() -> Empty {
        Empty {}
    }
}

impl Read for Repeat {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for v in &mut *buf {
            *v = self.byte;
        }
        Ok(buf.len())
    }
    #[inline]
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> io::Result<()> {
        for v in unsafe { buf.as_mut() } {
            v.write(self.byte);
        }
        let r = buf.capacity();
        unsafe { buf.advance(r) };
        Ok(())
    }
}

impl<T: Read> Read for Take<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.limit == 0 {
            return Ok(0);
        }
        let m = cmp::min(buf.len() as u64, self.limit) as usize;
        let n = self.inner.read(&mut buf[..m])?;
        self.limit -= n as u64;
        Ok(n)
    }
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> io::Result<()> {
        if self.limit == 0 {
            return Ok(());
        }
        if self.limit <= buf.capacity() as u64 {
            let n = cmp::min(self.limit, usize::MAX as u64) as usize;
            let r = cmp::min(n as usize, buf.init_ref().len());
            let mut b: BorrowedBuf<'_> = unsafe { &mut buf.as_mut()[..n] }.into();
            unsafe { b.set_init(r) };
            let mut c = b.unfilled();
            self.inner.read_buf(c.reborrow())?;
            let x = c.init_ref().len();
            let f = b.len();
            unsafe {
                buf.advance(f);
                buf.set_init(x);
            }
            self.limit -= f as u64;
        } else {
            let w = buf.written();
            self.inner.read_buf(buf.reborrow())?;
            self.limit -= (buf.written() - w) as u64;
        }
        Ok(())
    }
}
impl<T: BufRead> BufRead for Take<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        let a = cmp::min(amt as u64, self.limit) as usize;
        self.limit -= a as u64;
        self.inner.consume(a);
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.limit == 0 {
            return Ok(&[]);
        }
        let b = self.inner.fill_buf()?;
        let c = cmp::min(b.len() as u64, self.limit) as usize;
        Ok(&b[..c])
    }
}

impl<T: Read> Iterator for Bytes<T> {
    type Item = io::Result<u8>;

    #[inline]
    fn next(&mut self) -> Option<io::Result<u8>> {
        let mut b = 0;
        loop {
            return match self.inner.read(from_mut(&mut b)) {
                Ok(0) => None,
                Ok(..) => Some(Ok(b)),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => Some(Err(e)),
            };
        }
    }
}
impl<T: BufRead> Iterator for Split<T> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<io::Result<Vec<u8>>> {
        let mut b = Vec::new();
        match self.buf.read_until(self.delim, &mut b) {
            Ok(0) => None,
            Ok(_) => {
                if b[b.len() - 1] == self.delim {
                    b.pop();
                }
                Some(Ok(b))
            },
            Err(e) => Some(Err(e)),
        }
    }
}
impl<T: BufRead> Iterator for Lines<T> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<io::Result<String>> {
        let mut b = String::new();
        match self.buf.read_line(&mut b) {
            Ok(0) => None,
            Ok(_) => {
                if b.ends_with('\n') {
                    b.pop();
                    if b.ends_with('\r') {
                        b.pop();
                    }
                }
                Some(Ok(b))
            },
            Err(e) => Some(Err(e)),
        }
    }
}

impl<T: Eq> Eq for Cursor<T> {}
impl<T: Clone> Clone for Cursor<T> {
    #[inline]
    fn clone(&self) -> Cursor<T> {
        Cursor {
            inner: self.inner.clone(),
            pos:   self.pos,
        }
    }
    #[inline]
    fn clone_from(&mut self, other: &Cursor<T>) {
        self.inner.clone_from(&other.inner);
        self.pos = other.pos;
    }
}
impl<T: Default> Default for Cursor<T> {
    #[inline]
    fn default() -> Cursor<T> {
        Cursor {
            pos:   0u64,
            inner: Default::default(),
        }
    }
}
impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(self.inner.as_ref().len() as u64)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.pos)
    }
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let (b, o) = match style {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            },
            SeekFrom::End(n) => (self.inner.as_ref().len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };
        match b.checked_add_signed(o) {
            Some(n) => {
                self.pos = n;
                Ok(self.pos)
            },
            None => Err(Error::from(ErrorKind::InvalidInput)),
        }
    }
}
impl<T: AsRef<[u8]>> Read for Cursor<T> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = Read::read(&mut self.remaining_slice(), buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let n = buf.len();
        Read::read_exact(&mut self.remaining_slice(), buf)?;
        self.pos += n as u64;
        Ok(())
    }
    #[inline]
    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let n = cursor.written();
        Read::read_buf(&mut self.fill_buf()?, cursor.reborrow())?;
        self.pos += (cursor.written() - n) as u64;
        Ok(())
    }
}
impl<T: AsRef<[u8]>> BufRead for Cursor<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.pos += amt as u64;
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(self.remaining_slice())
    }
}
impl<T: PartialEq> PartialEq for Cursor<T> {
    #[inline]
    fn eq(&self, other: &Cursor<T>) -> bool {
        self.inner == other.inner && self.pos == other.pos
    }
}

impl Write for Cursor<Vec<u8>> {
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
        write_vec(&mut self.pos, &mut self.inner, buf)
    }
}
impl Write for Cursor<&mut [u8]> {
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
        write_slice(&mut self.pos, self.inner, buf)
    }
}
impl Write for Cursor<Box<[u8]>> {
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
        write_slice(&mut self.pos, &mut self.inner, buf)
    }
}
impl Write for Cursor<&mut Vec<u8>> {
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
        write_vec(&mut self.pos, self.inner, buf)
    }
}
impl<const N: usize> Write for Cursor<[u8; N]> {
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
        write_slice(&mut self.pos, &mut self.inner, buf)
    }
}

impl<T: Read, U: Read> Read for Chain<T, U> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.done {
            match self.first.read(buf)? {
                0 if !buf.is_empty() => self.done = true,
                n => return Ok(n),
            }
        }
        self.second.read(buf)
    }
}
impl<T: BufRead, U: BufRead> BufRead for Chain<T, U> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        if !self.done {
            self.first.consume(amt)
        } else {
            self.second.consume(amt)
        }
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if !self.done {
            match self.first.fill_buf()? {
                buf if buf.is_empty() => self.done = true,
                buf => return Ok(buf),
            }
        }
        self.second.fill_buf()
    }
}

impl<T: Seek> Seek for BufReader<T> {
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        let r = (self.buf.filled() - self.buf.pos()) as u64;
        self.inner
            .stream_position()
            .and_then(|pos| pos.checked_sub(r).ok_or_else(|| Error::from(ErrorKind::InvalidInput)))
    }
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let r = if let SeekFrom::Current(n) = pos {
            let v = (self.buf.filled() - self.buf.pos()) as i64;
            if let Some(o) = n.checked_sub(v) {
                self.inner.seek(SeekFrom::Current(o))?
            } else {
                self.inner.seek(SeekFrom::Current(-v))?;
                self.discard();
                self.inner.seek(SeekFrom::Current(n))?
            }
        } else {
            self.inner.seek(pos)?
        };
        self.discard();
        Ok(r)
    }
}
impl<T: Read> Read for BufReader<T> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buf.pos() == self.buf.filled() && buf.len() >= self.capacity() {
            self.discard();
            return self.inner.read(buf);
        }
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if self.buf.consume_with(buf.len(), |c| buf.copy_from_slice(c)) {
            return Ok(());
        }
        read_exact(self, buf)
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let n = {
            let i = self.buffer();
            buf.extend_from_slice(i);
            i.len()
        };
        self.discard();
        Ok(n + self.inner.read_to_end(buf)?)
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        if buf.is_empty() {
            Guard::new(buf).append(|b| self.read_to_end(b))
        } else {
            let mut b = Vec::new();
            self.read_to_end(&mut b)?;
            let s = core::str::from_utf8(&b).map_err(|_| Error::from(ErrorKind::InvalidData))?;
            buf.push_str(s);
            Ok(s.len())
        }
    }
    #[inline]
    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        if self.buf.pos() == self.buf.filled() && cursor.capacity() >= self.capacity() {
            self.discard();
            return self.inner.read_buf(cursor);
        }
        let p = cursor.written();
        self.fill_buf()?.read_buf(cursor.reborrow())?;
        self.consume(cursor.written() - p);
        Ok(())
    }
}
impl<T: Read> BufRead for BufReader<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.buf.consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.buf.fill_buf(&mut self.inner)
    }
}

impl Drop for Guard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.buf.set_len(self.len) }
    }
}

#[inline]
pub const fn empty() -> Empty {
    Empty
}
#[inline]
pub const fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

#[inline]
fn memchr(x: u8, text: &[u8]) -> Option<usize> {
    if text.len() < 2 * super::PTR_SIZE {
        memchr_naive(x, text)
    } else {
        memchr_aligned(x, text)
    }
}
#[inline]
fn memchr_naive(x: u8, text: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < text.len() {
        if text[i] == x {
            return Some(i);
        }
        i += 1;
    }
    None
}
fn memchr_aligned(x: u8, text: &[u8]) -> Option<usize> {
    let n = text.len();
    let p = text.as_ptr();
    let mut o = p.align_offset(super::PTR_SIZE);
    if o > 0 {
        o = cmp::min(o, n);
        if let Some(index) = memchr_naive(x, &text[..o]) {
            return Some(index);
        }
    }
    let r = super::repeat_byte(x);
    while o <= n - 2 * super::PTR_SIZE {
        unsafe {
            if super::zero_byte((*(p.add(o) as *const usize)) ^ r) || super::zero_byte((*(p.add(o + super::PTR_SIZE) as *const usize)) ^ r) {
                break;
            }
        }
        o += super::PTR_SIZE * 2;
    }
    memchr_naive(x, &text[o..]).map(|i| i + o)
}
#[inline]
fn read_string<R: Read + ?Sized>(r: &mut R, buf: &mut String) -> io::Result<usize> {
    Guard::new(buf).append(|b| read_to_end(r, b))
}
fn read_to_end<R: Read + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> io::Result<usize> {
    let s = buf.len();
    let c = buf.capacity();
    let mut i = 0;
    loop {
        if buf.len() == buf.capacity() {
            buf.reserve(32)
        }
        let mut b = BorrowedBuf::from(buf.spare_capacity_mut());
        unsafe { b.set_init(i) };
        let mut v = b.unfilled();
        match r.read_buf(v.reborrow()) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if v.written() == 0 {
            return Ok(buf.len() - s);
        }
        i = v.init_ref().len();
        let n = b.filled().len();
        unsafe { buf.set_len(n + buf.len()) };
        if buf.len() == buf.capacity() && buf.capacity() == c {
            let mut t = [0u8; 32];
            loop {
                match r.read(&mut t) {
                    Ok(0) => return Ok(buf.len() - s),
                    Ok(n) => {
                        buf.extend_from_slice(&t[..n]);
                        break;
                    },
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
    }
}
#[inline]
fn read_exact<R: Read + ?Sized>(this: &mut R, mut buf: &mut [u8]) -> io::Result<()> {
    while !buf.is_empty() {
        match this.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                let t = buf;
                buf = &mut t[n..];
            },
            Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }
    if !buf.is_empty() {
        return Err(Error::new(
            ErrorKind::UnexpectedEof,
            "failed to fill whole buffer",
        ));
    }
    Ok(())
}
fn write_vec(pos_mut: &mut u64, vec: &mut Vec<u8>, buf: &[u8]) -> io::Result<usize> {
    let n = buf.len();
    let i = {
        let p: usize = (*pos_mut).try_into().map_err(|_| Error::from(ErrorKind::InvalidInput))?;
        let d = p.saturating_add(n);
        if d > vec.capacity() {
            vec.reserve(d - vec.len());
        }
        if p > vec.len() {
            let z = p - vec.len();
            unsafe {
                vec.spare_capacity_mut().get_unchecked_mut(0..z).fill(MaybeUninit::new(0));
                vec.set_len(p);
            }
        }
        p
    };
    unsafe {
        vec.as_mut_ptr().add(i).copy_from(buf.as_ptr(), buf.len());
        let p = i + buf.len();
        if p > vec.len() {
            vec.set_len(p);
        }
    };
    *pos_mut += n as u64;
    Ok(n)
}
#[inline]
fn write_slice(pos_mut: &mut u64, slice: &mut [u8], buf: &[u8]) -> io::Result<usize> {
    let p = cmp::min(*pos_mut, slice.len() as u64) as usize;
    let a = (&mut slice[p..]).write(buf)?;
    *pos_mut += a as u64;
    Ok(a)
}
fn read_until<R: BufRead + ?Sized>(r: &mut R, delim: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
    let mut n = 0;
    loop {
        let (d, c) = {
            let a = match r.fill_buf() {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr(delim, a) {
                Some(i) => {
                    buf.extend_from_slice(&a[..=i]);
                    (true, i + 1)
                },
                None => {
                    buf.extend_from_slice(a);
                    (false, a.len())
                },
            }
        };
        r.consume(c);
        n += c;
        if d || c == 0 {
            return Ok(n);
        }
    }
}
#[inline]
fn read_buf<F: FnOnce(&mut [u8]) -> io::Result<usize>>(read: F, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
    let n = read(cursor.ensure_init().init_mut())?;
    unsafe { cursor.advance(n) };
    Ok(())
}
