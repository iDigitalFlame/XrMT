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
use core::cmp::{Eq, Ord, PartialEq};
use core::convert::{AsMut, AsRef, From};
use core::default::Default;
use core::fmt::Arguments;
use core::iter::Iterator;
use core::marker::{Copy, Sized};
use core::option::Option::{self, None, Some};
use core::ptr::copy_nonoverlapping;
use core::result::Result::{Err, Ok};
use core::slice::from_mut;
use core::str::from_utf8;

use crate::{BorrowedBuf, BorrowedCursor, ErrorKind, IoError, IoResult, Seek, SeekFrom, Write};

/// `Empty` ignores any data written via [`Write`], and will always be empty
/// (returning zero bytes) when read via [`Read`].
///
/// This struct is generally created by calling [`empty()`]. Please
/// see the documentation of [`empty()`] for more details.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Empty;
/// A reader which yields one byte over and over and over and over and over
/// and...
///
/// This struct is generally created by calling [`repeat()`]. Please
/// see the documentation of [`repeat()`] for more details.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Repeat(u8);
/// Reader adapter which limits the bytes read from an underlying reader.
///
/// This struct is generally created by calling [`take`] on a reader.
/// Please see the documentation of [`take`] for more details.
///
/// [`take`]: Read::take
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Take<T> {
    pub(crate) v: T,
    pub(crate) n: u64,
    r:            u64,
}
/// A `Cursor` wraps an in-memory buffer and provides it with a
/// [`Seek`] implementation.
///
/// `Cursor`s are used with in-memory buffers, anything implementing
/// <code>[AsRef]<\[u8]></code>, to allow them to implement [`Read`] and/or
/// [`Write`], allowing these buffers to be used anywhere you might use a reader
/// or writer that does actual I/O.
///
/// The standard library implements some I/O traits on various types which
/// are commonly used as a buffer, like <code>Cursor<`Vec<u8>`></code> and
/// <code>Cursor<[&\[u8\]][bytes]></code>.
///
/// # Examples
///
/// We may want to write bytes to a `File` in our production
/// code, but use an in-memory buffer in our tests. We can do this with
/// `Cursor`:
///
/// [bytes]: core::slice "slice"
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::io::{self, SeekFrom};
/// use xrmt_stx::fs::File;
///
/// // a library function we've written
/// fn write_ten_bytes_at_end<W: Write + Seek>(mut writer: W) -> IoResult<()> {
///     writer.seek(SeekFrom::End(-10))?;
///
///     for i in 0..10 {
///         writer.write(&[i])?;
///     }
///
///     // all went well
///     Ok(())
/// }
///
/// # fn foo() -> IoResult<()> {
/// // Here's some code that uses this library function.
/// //
/// // We might want to use a BufReader here for efficiency, but let's
/// // keep this example focused.
/// let mut file = File::create("foo.txt")?;
/// // First, we need to allocate 10 bytes to be able to write into.
/// file.set_len(10)?;
///
/// write_ten_bytes_at_end(&mut file)?;
/// # Ok(())
/// # }
///
/// // now let's write a test
/// #[test]
/// fn test_writes_bytes() {
///     // setting up a real File is much slower than an in-memory buffer,
///     // let's use a cursor instead
///     use xrmt_stx::io::Cursor;
///     let mut buff = Cursor::new(vec![0; 15]);
///
///     write_ten_bytes_at_end(&mut buff).unwrap();
///
///     assert_eq!(&buff.get_ref()[5..15], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
/// }
/// ```
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Cursor<T> {
    pub(crate) v:   T,
    pub(crate) pos: u64,
}
/// Adapter to chain together two readers.
///
/// This struct is generally created by calling [`chain`] on a reader.
/// Please see the documentation of [`chain`] for more details.
///
/// [`chain`]: Read::chain
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Chain<T, U> {
    pub(crate) d: bool,
    pub(crate) f: T,
    pub(crate) s: U,
}
/// An iterator over `u8` values of a reader.
///
/// This struct is generally created by calling [`bytes`] on a reader.
/// Please see the documentation of [`bytes`] for more details.
///
/// [`bytes`]: Read::bytes
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Bytes<T>(pub(super) T);

/// The `Read` trait allows for reading bytes from a source.
///
/// Implementors of the `Read` trait are called 'readers'.
///
/// Readers are defined by one required method, [`read()`]. Each call to
/// [`read()`] will attempt to pull bytes from this source into a provided
/// buffer. A number of other methods are implemented in terms of [`read()`],
/// giving implementors a number of ways to read bytes while only needing to
/// implement a single method.
///
/// Readers are intended to be composable with one another. Many implementors
/// throughout [`xrmt_io`] take and provide types which implement the
/// `Read` trait.
///
/// Please note that each call to [`read()`] may involve a system call, and
/// therefore, using something that implements [`BufRead`], such as
/// [`BufReader`], will be more efficient.
///
/// Repeated calls to the reader use the same cursor, so for example
/// calling `read_to_end` twice on a `File` will only return the file's
/// contents once. It's recommended to first call `rewind()` in that case.
///
/// # Examples
///
/// `File`s implement `Read`:
///
/// ```no_run
/// use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::fs::File;
///
/// fn main() -> IoResult<()> {
///     let mut f = File::open("foo.txt")?;
///     let mut buffer = [0; 10];
///
///     // read up to 10 bytes
///     f.read(&mut buffer)?;
///
///     let mut buffer = Vec::new();
///     // read the whole file
///     f.read_to_end(&mut buffer)?;
///
///     // read into a String, so that you don't need to do the conversion.
///     let mut buffer = String::new();
///     f.read_to_string(&mut buffer)?;
///
///     // and more! See the other methods for more details.
///     Ok(())
/// }
/// ```
///
/// Read from [`&str`] because [`&[u8]`][prim@slice] implements `Read`:
///
/// ```no_run
/// # use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::io::prelude::*;
///
/// fn main() -> IoResult<()> {
///     let mut b = "This string will be read".as_bytes();
///     let mut buffer = [0; 10];
///
///     // read up to 10 bytes
///     b.read(&mut buffer)?;
///
///     // etc... it works exactly as a File does!
///     Ok(())
/// }
/// ```
///
/// [`read()`]: Read::read
/// [`&str`]: prim@str
/// [`xrmt_io`]: crate
/// [`BufRead`]: crate::BufRead
/// [`BufReader`]: crate::BufReader
pub trait Read {
    /// Pull some bytes from this source into the specified buffer, returning
    /// how many bytes were read.
    ///
    /// This function does not provide any guarantees about whether it blocks
    /// waiting for data, but if an object needs to block for a read and cannot,
    /// it will typically signal this via an [`Err`] return value.
    ///
    /// If the return value of this method is [`Ok(n)`], then implementations
    /// must guarantee that `0 <= n <= buf.len()`. A nonzero `n` value
    /// indicates that the buffer `buf` has been filled in with `n` bytes of
    /// data from this source. If `n` is `0`, then it can indicate one of
    /// two scenarios:
    ///
    /// 1. This reader has reached its "end of file" and will likely no longer
    ///    be able to produce bytes. Note that this does not mean that the
    ///    reader will *always* no longer be able to produce bytes. As an
    ///    example, on Linux, this method will call the `recv` syscall for a
    ///    `TcpStream`, where returning zero indicates the connection was shut
    ///    down correctly. While for `File`, it is possible to reach the end of
    ///    file and get zero as result, but if more data is appended to the
    ///    file, future calls to `read` will return more data.
    /// 2. The buffer specified was 0 bytes in length.
    ///
    /// It is not an error if the returned value `n` is smaller than the buffer
    /// size, even when the reader is not at the end of the stream yet.
    /// This may happen for example because fewer bytes are actually available
    /// right now (e. g. being close to end-of-file) or because read() was
    /// interrupted by a signal.
    ///
    /// As this trait is safe to implement, callers in unsafe code cannot rely
    /// on `n <= buf.len()` for safety.
    /// Extra care needs to be taken when `unsafe` functions are used to access
    /// the read bytes. Callers have to ensure that no unchecked
    /// out-of-bounds accesses are possible even if `n > buf.len()`.
    ///
    /// *Implementations* of this method can make no assumptions about the
    /// contents of `buf` when this function is called. It is recommended
    /// that implementations only write data to `buf` instead of reading its
    /// contents.
    ///
    /// Correspondingly, however, *callers* of this method in unsafe code must
    /// not assume any guarantees about how the implementation uses `buf`.
    /// The trait is safe to implement, so it is possible that the code
    /// that's supposed to write to the buffer might also read from it. It
    /// is your responsibility to make sure that `buf` is initialized before
    /// calling `read`. Calling `read` with an uninitialized `buf` (of the kind
    /// one obtains via [`MaybeUninit<T>`]) is not safe, and can lead to
    /// undefined behavior.
    ///
    /// [`MaybeUninit<T>`]: core::mem::MaybeUninit
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. If an error is returned then it must be
    /// guaranteed that no bytes were read.
    ///
    /// An error of the [`ErrorKind::Interrupted`] kind is non-fatal and the
    /// read operation should be retried if there is nothing else to do.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // read up to 10 bytes
    ///     let n = f.read(&mut buffer[..])?;
    ///
    ///     println!("The bytes: {:?}", &buffer[..n]);
    ///     Ok(())
    /// }
    /// ```
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize>;

    /// Transforms this `Read` instance to an [`Iterator`] over its bytes.
    ///
    /// The returned type implements [`Iterator`] where the [`Item`] is
    /// <code>[Result]<[u8], [Error]></code>.
    /// The yielded item is [`Ok`] if a byte was successfully read and [`Err`]
    /// otherwise. EOF is mapped to returning [`None`] from this iterator.
    ///
    /// The default implementation calls `read` for each byte,
    /// which can be very inefficient for data that's not in memory,
    /// such as `File`. Consider using a [`BufReader`] in such cases.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// [`Item`]: core::iter::Iterator::Item
    /// [Result]: core::result::Result "Result"
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let f = BufReader::new(File::open("foo.txt")?);
    ///
    ///     for byte in f.bytes() {
    ///         println!("{}", byte?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    /// [`BufReader`]: crate::BufReader
    #[inline]
    fn bytes(self) -> Bytes<Self>
    where Self: Sized {
        Bytes { 0: self }
    }
    /// Creates a "by reference" adaptor for this instance of `Read`.
    ///
    /// The returned adapter also implements `Read` and will simply borrow this
    /// current reader.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::Read;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///     let mut other_buffer = Vec::new();
    ///
    ///     {
    ///         let reference = f.by_ref();
    ///
    ///         // read at most 5 bytes
    ///         reference.take(5).read_to_end(&mut buffer)?;
    ///
    ///     } // drop our &mut reference so we can use f again
    ///
    ///     // original file still usable, read the rest
    ///     f.read_to_end(&mut other_buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where Self: Sized {
        self
    }
    /// Creates an adapter which will read at most `limit` bytes from it.
    ///
    /// This function returns a new instance of `Read` which will read at most
    /// `limit` bytes, after which it will always return EOF ([`Ok(0)`]). Any
    /// read errors will not count towards the number of bytes read and future
    /// calls to [`read()`] may succeed.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// [`read()`]: Read::read
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 5];
    ///
    ///     // read at most five bytes
    ///     let mut handle = f.take(5);
    ///
    ///     handle.read(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn take(self, limit: u64) -> Take<Self>
    where Self: Sized {
        Take { v: self, n: limit, r: limit }
    }
    /// Creates an adapter which will chain this stream with another.
    ///
    /// The returned `Read` instance will first read all bytes from this object
    /// until EOF is encountered. Afterwards the output is equivalent to the
    /// output of `next`.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let f1 = File::open("foo.txt")?;
    ///     let f2 = File::open("bar.txt")?;
    ///
    ///     let mut handle = f1.chain(f2);
    ///     let mut buffer = String::new();
    ///
    ///     // read the value into a String. We could use any Read method here,
    ///     // this is just one example.
    ///     handle.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn chain<T: Read>(self, next: T) -> Chain<Self, T>
    where Self: Sized {
        Chain { d: false, f: self, s: next }
    }

    /// Determines if this `Read`er has an efficient `read_vectored`
    /// implementation.
    ///
    /// If a `Read`er does not override the default `read_vectored`
    /// implementation, code using it may want to avoid the method all together
    /// and coalesce writes into a single buffer for higher performance.
    ///
    /// The default implementation returns `false`.
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    /// Reads the exact number of bytes required to fill `buf`.
    ///
    /// This function reads as many bytes as necessary to completely fill the
    /// specified buffer `buf`.
    ///
    /// *Implementations* of this method can make no assumptions about the
    /// contents of `buf` when this function is called. It is recommended
    /// that implementations only write data to `buf` instead of reading its
    /// contents. The documentation on [`read`] has a more detailed
    /// explanation of this subject.
    ///
    /// # Errors
    ///
    /// If this function encounters an error of the kind
    /// [`ErrorKind::Interrupted`] then the error is ignored and the operation
    /// will continue.
    ///
    /// If this function encounters an "end of file" before completely filling
    /// the buffer, it returns an error of the kind
    /// [`ErrorKind::UnexpectedEof`]. The contents of `buf` are unspecified
    /// in this case.
    ///
    /// If any other read error is encountered then this function immediately
    /// returns. The contents of `buf` are unspecified in this case.
    ///
    /// If this function returns an error, it is unspecified how many bytes it
    /// has read, but it will never read more than would be necessary to
    /// completely fill the buffer.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// [`read`]: Read::read
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // read exactly 10 bytes
    ///     f.read_exact(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        read_exact(self, buf)
    }
    /// Pull some bytes from this source into the specified buffer.
    ///
    /// This is equivalent to the [`read`](Read::read) method, except that it is
    /// passed a [`BorrowedCursor`] rather than `[u8]` to allow use
    /// with uninitialized buffers. The new data will be appended to any
    /// existing contents of `buf`.
    ///
    /// The default implementation delegates to `read`.
    ///
    /// This method makes it possible to return both data and an error but it is
    /// advised against.
    #[inline]
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_, u8>) -> IoResult<()> {
        let n = self.read(buf.ensure_init())?;
        buf.advance_checked(n);
        Ok(())
    }
    /// Reads the exact number of bytes required to fill `cursor`.
    ///
    /// This is similar to the [`read_exact`](Read::read_exact) method, except
    /// that it is passed a [`BorrowedCursor`] rather than `[u8]` to allow use
    /// with uninitialized buffers.
    ///
    /// # Errors
    ///
    /// If this function encounters an error of the kind
    /// [`ErrorKind::Interrupted`] then the error is ignored and the
    /// operation will continue.
    ///
    /// If this function encounters an "end of file" before completely filling
    /// the buffer, it returns an error of the kind
    /// [`ErrorKind::UnexpectedEof`].
    ///
    /// If any other read error is encountered then this function immediately
    /// returns.
    ///
    /// If this function returns an error, all bytes read will be appended to
    /// `cursor`.
    #[inline]
    fn read_buf_exact(&mut self, cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        read_buf_exact(self, cur)
    }

    /// Reads all bytes until EOF in this source, placing them into `buf`.
    ///
    /// All bytes read from this source will be appended to the specified buffer
    /// `buf`. This function will continuously call [`read()`] to append more
    /// data to `buf` until [`read()`] returns either [`Ok(0)`] or an error
    /// of non-[`ErrorKind::Interrupted`] kind.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Errors
    ///
    /// If this function encounters an error of the kind
    /// [`ErrorKind::Interrupted`] then the error is ignored and the operation
    /// will continue.
    ///
    /// If any other read error is encountered then this function immediately
    /// returns. Any bytes which have already been read will be appended to
    /// `buf`.
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// [`read()`]: Read::read
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///
    ///     // read the whole file
    ///     f.read_to_end(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// (See also the `fs::read` convenience function for reading
    /// from a file.)
    ///
    /// ## Implementing `read_to_end`
    ///
    /// When implementing the `io::Read` trait, it is recommended to allocate
    /// memory using [`Vec::try_reserve`]. However, this behavior is not
    /// guaranteed by all implementations, and `read_to_end` may not handle
    /// out-of-memory situations gracefully.
    ///
    /// ```no_run
    /// # use xrmt_stx::io::{self, BufRead};
    /// # struct Example { example_datasource: io::Empty } impl Example {
    /// # fn get_some_data_for_the_example(&self) -> &'static [u8] { &[] }
    /// fn read_to_end(&mut self, dest_vec: &mut Vec<u8>) -> IoResult<usize> {
    ///     let initial_vec_len = dest_vec.len();
    ///     loop {
    ///         let src_buf = self.example_datasource.fill_buf()?;
    ///         if src_buf.is_empty() {
    ///             break;
    ///         }
    ///         dest_vec.try_reserve(src_buf.len())?;
    ///         dest_vec.extend_from_slice(src_buf);
    ///
    ///         // Any irreversible side effects should happen after `try_reserve` succeeds,
    ///         // to avoid losing data on allocation error.
    ///         let read = src_buf.len();
    ///         self.example_datasource.consume(read);
    ///     }
    ///     Ok(dest_vec.len() - initial_vec_len)
    /// }
    /// # }
    /// ```
    /// [`Vec::try_reserve`]: crate::Vec::try_reserve
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, buf: &mut crate::Vec<u8>) -> IoResult<usize> {
        crate::read_to_end(self, buf)
    }
    /// Reads all bytes until EOF in this source, appending them to `buf`.
    ///
    /// If successful, this function returns the number of bytes which were read
    /// and appended to `buf`.
    ///
    /// # Errors
    ///
    /// If the data in this stream is *not* valid UTF-8 then an error is
    /// returned and `buf` is unchanged.
    ///
    /// See [`read_to_end`] for other error semantics.
    ///
    /// [`read_to_end`]: Read::read_to_end
    ///
    /// # Examples
    ///
    /// `File`s implement `Read`:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = String::new();
    ///
    ///     f.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// (See also the `fs::read_to_string` convenience function for
    /// reading from a file.)
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_string(&mut self, buf: &mut crate::String) -> IoResult<usize> {
        crate::read_string(self, buf)
    }
}

impl<T> Take<T> {
    /// Returns the number of bytes that can be read before this instance will
    /// return EOF.
    ///
    /// # Note
    ///
    /// This instance may reach `EOF` after reading fewer bytes than indicated
    /// by this method if the underlying [`Read`] instance reaches EOF.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // read at most five bytes
    ///     let handle = f.take(5);
    ///
    ///     println!("limit: {}", handle.limit());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn limit(&self) -> u64 {
        self.n
    }
    /// Gets a reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.v
    }
    /// Consumes the `Take`, returning the wrapped reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        self.v
    }
    /// Returns the number of bytes read so far.
    #[inline]
    pub fn position(&self) -> u64 {
        self.r - self.n
    }
    /// Gets a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.v
    }
    /// Sets the number of bytes that can be read before this instance will
    /// return EOF. This is the same as constructing a new `Take` instance, so
    /// the amount of bytes read and the previous limit value don't matter when
    /// calling this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // read at most five bytes
    ///     let mut handle = f.take(5);
    ///     handle.set_limit(10);
    ///
    ///     assert_eq!(handle.limit(), 10);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn set_limit(&mut self, limit: u64) {
        self.n = limit;
    }
}
impl<T> Cursor<T> {
    /// Creates a new cursor wrapping the provided underlying in-memory buffer.
    ///
    /// Cursor initial position is `0` even if underlying buffer (e.g., `Vec`)
    /// is not empty. So writing to cursor starts with overwriting `Vec`
    /// content, not with appending to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    /// ```
    #[inline]
    pub const fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0u64, v: inner }
    }

    /// Gets a reference to the underlying value in this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_ref();
    /// ```
    #[inline]
    pub const fn get_ref(&self) -> &T {
        &self.v
    }
    /// Returns the current position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::io::SeekFrom;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.seek(SeekFrom::Current(2)).unwrap();
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.seek(SeekFrom::Current(-1)).unwrap();
    /// assert_eq!(buff.position(), 1);
    /// ```
    #[inline]
    pub const fn position(&self) -> u64 {
        self.pos
    }
    /// Gets a mutable reference to the underlying value in this cursor.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying value as it may corrupt this cursor's position.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let mut buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_mut();
    /// ```
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.v
    }
    /// Sets the position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.set_position(4);
    /// assert_eq!(buff.position(), 4);
    /// ```
    #[inline]
    pub const fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }

    /// Consumes this cursor, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let vec = buff.into_inner();
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        self.v
    }
}
impl<T, U> Chain<T, U> {
    /// Consumes the `Chain`, returning the wrapped readers.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn into_inner(self) -> (T, U) {
        (self.f, self.s)
    }
    /// Gets references to the underlying readers in this `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.f, &self.s)
    }
    /// Gets mutable references to the underlying readers in this `Chain`.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let mut chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.f, &mut self.s)
    }
}
impl<T: AsRef<[u8]>> Cursor<T> {
    /// Splits the underlying slice at the cursor position and returns them.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.split(), ([].as_slice(), [1, 2, 3, 4, 5].as_slice()));
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.split(), ([1, 2].as_slice(), [3, 4, 5].as_slice()));
    ///
    /// buff.set_position(6);
    /// assert_eq!(buff.split(), ([1, 2, 3, 4, 5].as_slice(), [].as_slice()));
    /// ```
    #[inline]
    pub fn split(&self) -> (&[u8], &[u8]) {
        let i = self.v.as_ref();
        let n = self.pos.min(i.len() as u64);
        i.split_at(n as usize)
    }
}
impl<T: AsMut<[u8]>> Cursor<T> {
    /// Splits the underlying slice at the cursor position and returns them
    /// mutably.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.split_mut(), ([].as_mut_slice(), [1, 2, 3, 4, 5].as_mut_slice()));
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.split_mut(), ([1, 2].as_mut_slice(), [3, 4, 5].as_mut_slice()));
    ///
    /// buff.set_position(6);
    /// assert_eq!(buff.split_mut(), ([1, 2, 3, 4, 5].as_mut_slice(), [].as_mut_slice()));
    /// ```
    #[inline]
    pub fn split_mut(&mut self) -> (&mut [u8], &mut [u8]) {
        let i = self.v.as_mut();
        let p = self.pos.min(i.len() as u64);
        i.split_at_mut(p as usize)
    }
}

impl Read for Empty {
    #[inline]
    fn read(&mut self, _buf: &mut [u8]) -> IoResult<usize> {
        Ok(0)
    }
    #[inline]
    fn read_buf(&mut self, _cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        Ok(())
    }
}
impl Seek for Empty {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        Ok(0)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        Ok(0)
    }
    #[inline]
    fn seek(&mut self, _pos: SeekFrom) -> IoResult<u64> {
        Ok(0)
    }
}
impl Copy for Empty {}
impl Clone for Empty {
    #[inline]
    fn clone(&self) -> Empty {
        Empty
    }
}
impl Write for Empty {
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
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write_fmt(&mut self, _args: Arguments<'_>) -> IoResult<()> {
        Ok(())
    }
}
impl Write for &Empty {
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
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn write_fmt(&mut self, _args: Arguments<'_>) -> IoResult<()> {
        Ok(())
    }
}
impl Default for Empty {
    #[inline]
    fn default() -> Empty {
        Empty
    }
}

impl Read for Repeat {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        buf.fill(self.0);
        Ok(buf.len())
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        buf.fill(self.0);
        Ok(())
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, _: &mut crate::Vec<u8>) -> IoResult<usize> {
        Err(IoError::from(ErrorKind::OutOfMemory))
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_string(&mut self, _: &mut crate::String) -> IoResult<usize> {
        Err(IoError::from(ErrorKind::OutOfMemory))
    }
    #[inline]
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_, u8>) -> IoResult<()> {
        unsafe {
            buf.as_mut().write_filled(self.0);
            buf.advance_checked(buf.capacity());
        }
        Ok(())
    }
    #[inline]
    fn read_buf_exact(&mut self, buf: BorrowedCursor<'_, u8>) -> IoResult<()> {
        self.read_buf(buf)
    }
}

impl Read for &[u8] {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let n = buf.len().min(self.len());
        let (a, b) = unsafe { self.split_at_unchecked(n) };
        // Bounds were already checked, compiler might not know.
        if n == 1 {
            unsafe { *buf.get_unchecked_mut(0) = *a.get_unchecked(0) };
        } else {
            unsafe { copy_nonoverlapping(a.as_ptr(), buf.as_mut_ptr(), n) };
        }
        *self = b;
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        if buf.len() > self.len() {
            *self = &self[self.len()..];
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        let (a, b) = unsafe { self.split_at_unchecked(buf.len()) };
        // Bounds were already checked, compiler might not know.
        if buf.len() == 1 {
            unsafe { *buf.get_unchecked_mut(0) = *a.get_unchecked(0) };
        } else {
            buf.copy_from_slice(a);
        }
        *self = b;
        Ok(())
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, buf: &mut crate::Vec<u8>) -> IoResult<usize> {
        let n = self.len();
        buf.try_reserve(n)?;
        buf.extend_from_slice(*self);
        // Will be in len bounds
        unsafe { *self = self.get_unchecked(n..) };
        Ok(n)
    }
    #[inline]
    fn read_buf(&mut self, mut cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        let n = cur.capacity().min(self.len());
        let (a, b) = unsafe { self.split_at_unchecked(n) };
        cur.append(a);
        *self = b;
        Ok(())
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_string(&mut self, buf: &mut crate::String) -> IoResult<usize> {
        let c = str::from_utf8(self).map_err(|_| IoError::from(ErrorKind::InvalidInput))?;
        let n = self.len();
        buf.try_reserve(n)?;
        buf.push_str(c);
        // Will be in len bounds
        unsafe { *self = self.get_unchecked(n..) };
        Ok(n)
    }
    #[inline]
    fn read_buf_exact(&mut self, mut cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        if cur.capacity() > self.len() {
            cur.append(*self);
            *self = &self[self.len()..];
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        let (a, b) = unsafe { self.split_at_unchecked(cur.capacity()) };
        cur.append(a);
        *self = b;
        Ok(())
    }
}
impl<T: ?Sized + Read> Read for &mut T {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        (**self).is_read_vectored()
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (**self).read(buf)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        (**self).read_exact(buf)
    }
    #[inline]
    fn read_buf(&mut self, cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        (**self).read_buf(cur)
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, buf: &mut crate::Vec<u8>) -> IoResult<usize> {
        (**self).read_to_end(buf)
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_string(&mut self, buf: &mut crate::String) -> IoResult<usize> {
        (**self).read_to_string(buf)
    }
    #[inline]
    fn read_buf_exact(&mut self, cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        (**self).read_buf_exact(cur)
    }
}

impl<T: Read> Iterator for Bytes<T> {
    type Item = IoResult<u8>;

    fn next(&mut self) -> Option<IoResult<u8>> {
        let mut v = 0u8;
        loop {
            return match self.0.read(from_mut(&mut v)) {
                Ok(0) => None,
                Ok(..) => Some(Ok(v)),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => Some(Err(e)),
            };
        }
    }
}

impl<T: Read> Read for Take<T> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.n == 0 {
            return Ok(0);
        }
        let m = (buf.len() as u64).min(self.n) as usize;
        // Bounds were already checked, compiler might not know.
        let n = self.v.read(unsafe { buf.get_unchecked_mut(0..m) })?;
        self.n -= n as u64;
        Ok(n)
    }
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_, u8>) -> IoResult<()> {
        if self.n == 0 {
            return Ok(());
        }
        if self.n <= buf.capacity() as u64 {
            let n = self.n.min(usize::MAX as u64) as usize;
            let i = buf.is_init();
            let mut b = BorrowedBuf::from(unsafe { buf.as_mut().get_unchecked_mut(0..n) });
            if i {
                unsafe { b.set_init() };
            }
            let mut c = b.unfilled();
            let r = self.v.read_buf(c.reborrow());
            let v = b.len();
            if b.is_init() {
                unsafe {
                    buf.as_mut().get_unchecked_mut(n..).write_filled(0);
                    buf.set_init();
                }
            }
            unsafe { buf.advance(v) };
            self.n -= v as u64;
            r?;
        } else {
            let w = buf.written();
            let r = self.v.read_buf(buf.reborrow());
            self.n -= (buf.written() - w) as u64;
            r?;
        }
        Ok(())
    }
}
impl<T: Seek> Seek for Take<T> {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        Ok(self.r)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        Ok(self.position())
    }
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let i = match pos {
            SeekFrom::End(v) => self.r.checked_add_signed(v),
            SeekFrom::Start(v) => Some(v),
            SeekFrom::Current(v) => self.position().checked_add_signed(v),
        }
        .and_then(|v| if v <= self.r { Some(v) } else { None })
        .ok_or_else(|| IoError::from(ErrorKind::InvalidInput))?;
        while i != self.position() {
            let (c, v) = match i.checked_signed_diff(self.position()) {
                Some(v) => (true, v),
                None if i > self.position() => (false, i64::MAX),
                None => (false, i64::MIN),
            };
            self.v.seek_relative(v)?;
            self.n = self.n.wrapping_sub(v as u64);
            if c {
                break;
            }
        }
        Ok(i)
    }
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        if !self.position().checked_add_signed(offset).is_some_and(|p| p <= self.r) {
            return Err(IoError::from(ErrorKind::InvalidInput));
        }
        self.v.seek_relative(offset)?;
        self.n = self.n.wrapping_sub(offset as u64);
        Ok(())
    }
}

impl<T: Read, U: Read> Read for Chain<T, U> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if !self.d {
            match self.f.read(buf)? {
                0 if !buf.is_empty() => self.d = true,
                n => return Ok(n),
            }
        }
        self.s.read(buf)
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, buf: &mut crate::Vec<u8>) -> IoResult<usize> {
        let mut n = 0usize;
        if !self.d {
            n += self.f.read_to_end(buf)?;
            self.d = true;
        }
        Ok(n + self.s.read_to_end(buf)?)
    }
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_, u8>) -> IoResult<()> {
        if buf.capacity() == 0 {
            return Ok(());
        }
        if !self.d {
            let n = buf.written();
            self.f.read_buf(buf.reborrow())?;
            if buf.written() != n {
                return Ok(());
            }
            self.d = true;
        }
        self.s.read_buf(buf)
    }
}

impl<T: Eq> Eq for Cursor<T> {}
impl<T: Clone> Clone for Cursor<T> {
    #[inline]
    fn clone(&self) -> Cursor<T> {
        Cursor {
            v:   self.v.clone(),
            pos: self.pos,
        }
    }
}
impl<T: Default> Default for Cursor<T> {
    #[inline]
    fn default() -> Cursor<T> {
        Cursor { v: T::default(), pos: 0u64 }
    }
}
impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        Ok(self.v.as_ref().len() as u64)
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        Ok(self.pos)
    }
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let (b, o) = match pos {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            },
            SeekFrom::End(n) => (self.v.as_ref().len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };
        match b.checked_add_signed(o) {
            Some(n) => {
                self.pos = n;
                Ok(self.pos)
            },
            None => Err(IoError::from(ErrorKind::InvalidInput)),
        }
    }
}
impl<T: AsRef<[u8]>> Read for Cursor<T> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let n = Cursor::split(self).1.read(buf)?;
        self.pos += n as u64;
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        let r = Cursor::split(self).1.read_exact(buf);
        match r {
            Ok(_) => self.pos += buf.len() as u64,
            Err(_) => self.pos = self.v.as_ref().len() as u64,
        }
        r
    }
    #[inline]
    fn read_buf(&mut self, mut cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        let n = cur.written();
        Cursor::split(self).1.read_buf(cur.reborrow())?;
        self.pos += (cur.written() - n) as u64;
        Ok(())
    }
    #[inline]
    fn read_buf_exact(&mut self, mut cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
        let n = cur.written();
        let r = Cursor::split(self).1.read_buf_exact(cur.reborrow());
        self.pos += (cur.written() - n) as u64;
        r
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_end(&mut self, buf: &mut crate::Vec<u8>) -> IoResult<usize> {
        let v = Cursor::split(self).1;
        let n = v.len();
        buf.try_reserve(n)?;
        buf.extend_from_slice(v);
        self.pos += n as u64;
        Ok(n)
    }
    #[cfg(feature = "alloc")]
    #[inline]
    fn read_to_string(&mut self, buf: &mut crate::String) -> IoResult<usize> {
        let v = from_utf8(Cursor::split(self).1).map_err(|_| IoError::from(ErrorKind::InvalidInput))?;
        let n = v.len();
        buf.try_reserve(n)?;
        buf.push_str(v);
        self.pos += n as u64;
        Ok(n)
    }
}
impl<T: PartialEq> PartialEq for Cursor<T> {
    #[inline]
    fn eq(&self, other: &Cursor<T>) -> bool {
        self.v.eq(&other.v) && self.pos.eq(&other.pos)
    }
}

/// Creates a value that is always at EOF for reads, and ignores all data
/// written.
///
/// All calls to [`write`] on the returned instance will return
/// [`Ok(buf.len())`] and the contents of the buffer will not be inspected.
///
/// All calls to [`read`] from the returned reader will return [`Ok(0)`].
///
/// [`write`]: Write::write
/// [`read`]: Read::read
///
/// # Examples
///
/// ```rust
/// use xrmt_stx::io::{self, Write};
///
/// let buffer = vec![1, 2, 3, 5, 8];
/// let num_bytes = io::empty().write(&buffer).unwrap();
/// assert_eq!(num_bytes, 5);
/// ```
///
///
/// ```rust
/// use xrmt_stx::io::{self, Read};
///
/// let mut buffer = String::new();
/// io::empty().read_to_string(&mut buffer).unwrap();
/// assert!(buffer.is_empty());
/// ```
#[inline]
pub const fn empty() -> Empty {
    Empty
}
/// Creates an instance of a reader that infinitely repeats one byte.
///
/// All reads from this reader will succeed by filling the specified buffer with
/// the given byte.
///
/// # Examples
///
/// ```
/// use xrmt_stx::io::{self, Read};
///
/// let mut buffer = [0; 3];
/// io::repeat(0b101).read_exact(&mut buffer).unwrap();
/// assert_eq!(buffer, [0b101, 0b101, 0b101]);
/// ```
#[inline]
pub const fn repeat(byte: u8) -> Repeat {
    Repeat(byte)
}

pub(crate) fn read_exact<T: ?Sized + Read>(r: &mut T, mut b: &mut [u8]) -> IoResult<()> {
    while !b.is_empty() {
        match r.read(b) {
            Ok(0) => break,
            Ok(n) => b = &mut b[n..], // Keep the bounds check here as we don't know if the return result is valid
            Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }
    if !b.is_empty() {
        return Err(IoError::from(ErrorKind::UnexpectedEof));
    }
    Ok(())
}
pub(crate) fn read_buf_exact<T: ?Sized + Read>(r: &mut T, mut cur: BorrowedCursor<'_, u8>) -> IoResult<()> {
    while cur.capacity() > 0 {
        let p = cur.written();
        match r.read_buf(cur.reborrow()) {
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
