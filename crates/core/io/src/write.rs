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
#![cfg(not(feature = "std"))]

extern crate core;

use core::clone::Clone;
use core::cmp::Ord;
use core::convert::From;
use core::default::Default;
use core::fmt::{write, Arguments};
use core::marker::{Copy, Sized};
use core::mem::replace;
use core::result::Result::{Err, Ok};

use crate::{BorrowedCursor, Cursor, ErrorKind, FmtError, FmtResult, FmtWrite, IoError, IoResult};

/// A writer which will move data into the void.
///
/// This struct is generally created by calling [`sink()`]. Please
/// see the documentation of [`sink()`] for more details.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Sink;

/// A trait for objects which are byte-oriented sinks.
///
/// Implementors of the `Write` trait are sometimes called 'writers'.
///
/// Writers are defined by two required methods, [`write`] and [`flush`]:
///
/// * The [`write`] method will attempt to write some data into the object,
///   returning how many bytes were successfully written.
///
/// * The [`flush`] method is useful for adapters and explicit buffers
///   themselves for ensuring that all buffered data has been pushed out to the
///   'true sink'.
///
/// Writers are intended to be composable with one another. Many implementors
/// throughout [`xrmt_io`] take and provide types which implement the
/// `Write` trait.
///
/// [`write`]: Write::write
/// [`flush`]: Write::flush
/// [`xrmt_io`]: crate
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::fs::File;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let data = b"some bytes";
///
///     let mut pos = 0;
///     let mut buffer = File::create("foo.txt")?;
///
///     while pos < data.len() {
///         let bytes_written = buffer.write(&data[pos..])?;
///         pos += bytes_written;
///     }
///     Ok(())
/// }
/// ```
///
/// The trait also provides convenience methods like [`write_all`], which calls
/// `write` in a loop until its entire input has been written.
///
/// [`write_all`]: Write::write_all
pub trait Write {
    /// Flushes this output stream, ensuring that all intermediately buffered
    /// contents reach their destination.
    ///
    /// # Errors
    ///
    /// It is considered an error if not all bytes could be written due to
    /// I/O errors or EOF being reached.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::io::BufWriter;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = BufWriter::new(File::create("foo.txt")?);
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     buffer.flush()?;
    ///     Ok(())
    /// }
    /// ```
    fn flush(&mut self) -> IoResult<()>;
    /// Writes a buffer into this writer, returning how many bytes were written.
    ///
    /// This function will attempt to write the entire contents of `buf`, but
    /// the entire write might not succeed, or the write may also generate an
    /// error. Typically, a call to `write` represents one attempt to write to
    /// any wrapped object.
    ///
    /// Calls to `write` are not guaranteed to block waiting for data to be
    /// written, and a write which would otherwise block can be indicated
    /// through an [`Err`] variant.
    ///
    /// If this method consumed `n > 0` bytes of `buf` it must return [`Ok(n)`].
    /// If the return value is `Ok(n)` then `n` must satisfy `n <= buf.len()`.
    /// A return value of `Ok(0)` typically means that the underlying object is
    /// no longer able to accept bytes and will likely not be able to in the
    /// future as well, or that the buffer provided is empty.
    ///
    /// # Errors
    ///
    /// Each call to `write` may generate an I/O error indicating that the
    /// operation could not be completed. If an error is returned then no bytes
    /// in the buffer were written to this writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be
    /// written to this writer.
    ///
    /// An error of the [`ErrorKind::Interrupted`] kind is non-fatal and the
    /// write operation should be retried if there is nothing else to do.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // Writes some prefix of the byte string, not necessarily all of it.
    ///     buffer.write(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`Ok(n)`]: Ok
    fn write(&mut self, buf: &[u8]) -> IoResult<usize>;

    /// Creates a "by reference" adapter for this instance of `Write`.
    ///
    /// The returned adapter also implements `Write` and will simply borrow this
    /// current writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::Write;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     let reference = buffer.by_ref();
    ///
    ///     // we can use reference just like our original buffer
    ///     reference.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }

    /// Determines if this `Write`r has an efficient `write_vectored`
    /// implementation.
    ///
    /// If a `Write`r does not override the default `write_vectored`
    /// implementation, code using it may want to avoid the method all together
    /// and coalesce writes into a single buffer for higher performance.
    ///
    /// The default implementation returns `false`.
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    /// Attempts to write an entire buffer into this writer.
    ///
    /// This method will continuously call [`write`] until there is no more data
    /// to be written or an error of non-[`ErrorKind::Interrupted`] kind is
    /// returned. This method will not return until the entire buffer has been
    /// successfully written or such an error occurs. The first error that is
    /// not of [`ErrorKind::Interrupted`] kind generated from this method will
    /// be returned.
    ///
    /// If the buffer contains no data, this will never call [`write`].
    ///
    /// # Errors
    ///
    /// This function will return the first error of
    /// non-[`ErrorKind::Interrupted`] kind that [`write`] returns.
    ///
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(IoError::from(ErrorKind::WriteZero)),
                Ok(n) => buf = &buf[n..], // Keep the bounds check here as we don't know if the return result is valid
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    /// Writes a formatted string into this writer, returning any error
    /// encountered.
    ///
    /// This method is primarily used to interface with the
    /// [`format_args!()`] macro, and it is rare that this should
    /// explicitly be called. The [`write!()`] macro should be favored to
    /// invoke this method instead.
    ///
    /// This function internally uses the [`write_all`] method on
    /// this trait and hence will continuously write data so long as no errors
    /// are received. This also means that partial writes are not indicated in
    /// this signature.
    ///
    /// [`write!()`]: core::write!
    /// [`write_all`]: Write::write_all
    ///
    /// # Errors
    ///
    /// This function will return any I/O error reported while formatting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // this call
    ///     write!(buffer, "{:.*}", 2, 1.234567)?;
    ///     // turns into this:
    ///     buffer.write_fmt(format_args!("{:.*}", 2, 1.234567))?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn write_fmt(&mut self, args: Arguments<'_>) -> IoResult<()> {
        Adapter::new(self).write(args)
    }
}

struct Adapter<'a, T: ?Sized + 'a> {
    inner: &'a mut T,
    error: IoResult<()>,
}

impl<'a, T: ?Sized + Write> Adapter<'a, T> {
    #[inline]
    fn new(v: &'a mut T) -> Adapter<'a, T> {
        Adapter { inner: v, error: Ok(()) }
    }

    #[inline]
    fn write(mut self, args: Arguments<'_>) -> IoResult<()> {
        if write(&mut self, args).is_err() {
            return if self.error.is_err() {
                self.error
            } else {
                Err(IoError::from(ErrorKind::InvalidInput))
            };
        }
        Ok(())
    }
}

impl Copy for Sink {}
impl Clone for Sink {
    #[inline]
    fn clone(&self) -> Sink {
        Sink
    }
}
impl Write for Sink {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        Ok(b.len())
    }
}
impl Write for &Sink {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        Ok(b.len())
    }
}
impl Default for Sink {
    #[inline]
    fn default() -> Sink {
        Sink
    }
}

impl<'a> Write for BorrowedCursor<'a, u8> {
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        if self.write(buf)? < buf.len() {
            Err(IoError::from(ErrorKind::UnexpectedEof))
        } else {
            Ok(())
        }
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = buf.len().min(self.capacity());
        // Bounds check was already done, compiler might not know.
        self.append(unsafe { buf.get_unchecked(0..n) });
        Ok(n)
    }
}

impl Write for &mut [u8] {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        let n = b.len().min(self.len());
        let (a, v) = unsafe { replace(self, &mut []).split_at_mut_unchecked(n) };
        // Bounds check was already done, compiler might not know.
        a.copy_from_slice(unsafe { b.get_unchecked(0..n) });
        *self = v;
        Ok(n)
    }
    #[inline]
    fn write_all(&mut self, b: &[u8]) -> IoResult<()> {
        if self.write(b)? == b.len() {
            Ok(())
        } else {
            Err(IoError::from(ErrorKind::WriteZero))
        }
    }
}
impl<T: ?Sized + Write> Write for &mut T {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        (**self).flush()
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }
    #[inline]
    fn write(&mut self, b: &[u8]) -> IoResult<usize> {
        (**self).write(b)
    }
    #[inline]
    fn write_all(&mut self, b: &[u8]) -> IoResult<()> {
        (**self).write_all(b)
    }
    #[inline]
    fn write_fmt(&mut self, args: Arguments<'_>) -> IoResult<()> {
        (**self).write_fmt(args)
    }
}
impl<T: ?Sized + Write> FmtWrite for Adapter<'_, T> {
    #[inline]
    fn write_str(&mut self, v: &str) -> FmtResult {
        match self.inner.write_all(v.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.error = Err(e);
                Err(FmtError)
            },
        }
    }
}

impl Write for Cursor<&mut [u8]> {
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
        let n = write_slice(self.pos, &mut self.v, buf)?;
        self.pos += n as u64;
        if n < buf.len() {
            Err(IoError::from(ErrorKind::UnexpectedEof))
        } else {
            Ok(())
        }
    }
}
impl<const N: usize> Write for Cursor<[u8; N]> {
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
        let n = write_slice(self.pos, &mut self.v, buf)?;
        self.pos += n as u64;
        if n < buf.len() {
            Err(IoError::from(ErrorKind::UnexpectedEof))
        } else {
            Ok(())
        }
    }
}

/// Creates an instance of a writer which will successfully consume all data.
///
/// All calls to [`write`] on the returned instance will return
/// [`Ok(buf.len())`] and the contents of the buffer will not be inspected.
///
/// [`write`]: Write::write
///
/// # Examples
///
/// ```rust
/// use xrmt_stx::io::{self, Write};
///
/// let buffer = vec![1, 2, 3, 5, 8];
/// let num_bytes = io::sink().write(&buffer).unwrap();
/// assert_eq!(num_bytes, 5);
/// ```
#[inline]
pub const fn sink() -> Sink {
    Sink
}

#[inline]
pub(crate) fn write_slice(pos: u64, v: &mut [u8], b: &[u8]) -> IoResult<usize> {
    // Bounds check was already done, compiler might not know.
    Ok(unsafe { v.get_unchecked_mut((pos.min(v.len() as u64) as usize)..).write(b)? })
}
