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
#![cfg(all(not(feature = "std"), feature = "alloc"))]

extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::cmp::Ord;
use core::convert::From;
use core::fmt::{Arguments, Debug, Display, Formatter};
use core::iter::Extend;
use core::marker::Sized;
use core::mem::{take, ManuallyDrop, MaybeUninit};
use core::ops::Drop;
use core::option::Option::{self, None, Some};
use core::ptr::{copy_nonoverlapping, read};
use core::result::Result::{self, Err, Ok};
use core::slice::memchr::memrchr;

use crate::{write_slice, CoreError, Cursor, Error, ErrorKind, FmtResult, IoError, IoResult, Seek, SeekFrom, Write, BASE_BUF_SIZE};

/// Error returned for the buffered data from `BufWriter::into_parts`, when the
/// underlying writer has previously panicked.  Contains the (possibly partly
/// written) buffered data.
///
/// # Example
///
/// ```
/// use xrmt_stx::io::{self, BufWriter, Write};
/// use xrmt_stx::panic::{catch_unwind, AssertUnwindSafe};
///
/// struct PanickingWriter;
/// impl Write for PanickingWriter {
///   fn write(&mut self, buf: &[u8]) -> IoResult<usize> { panic!() }
///   fn flush(&mut self) -> IoResult<()> { panic!() }
/// }
///
/// let mut stream = BufWriter::new(PanickingWriter);
/// write!(stream, "some data").unwrap();
/// let result = catch_unwind(AssertUnwindSafe(|| {
///     stream.flush().unwrap()
/// }));
/// assert!(result.is_err());
/// let (recovered_writer, buffered_data) = stream.into_parts();
/// assert!(matches!(recovered_writer, PanickingWriter));
/// assert_eq!(buffered_data.unwrap_err().into_inner(), b"some data");
/// ```
pub struct WriterPanicked(());
/// An error returned by [`BufWriter::into_inner`] which combines an error that
/// happened while writing out the buffer, and the buffered writer object
/// which may be used to recover from the condition.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::BufWriter;
/// use xrmt_stx::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// // do stuff with the stream
///
/// // we want to get our `TcpStream` back, so let's try:
///
/// let stream = match stream.into_inner() {
///     Ok(s) => s,
///     Err(e) => {
///         // Here, e is an IntoInnerError
///         panic!("An error occurred");
///     }
/// };
/// ```
pub struct IntoInnerError<T>(T, Error);
/// Wraps a writer and buffers its output.
///
/// It can be excessively inefficient to work directly with something that
/// implements [`Write`]. For example, every call to
/// `write` on `TcpStream` results in a system call. A
/// `BufWriter<W>` keeps an in-memory buffer of data and writes it to an
/// underlying writer in large, infrequent batches.
///
/// `BufWriter<W>` can improve the speed of programs that make *small* and
/// *repeated* write calls to the same file or network socket. It does not
/// help when writing very large amounts at once, or writing just one or a few
/// times. It also provides no advantage when writing to a destination that is
/// in memory, like a <code>[Vec]\<u8></code>.
///
/// It is critical to call [`flush`] before `BufWriter<W>` is dropped. Though
/// dropping will attempt to flush the contents of the buffer, any errors
/// that happen in the process of dropping will be ignored. Calling [`flush`]
/// ensures that the buffer is empty and thus dropping will not even attempt
/// file operations.
///
/// # Examples
///
/// Let's write the numbers one through ten to a `TcpStream`:
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::net::TcpStream;
///
/// let mut stream = TcpStream::connect("127.0.0.1:34254").unwrap();
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// ```
///
/// Because we're not buffering, we write each one in turn, incurring the
/// overhead of a system call per byte written. We can fix this with a
/// `BufWriter<W>`:
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::io::BufWriter;
/// use xrmt_stx::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// stream.flush().unwrap();
/// ```
///
/// By wrapping the stream with a `BufWriter<W>`, these ten writes are all
/// grouped together by the buffer and will all be written out in one system
/// call when the `stream` is flushed.
///
/// [`flush`]: BufWriter::flush
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct BufWriter<T: ?Sized + Write> {
    buf:   Vec<u8>,
    inner: T,
}
/// Wraps a writer and buffers output to it, flushing whenever a newline
/// (`0x0a`, `'\n'`) is detected.
///
/// The [`BufWriter`] struct wraps a writer and buffers its output.
/// But it only does this batched write when it goes out of scope, or when the
/// internal buffer is full. Sometimes, you'd prefer to write each line as it's
/// completed, rather than the entire buffer at once. Enter `LineWriter`. It
/// does exactly that.
///
/// Like [`BufWriter`], a `LineWriter`’s buffer will also be flushed when the
/// `LineWriter` goes out of scope or when its internal buffer is full.
///
/// If there's still a partial line in the buffer when the `LineWriter` is
/// dropped, it will flush those contents.
///
/// # Examples
///
/// We can use `LineWriter` to write one line at a time, significantly
/// reducing the number of actual writes to the file.
///
/// ```no_run
/// use xrmt_stx::fs::{self, File};
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::io::LineWriter;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let road_not_taken = b"I shall be telling this with a sigh
/// Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.";
///
///     let file = File::create("poem.txt")?;
///     let mut file = LineWriter::new(file);
///
///     file.write_all(b"I shall be telling this with a sigh")?;
///
///     // No bytes are written until a newline is encountered (or
///     // the internal buffer is filled).
///     assert_eq!(fs::read_to_string("poem.txt")?, "");
///     file.write_all(b"\n")?;
///     assert_eq!(
///         fs::read_to_string("poem.txt")?,
///         "I shall be telling this with a sigh\n",
///     );
///
///     // Write the rest of the poem.
///     file.write_all(b"Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.")?;
///
///     // The last line of the poem doesn't end in a newline, so
///     // we have to flush or drop the `LineWriter` to finish
///     // writing.
///     file.flush()?;
///
///     // Confirm the whole poem was written.
///     assert_eq!(fs::read("poem.txt")?, &road_not_taken[..]);
///     Ok(())
/// }
/// ```
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct LineWriter<T: ?Sized + Write>(BufWriter<T>);

struct BufGuard<'a> {
    b: &'a mut Vec<u8>,
    n: usize,
}

impl WriterPanicked {
    /// Returns the perhaps-unwritten data.  Some of this data may have been
    /// written by the panicking call(s) to the underlying writer, so simply
    /// writing it again is not a good idea.
    #[inline]
    pub fn into_inner(self) -> Vec<u8> {
        Vec::new()
    }
}
impl<'a> BufGuard<'a> {
    #[inline]
    fn new(b: &'a mut Vec<u8>) -> BufGuard<'a> {
        BufGuard { b, n: 0 }
    }

    #[inline]
    fn done(&self) -> bool {
        self.n >= self.b.len()
    }
    #[inline]
    fn remaining(&self) -> &[u8] {
        // Checked via 'done'
        unsafe { self.b.get_unchecked(self.n..) }
    }
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.n += amt;
    }
}
impl<T> IntoInnerError<T> {
    /// Returns the buffered writer instance which generated the error.
    ///
    /// The returned object can be used for error recovery, such as
    /// re-inspecting the buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // do stuff with the stream
    ///
    /// // we want to get our `TcpStream` back, so let's try:
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // Here, e is an IntoInnerError, let's re-examine the buffer:
    ///         let buffer = e.into_inner();
    ///
    ///         // do stuff to try to recover
    ///
    ///         // afterwards, let's just return the stream
    ///         buffer.into_inner().unwrap()
    ///     }
    /// };
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
    /// Returns the error which caused the call to [`BufWriter::into_inner()`]
    /// to fail.
    ///
    /// This error was returned when attempting to write the internal buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // do stuff with the stream
    ///
    /// // we want to get our `TcpStream` back, so let's try:
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // Here, e is an IntoInnerError, let's log the inner error.
    ///         //
    ///         // We'll just 'log' to stdout for this example.
    ///         println!("{}", e.error());
    ///
    ///         panic!("An unexpected error occurred.");
    ///     }
    /// };
    /// ```
    #[inline]
    pub fn error(&self) -> &Error {
        &self.1
    }
    /// Consumes the [`IntoInnerError`] and returns the error which caused the
    /// call to [`BufWriter::into_inner()`] to fail.  Unlike `error`, this
    /// can be used to obtain ownership of the underlying error.
    ///
    /// # Example
    /// ```
    /// use xrmt_stx::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let err = into_inner_err.into_error();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// ```
    #[inline]
    pub fn into_error(self) -> Error {
        self.1
    }
    /// Consumes the [`IntoInnerError`] and returns the error which caused the
    /// call to [`BufWriter::into_inner()`] to fail, and the underlying
    /// writer.
    ///
    /// This can be used to simply obtain ownership of the underlying error; it
    /// can also be used for advanced error recovery.
    ///
    /// # Example
    /// ```
    /// use xrmt_stx::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let (err, recovered_writer) = into_inner_err.into_parts();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// assert_eq!(recovered_writer.buffer(), b"t be actually written");
    /// ```
    #[inline]
    pub fn into_parts(self) -> (Error, T) {
        (self.1, self.0)
    }
}
impl<T: Write> BufWriter<T> {
    /// Creates a new `BufWriter<W>` with a default buffer capacity. The default
    /// is currently 8 KiB, but may change in the future.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    /// ```
    #[inline]
    pub fn new(inner: T) -> BufWriter<T> {
        BufWriter {
            inner,
            buf: Vec::with_capacity(BASE_BUF_SIZE),
        }
    }
    /// Creates a new `BufWriter<W>` with at least the specified buffer
    /// capacity.
    ///
    /// # Examples
    ///
    /// Creating a buffer with a buffer of at least a hundred bytes.
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:34254").unwrap();
    /// let mut buffer = BufWriter::with_capacity(100, stream);
    /// ```
    #[inline]
    pub fn with_capacity(len: usize, inner: T) -> BufWriter<T> {
        BufWriter {
            inner,
            buf: Vec::with_capacity(len),
        }
    }

    /// Disassembles this `BufWriter<W>`, returning the underlying writer, and
    /// any buffered but unwritten data.
    ///
    /// If the underlying writer panicked, it is not known what portion of the
    /// data was written. In this case, we return `WriterPanicked` for the
    /// buffered data (from which the buffer contents can still be
    /// recovered).
    ///
    /// `into_parts` makes no attempt to flush data and cannot fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::{BufWriter, Write};
    ///
    /// let mut buffer = [0u8; 10];
    /// let mut stream = BufWriter::new(buffer.as_mut());
    /// write!(stream, "too much data").unwrap();
    /// stream.flush().expect_err("it doesn't fit");
    /// let (recovered_writer, buffered_data) = stream.into_parts();
    /// assert_eq!(recovered_writer.len(), 0);
    /// assert_eq!(&buffered_data.unwrap(), b"ata");
    /// ```
    #[inline]
    pub fn into_parts(self) -> (T, Result<Vec<u8>, WriterPanicked>) {
        let mut m = ManuallyDrop::new(self);
        let (b, i) = (take(&mut m.buf), unsafe { read(&m.inner) });
        (i, Ok(b))
    }
    // Unwraps this `BufWriter<W>`, returning the underlying writer.
    ///
    /// The buffer is written out before returning the writer.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if an error occurs while flushing the
    /// buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // unwrap the TcpStream and flush the buffer
    /// let stream = buffer.into_inner().unwrap();
    /// ```
    #[inline]
    pub fn into_inner(mut self) -> Result<T, IntoInnerError<BufWriter<T>>> {
        match self.flush_buf() {
            Err(e) => Err(IntoInnerError(self, e)),
            Ok(()) => Ok(self.into_parts().0),
        }
    }
}
impl<T: Write> LineWriter<T> {
    /// Creates a new `LineWriter`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::LineWriter;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::new(file);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn new(inner: T) -> LineWriter<T> {
        LineWriter(BufWriter::with_capacity(0x400, inner))
    }
    /// Creates a new `LineWriter` with at least the specified capacity for the
    /// internal buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::LineWriter;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::with_capacity(100, file);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn with_capacity(len: usize, inner: T) -> LineWriter<T> {
        LineWriter(BufWriter::with_capacity(len, inner))
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// Caution must be taken when calling methods on the mutable reference
    /// returned as extra writes could corrupt the output stream.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::LineWriter;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let mut file = LineWriter::new(file);
    ///
    ///     // we can use reference just like file
    ///     let reference = file.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
    /// Unwraps this `LineWriter`, returning the underlying writer.
    ///
    /// The internal buffer is written out before returning the writer.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if an error occurs while flushing the
    /// buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::LineWriter;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let file = File::create("poem.txt")?;
    ///
    ///     let writer: LineWriter<File> = LineWriter::new(file);
    ///
    ///     let file: File = writer.into_inner()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn into_inner(self) -> Result<T, IntoInnerError<LineWriter<T>>> {
        self.0.into_inner().map_err(|e| {
            let (e, w) = e.into_parts();
            IntoInnerError(LineWriter(w), e)
        })
    }
}
impl<T: ?Sized + Write> BufWriter<T> {
    /// Gets a reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // we can use reference just like buffer
    /// let reference = buffer.get_ref();
    /// ```
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
    /// Returns a reference to the internally buffered data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // See how many bytes are currently buffered
    /// let bytes_buffered = buf_writer.buffer().len();
    /// ```
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }
    /// Returns the number of bytes the internal buffer can hold without
    /// flushing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // Check the capacity of the inner buffer
    /// let capacity = buf_writer.capacity();
    /// // Calculate how many bytes can be written without flushing
    /// let without_flush = capacity - buf_writer.buffer().len();
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }
    /// Gets a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // we can use reference just like buffer
    /// let reference = buffer.get_mut();
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub(crate) fn flush_buf(&mut self) -> IoResult<()> {
        let mut g = BufGuard::new(&mut self.buf);
        while !g.done() {
            match self.inner.write(g.remaining()) {
                Ok(0) => return Err(IoError::from(ErrorKind::WriteZero)),
                Ok(n) => g.consume(n),
                Err(e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    #[inline]
    pub(crate) fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    #[inline]
    fn spare_capacity(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }
    #[inline]
    fn write_to_buf(&mut self, b: &[u8]) -> usize {
        let n = self.spare_capacity().min(b.len());
        // Bounds checked above.
        unsafe { self.write_to_buffer_unchecked(b.get_unchecked(0..n)) };
        n
    }
    #[cold]
    #[inline(never)]
    fn write_cold(&mut self, b: &[u8]) -> IoResult<usize> {
        if b.len() > self.spare_capacity() {
            self.flush_buf()?;
        }
        if b.len() >= self.buf.capacity() {
            self.get_mut().write(b)
        } else {
            unsafe { self.write_to_buffer_unchecked(b) };
            Ok(b.len())
        }
    }
    #[cold]
    #[inline(never)]
    fn write_all_cold(&mut self, b: &[u8]) -> IoResult<()> {
        if b.len() > self.spare_capacity() {
            self.flush_buf()?;
        }
        if b.len() >= self.buf.capacity() {
            self.get_mut().write_all(b)
        } else {
            unsafe { self.write_to_buffer_unchecked(b) };
            Ok(())
        }
    }

    #[inline]
    unsafe fn write_to_buffer_unchecked(&mut self, b: &[u8]) {
        let (o, n) = (self.buf.len(), b.len());
        unsafe {
            copy_nonoverlapping(b.as_ptr(), self.buf.as_mut_ptr().add(o), n);
            self.buf.set_len(o + n)
        };
    }
}
impl<T: ?Sized + Write> LineWriter<T> {
    /// Gets a reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs::File;
    /// use xrmt_stx::io::LineWriter;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::new(file);
    ///
    ///     let reference = file.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    #[inline]
    fn flush_if_completed(&mut self) -> IoResult<()> {
        match self.0.buffer().last().copied() {
            Some(b'\n') => self.0.flush_buf(),
            _ => Ok(()),
        }
    }
}

impl Drop for BufGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        if self.n > 0 {
            self.b.drain(0..self.n);
        }
    }
}

impl<A: Allocator> Write for Cursor<Vec<u8, A>> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = write(self.pos, &mut self.v, buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.pos += write(self.pos, &mut self.v, buf)? as u64;
        Ok(())
    }
}
impl<A: Allocator> Write for Cursor<Box<[u8], A>> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = write_slice(self.pos, &mut self.v, buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.pos += write_slice(self.pos, &mut self.v, buf)? as u64;
        Ok(())
    }
}
impl<A: Allocator> Write for Cursor<&mut Vec<u8, A>> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = write(self.pos, self.v, buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.pos += write(self.pos, self.v, buf)? as u64;
        Ok(())
    }
}

impl Debug for WriterPanicked {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Display for WriterPanicked {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl CoreError for WriterPanicked {
    #[inline]
    fn source(&self) -> Option<&(dyn CoreError + 'static)> {
        None
    }
}

impl<T> Debug for IntoInnerError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(&self.1, f)
    }
}
impl<T> Display for IntoInnerError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.1, f)
    }
}
impl<T> CoreError for IntoInnerError<T> {
    #[inline]
    fn source(&self) -> Option<&(dyn CoreError + 'static)> {
        Some(&self.1)
    }
}
impl<T> From<IntoInnerError<T>> for Error {
    #[inline]
    fn from(v: IntoInnerError<T>) -> Error {
        v.1
    }
}

impl<W: ?Sized + Write> Drop for BufWriter<W> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}
impl<W: ?Sized + Write> Write for BufWriter<W> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.flush_buf().and_then(|_| self.get_mut().flush())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        if b.len() < self.spare_capacity() {
            unsafe { self.write_to_buffer_unchecked(b) };
            Ok(b.len())
        } else {
            self.write_cold(b)
        }
    }
    #[inline]
    fn write_all(&mut self, b: &[u8]) -> IoResult<()> {
        if b.len() < self.spare_capacity() {
            unsafe { self.write_to_buffer_unchecked(b) };
            Ok(())
        } else {
            self.write_all_cold(b)
        }
    }
}
impl<W: ?Sized + Write + Seek> Seek for BufWriter<W> {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        self.flush_buf()?;
        self.get_mut().seek(pos)
    }
}

impl<W: ?Sized + Write> Write for LineWriter<W> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        let i = match memrchr(b'\n', b) {
            None => {
                self.flush_if_completed()?;
                return self.0.write(b);
            },
            Some(v) => v + 1,
        };
        self.0.flush_buf()?;
        // Bounds checked by 'memrchr'
        let f = self.0.get_mut().write(unsafe { b.get_unchecked(0..i) })?;
        if f == 0 {
            return Ok(0);
        }
        let r = if f >= i {
            if b.len() - f >= self.0.capacity() {
                return Ok(f);
            }
            unsafe { b.get_unchecked(f..) }
        } else if i - f <= self.0.capacity() {
            unsafe { b.get_unchecked(f..i) }
        } else {
            let a = unsafe { b.get_unchecked(f..self.0.capacity()) };
            match memrchr(b'\n', a) {
                Some(v) => unsafe { a.get_unchecked(0..v + 1) },
                None => a,
            }
        };
        Ok(f + self.0.write_to_buf(r))
    }
    fn write_all(&mut self, b: &[u8]) -> IoResult<()> {
        match memrchr(b'\n', b) {
            None => {
                self.flush_if_completed()?;
                self.0.write_all(b)
            },
            Some(i) => {
                let (r, t) = b.split_at(i + 1);
                if self.0.buffer().is_empty() {
                    self.0.get_mut().write_all(r)?;
                } else {
                    self.0.write_all(r)?;
                    self.0.flush_buf()?;
                }
                self.0.write_all(t)
            },
        }
    }
}

impl<A: Allocator> Write for Vec<u8, A> {
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
        self.extend_from_slice(buf);
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}
impl<A: Allocator> Write for VecDeque<u8, A> {
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
        self.extend(buf);
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.extend(buf);
        Ok(())
    }
}
impl<A: Allocator, T: ?Sized + Write> Write for Box<T, A> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        (**self).flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (**self).write(buf)
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        (**self).write_all(buf)
    }
    #[inline]
    fn write_fmt(&mut self, args: Arguments<'_>) -> IoResult<()> {
        (**self).write_fmt(args)
    }
}

fn write<A: Allocator>(pos: u64, b: &mut Vec<u8, A>, buf: &[u8]) -> IoResult<usize> {
    let p = reserve(pos, b, buf.len())?;
    unsafe {
        b.as_mut_ptr().add(p).copy_from(buf.as_ptr(), buf.len());
        if p + buf.len() > b.len() {
            b.set_len(p);
        }
    };
    Ok(buf.len())
}
fn reserve<A: Allocator>(pos: u64, b: &mut Vec<u8, A>, size: usize) -> IoResult<usize> {
    let p = pos as usize;
    if (p as u64) < pos {
        return Err(IoError::from(ErrorKind::OutOfMemory));
    }
    let d = p.saturating_add(size);
    if d > b.capacity() {
        b.reserve(d - b.len());
    }
    if p > b.len() {
        unsafe {
            let d = p - b.len();
            b.spare_capacity_mut().get_unchecked_mut(0..d).fill(MaybeUninit::new(0));
            b.set_len(p);
        };
    }
    Ok(p)
}
