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
#![cfg(not(feature = "std"))]

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::fmt::{self, Arguments, Debug, Display, Formatter};
use core::{cmp, error, mem, ptr};

use crate::util::stx::io::{self, Error, ErrorKind, Seek, SeekFrom};
use crate::util::stx::prelude::*;

pub struct Sink;
pub struct WriterPanicked;
pub struct BufWriter<T: Write> {
    inner:    T,
    buf:      Vec<u8>,
    panicked: bool,
}
pub struct LineWriter<T: Write> {
    inner: BufWriter<T>,
}
pub struct IntoInnerError<T>(T, Error);

pub trait Write {
    fn flush(&mut self) -> io::Result<()>;
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write_all(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    #[inline]
    fn write_fmt(&mut self, fmt: Arguments<'_>) -> io::Result<()> {
        Adapter::new(self).write(fmt)
    }
}

struct BufGuard<'a> {
    buffer:  &'a mut Vec<u8>,
    written: usize,
}
struct Adapter<'a, T: ?Sized + 'a> {
    inner: &'a mut T,
    error: io::Result<()>,
}

impl WriterPanicked {
    #[inline]
    pub fn into_inner(self) -> Vec<u8> {
        Vec::new()
    }
}
impl<'a> BufGuard<'a> {
    #[inline]
    fn new(buffer: &'a mut Vec<u8>) -> BufGuard<'a> {
        BufGuard { buffer, written: 0 }
    }

    #[inline]
    fn done(&self) -> bool {
        self.written >= self.buffer.len()
    }
    #[inline]
    fn remaining(&self) -> &[u8] {
        &self.buffer[self.written..]
    }
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.written += amt;
    }
}
impl<T> IntoInnerError<T> {
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
    #[inline]
    pub fn error(&self) -> &Error {
        &self.1
    }
    #[inline]
    pub fn into_error(self) -> Error {
        self.1
    }
    #[inline]
    pub fn into_parts(self) -> (Error, T) {
        (self.1, self.0)
    }
}
impl<T: Write> BufWriter<T> {
    #[inline]
    pub fn new(inner: T) -> BufWriter<T> {
        BufWriter {
            inner,
            buf: Vec::with_capacity(io::BUF_SIZE),
            panicked: false,
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize, inner: T) -> BufWriter<T> {
        BufWriter {
            inner,
            buf: Vec::with_capacity(capacity),
            panicked: false,
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    pub fn into_parts(mut self) -> (T, Result<Vec<u8>, WriterPanicked>) {
        let b = mem::take(&mut self.buf);
        let v = if !self.panicked { Ok(b) } else { Err(WriterPanicked) };
        let i = unsafe { ptr::read(&self.inner) };
        mem::forget(self);
        (i, v)
    }
    #[inline]
    pub fn into_inner(mut self) -> Result<T, IntoInnerError<BufWriter<T>>> {
        match self.flush_buf() {
            Err(e) => Err(IntoInnerError(self, e)),
            Ok(()) => Ok(self.into_parts().0),
        }
    }

    #[inline]
    fn spare_capacity(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }
    fn flush_buf(&mut self) -> io::Result<()> {
        let mut g = BufGuard::new(&mut self.buf);
        while !g.done() {
            match self.inner.write(g.remaining()) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(n) => g.consume(n),
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    fn write_to_buf(&mut self, buf: &[u8]) -> usize {
        let n = self.spare_capacity().min(buf.len());
        unsafe { self.write_to_buffer_unchecked(&buf[..n]) };
        n
    }
    #[cold]
    #[inline(never)]
    fn write_cold(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.spare_capacity() {
            self.flush_buf()?;
        }
        if buf.len() >= self.buf.capacity() {
            self.get_mut().write(buf)
        } else {
            unsafe { self.write_to_buffer_unchecked(buf) };
            Ok(buf.len())
        }
    }
    #[cold]
    #[inline(never)]
    fn write_all_cold(&mut self, buf: &[u8]) -> io::Result<()> {
        if buf.len() > self.spare_capacity() {
            self.flush_buf()?;
        }
        if buf.len() >= self.buf.capacity() {
            self.get_mut().write_all(buf)
        } else {
            unsafe { self.write_to_buffer_unchecked(buf) };
            Ok(())
        }
    }

    #[inline]
    unsafe fn write_to_buffer_unchecked(&mut self, buf: &[u8]) {
        let (o, n) = (self.buf.len(), buf.len());
        ptr::copy_nonoverlapping(buf.as_ptr(), self.buf.as_mut_ptr().add(o), n);
        self.buf.set_len(o + n);
    }
}
impl<W: Write> LineWriter<W> {
    #[inline]
    pub fn new(inner: W) -> LineWriter<W> {
        LineWriter {
            inner: BufWriter::with_capacity(1024, inner),
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize, inner: W) -> LineWriter<W> {
        LineWriter {
            inner: BufWriter::with_capacity(capacity, inner),
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }
    #[inline]
    pub fn into_inner(self) -> Result<W, IntoInnerError<LineWriter<W>>> {
        self.inner.into_inner().map_err(|e| {
            let (e, w) = e.into_parts();
            IntoInnerError(LineWriter { inner: w }, e)
        })
    }

    #[inline]
    fn flush_if_completed(&mut self) -> io::Result<()> {
        match self.inner.buffer().last().copied() {
            Some(b'\n') => self.inner.flush_buf(),
            _ => Ok(()),
        }
    }
}
impl<'a, T: Write + ?Sized> Adapter<'a, T> {
    #[inline]
    fn new(inner: &'a mut T) -> Adapter<'a, T> {
        Adapter { inner, error: Ok(()) }
    }

    #[inline]
    fn write(mut self, fmt: Arguments<'_>) -> io::Result<()> {
        if fmt::write(&mut self, fmt).is_err() {
            return if self.error.is_err() {
                self.error
            } else {
                Err(Error::new(ErrorKind::InvalidInput, "format error"))
            };
        }
        Ok(())
    }
}

impl Drop for BufGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        if self.written > 0 {
            self.buffer.drain(..self.written);
        }
    }
}
impl<T: Write + ?Sized> fmt::Write for Adapter<'_, T> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Err(e) = self.inner.write_all(s.as_bytes()) {
            self.error = Err(e);
            return Err(fmt::Error);
        }
        Ok(())
    }
}

impl Copy for Sink {}
impl Clone for Sink {
    #[inline]
    fn clone(&self) -> Sink {
        Sink {}
    }
}
impl Write for Sink {
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
        Ok(buf.len())
    }
}
impl Write for &Sink {
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
        Ok(buf.len())
    }
}
impl Default for Sink {
    #[inline]
    fn default() -> Sink {
        Sink {}
    }
}

impl Write for Vec<u8> {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}
impl Write for &mut [u8] {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let n = cmp::min(data.len(), self.len());
        let (a, b) = mem::replace(self, &mut []).split_at_mut(n);
        a.copy_from_slice(&data[..n]);
        *self = b;
        Ok(n)
    }
    #[inline]
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if self.write(data)? == data.len() {
            Ok(())
        } else {
            Err(ErrorKind::WriteZero.into())
        }
    }
}
impl Write for VecDeque<u8> {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend(buf);
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend(buf);
        Ok(())
    }
}

impl<W: Write + ?Sized> Write for &mut W {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write_all(buf)
    }
    #[inline]
    fn write_fmt(&mut self, fmt: Arguments<'_>) -> io::Result<()> {
        (**self).write_fmt(fmt)
    }
}
impl<W: Write + ?Sized> Write for Box<W> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write_all(buf)
    }
    #[inline]
    fn write_fmt(&mut self, fmt: Arguments<'_>) -> io::Result<()> {
        (**self).write_fmt(fmt)
    }
}

impl Debug for WriterPanicked {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
impl Display for WriterPanicked {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
impl error::Error for WriterPanicked {
    #[inline]
    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl<T> Debug for IntoInnerError<T> {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
impl<T> Display for IntoInnerError<T> {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
impl<T> From<IntoInnerError<T>> for Error {
    #[inline]
    fn from(v: IntoInnerError<T>) -> Error {
        v.1
    }
}
impl<T> error::Error for IntoInnerError<T> {
    #[inline]
    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl<W: Write> Drop for BufWriter<W> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.flush_buf(); // IGNORE ERROR
    }
}
impl<W: Write> Write for BufWriter<W> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf().and_then(|()| self.get_mut().flush())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() < self.spare_capacity() {
            unsafe { self.write_to_buffer_unchecked(buf) };
            Ok(buf.len())
        } else {
            self.write_cold(buf)
        }
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if buf.len() < self.spare_capacity() {
            unsafe { self.write_to_buffer_unchecked(buf) };
            Ok(())
        } else {
            self.write_all_cold(buf)
        }
    }
}
impl<W: Write + Seek> Seek for BufWriter<W> {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.flush_buf()?;
        self.get_mut().seek(pos)
    }
}

impl<W: Write> Write for LineWriter<W> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let i = match memrchr(b'\n', buf) {
            None => {
                self.flush_if_completed()?;
                return self.inner.write(buf);
            },
            Some(v) => v + 1,
        };
        self.inner.flush_buf()?;
        let f = self.inner.get_mut().write(&buf[..i])?;
        if f == 0 {
            return Ok(0);
        }
        let r = if f >= i {
            &buf[f..]
        } else if i - f <= self.inner.capacity() {
            &buf[f..i]
        } else {
            let a = &buf[f..];
            let a = &a[..self.inner.capacity()];
            match memrchr(b'\n', a) {
                Some(v) => &a[..v + 1],
                None => a,
            }
        };
        Ok(f + self.inner.write_to_buf(r))
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match memrchr(b'\n', buf) {
            None => {
                self.flush_if_completed()?;
                self.inner.write_all(buf)
            },
            Some(i) => {
                let (r, t) = buf.split_at(i + 1);
                if self.inner.buffer().is_empty() {
                    self.inner.get_mut().write_all(r)?;
                } else {
                    self.inner.write_all(r)?;
                    self.inner.flush_buf()?;
                }
                self.inner.write_all(t)
            },
        }
    }
}

#[inline]
pub const fn sink() -> Sink {
    Sink
}

fn memrchr(x: u8, text: &[u8]) -> Option<usize> {
    let c = text.len();
    let p = text.as_ptr();
    let (n, mut o) = {
        let (y, _, z) = unsafe { text.align_to::<(usize, usize)>() };
        (y.len(), c - z.len())
    };
    if let Some(i) = text[o..].iter().rposition(|v| *v == x) {
        return Some(o + i);
    }
    let r = io::repeat_byte(x);
    while o > n {
        unsafe {
            if io::zero_byte((*(p.add(o - 2 * io::PTR_SIZE) as *const usize)) ^ r) || io::zero_byte((*(p.add(o - io::PTR_SIZE) as *const usize)) ^ r) {
                break;
            }
        }
        o -= 2 * io::PTR_SIZE;
    }
    text[..o].iter().rposition(|v| *v == x)
}
