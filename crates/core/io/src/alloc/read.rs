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
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::cmp::Ord;
use core::convert::{AsRef, From};
use core::iter::Iterator;
use core::marker::Sized;
use core::mem::{drop, MaybeUninit};
use core::ops::{Drop, FnMut, FnOnce};
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::slice::memchr::memchr;
use core::str::from_utf8;

use crate::{read_buf_exact, read_exact, BorrowedBuf, BorrowedCursor, Chain, Cursor, Empty, ErrorKind, IoError, IoResult, Read, Seek, SeekFrom, Take, BASE_BUF_SIZE};

/// An iterator over the contents of an instance of `BufRead` split on a
/// particular byte.
///
/// This struct is generally created by calling [`split`] on a `BufRead`.
/// Please see the documentation of [`split`] for more details.
///
/// [`split`]: BufRead::split
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Split<T> {
    b: T,
    d: u8,
}
/// An iterator over the lines of an instance of `BufRead`.
///
/// This struct is generally created by calling [`lines`] on a `BufRead`.
/// Please see the documentation of [`lines`] for more details.
///
/// [`lines`]: BufRead::lines
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Lines<T>(T);
/// The `BufReader<R>` struct adds buffering to any reader.
///
/// It can be excessively inefficient to work directly with a [`Read`] instance.
/// For example, every call to `read` on `TcpStream`
/// results in a system call. A `BufReader<R>` performs large, infrequent reads
/// on the underlying [`Read`] and maintains an in-memory buffer of the results.
///
/// `BufReader<R>` can improve the speed of programs that make *small* and
/// *repeated* read calls to the same file or network socket. It does not
/// help when reading very large amounts at once, or reading just one or a few
/// times. It also provides no advantage when reading from a source that is
/// already in memory, like a <code>[Vec]\<u8></code>.
///
/// When the `BufReader<R>` is dropped, the contents of its buffer will be
/// discarded. Creating multiple instances of a `BufReader<R>` on the same
/// stream can cause data loss. Reading from the underlying reader after
/// unwrapping the `BufReader<R>` with [`BufReader::into_inner`] can also cause
/// data loss.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::io::BufReader;
/// use xrmt_stx::fs::File;
///
/// fn main() -> xrmt_stx::IoResult<()> {
///     let f = File::open("log.txt")?;
///     let mut reader = BufReader::new(f);
///
///     let mut line = String::new();
///     let len = reader.read_line(&mut line)?;
///     println!("First line is {len} bytes long");
///     Ok(())
/// }
/// ```
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct BufReader<T: ?Sized> {
    b: Buffer,
    v: T,
}

/// A `BufRead` is a type of `Read`er which has an internal buffer, allowing it
/// to perform extra ways of reading.
///
/// For example, reading line-by-line is inefficient without using a buffer, so
/// if you want to read by line, you'll need `BufRead`, which includes a
/// [`read_line`] method as well as a [`lines`] iterator.
///
/// # Examples
///
/// A locked standard input implements `BufRead`:
///
/// ```no_run
/// use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::io::prelude::*;
///
/// let stdin = io::stdin();
/// for line in stdin.lock().lines() {
///     println!("{}", line?);
/// }
/// # xrmt_stx::IoResult::Ok(())
/// ```
///
/// If you have something that implements [`Read`], you can use the [`BufReader`
/// type][`BufReader`] to turn it into a `BufRead`.
///
/// For example, `File` implements [`Read`], but not `BufRead`.
/// [`BufReader`] to the rescue!
///
/// [`read_line`]: BufRead::read_line
/// [`lines`]: BufRead::lines
///
/// ```no_run
/// use xrmt_stx::io::{self, BufReader};
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::fs::File;
///
/// fn main() -> IoResult<()> {
///     let f = File::open("foo.txt")?;
///     let f = BufReader::new(f);
///
///     for line in f.lines() {
///         let line = line?;
///         println!("{line}");
///     }
///
///     Ok(())
/// }
/// ```
pub trait BufRead: Read {
    /// Tells this buffer that `amt` bytes have been consumed from the buffer,
    /// so they should no longer be returned in calls to `read`.
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`fill_buf`] method to function properly. This function does
    /// not perform any I/O, it simply informs this object that some amount of
    /// its buffer, returned from [`fill_buf`], has been consumed and should
    /// no longer be returned. As such, this function may do odd things if
    /// [`fill_buf`] isn't called before calling it.
    ///
    /// The `amt` must be `<=` the number of bytes in the buffer returned by
    /// [`fill_buf`].
    ///
    /// # Examples
    ///
    /// Since `consume()` is meant to be used with [`fill_buf`],
    /// that method's example includes an example of `consume()`.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    fn consume(&mut self, n: usize);
    /// Returns the contents of the internal buffer, filling it with more data
    /// from the inner reader if it is empty.
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`consume`] method to function properly. When calling this
    /// method, none of the contents will be "read" in the sense that later
    /// calling `read` may return the same contents. As such, [`consume`] must
    /// be called with the number of bytes that are consumed from this buffer to
    /// ensure that the bytes are never returned twice.
    ///
    /// [`consume`]: BufRead::consume
    ///
    /// An empty buffer returned indicates that the stream has reached EOF.
    ///
    /// # Errors
    ///
    /// This function will return an I/O error if the underlying reader was
    /// read, but returned an error.
    ///
    /// # Examples
    ///
    /// A locked standard input implements `BufRead`:
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    ///
    /// let stdin = io::stdin();
    /// let mut stdin = stdin.lock();
    ///
    /// let buffer = stdin.fill_buf()?;
    ///
    /// // work with buffer
    /// println!("{buffer:?}");
    ///
    /// // ensure the bytes we worked with aren't returned again later
    /// let length = buffer.len();
    /// stdin.consume(length);
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    fn fill_buf(&mut self) -> IoResult<&[u8]>;

    /// Returns an iterator over the lines of this reader.
    ///
    /// The iterator returned from this function will yield instances of
    /// <code>[IoResult]<[String]></code>. Each string returned will *not*
    /// have a newline byte (the `0xA` byte) or `CRLF` (`0xD`, `0xA` bytes)
    /// at the end.
    ///
    /// [IoResult]: self::Result "IoResult"
    ///
    /// # Examples
    ///
    /// [`xrmt_io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to iterate over all the lines in a byte
    /// slice.
    ///
    /// ```
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines_iter = cursor.lines().map(|l| l.unwrap());
    /// assert_eq!(lines_iter.next(), Some(String::from("lorem")));
    /// assert_eq!(lines_iter.next(), Some(String::from("ipsum")));
    /// assert_eq!(lines_iter.next(), Some(String::from("dolor")));
    /// assert_eq!(lines_iter.next(), None);
    /// ```
    ///
    /// # Errors
    ///
    /// Each line of the iterator has the same error semantics as
    /// [`BufRead::read_line`].
    #[inline]
    fn lines(self) -> Lines<Self>
    where Self: Sized {
        Lines(self)
    }
    /// Returns an iterator over the contents of this reader split on the byte
    /// `byte`.
    ///
    /// The iterator returned from this function will return instances of
    /// <code>[IoResult]<[Vec]\<u8>></code>. Each vector returned will *not*
    /// have the delimiter byte at the end.
    ///
    /// This function will yield errors whenever [`read_until`] would have
    /// also yielded an error.
    ///
    /// [IoResult]: self::Result "IoResult"
    /// [`read_until`]: BufRead::read_until
    ///
    /// # Examples
    ///
    /// [`xrmt_io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to iterate over all hyphen delimited
    /// segments in a byte slice
    ///
    /// ```
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem-ipsum-dolor");
    ///
    /// let mut split_iter = cursor.split(b'-').map(|l| l.unwrap());
    /// assert_eq!(split_iter.next(), Some(b"lorem".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"ipsum".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"dolor".to_vec()));
    /// assert_eq!(split_iter.next(), None);
    /// ```
    #[inline]
    fn split(self, byte: u8) -> Split<Self>
    where Self: Sized {
        Split { b: self, d: byte }
    }

    /// Checks if the underlying `Read` has any data left to be read.
    ///
    /// This function may fill the buffer to check for data,
    /// so this functions returns `Result<bool>`, not `bool`.
    ///
    /// Default implementation calls `fill_buf` and checks that
    /// returned slice is empty (which means that there is no data left,
    /// since EOF is reached).
    ///
    /// Examples
    ///
    /// ```
    /// use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::io::prelude::*;
    ///
    /// let stdin = io::stdin();
    /// let mut stdin = stdin.lock();
    ///
    /// while stdin.has_data_left()? {
    ///     let mut line = String::new();
    ///     stdin.read_line(&mut line)?;
    ///     // work with line
    ///     println!("{line:?}");
    /// }
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        self.fill_buf().map(|v| !v.is_empty())
    }
    /// Skips all bytes until the delimiter `byte` or EOF is reached.
    ///
    /// This function will read (and discard) bytes from the underlying stream
    /// until the delimiter or EOF is found.
    ///
    /// If successful, this function will return the total number of bytes read,
    /// including the delimiter byte.
    ///
    /// This is useful for efficiently skipping data such as NUL-terminated
    /// strings in binary file formats without buffering.
    ///
    /// This function is blocking and should be used carefully: it is possible
    /// for an attacker to continuously send bytes without ever sending the
    /// delimiter or EOF.
    ///
    /// # Errors
    ///
    /// This function will ignore all instances of [`ErrorKind::Interrupted`]
    /// and will otherwise return any errors returned by [`fill_buf`].
    ///
    /// If an I/O error is encountered then all bytes read so far will be
    /// present in `buf` and its length will have been adjusted appropriately.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # Examples
    ///
    /// [`xrmt_io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to read some NUL-terminated information
    /// about Ferris from a binary string, skipping the fun fact:
    ///
    /// ```
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"Ferris\0Likes long walks on the beach\0Crustacean\0");
    ///
    /// // read name
    /// let mut name = Vec::new();
    /// let num_bytes = cursor.read_until(b'\0', &mut name)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 7);
    /// assert_eq!(name, b"Ferris\0");
    ///
    /// // skip fun fact
    /// let num_bytes = cursor.skip_until(b'\0')
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 30);
    ///
    /// // read animal type
    /// let mut animal = Vec::new();
    /// let num_bytes = cursor.read_until(b'\0', &mut animal)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 11);
    /// assert_eq!(animal, b"Crustacean\0");
    /// ```
    #[inline]
    fn skip_until(&mut self, byte: u8) -> IoResult<usize> {
        read_loop(self, byte, |_| {})
    }
    /// Reads all bytes until a newline (the `0xA` byte) is reached, and append
    /// them to the provided `String` buffer.
    ///
    /// Previous content of the buffer will be preserved. To avoid appending to
    /// the buffer, you need to [`clear`] it first.
    ///
    /// This function will read bytes from the underlying stream until the
    /// newline delimiter (the `0xA` byte) or EOF is found. Once found, all
    /// bytes up to, and including, the delimiter (if found) will be
    /// appended to `buf`.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// If this function returns [`Ok(0)`], the stream has reached EOF.
    ///
    /// This function is blocking and should be used carefully: it is possible
    /// for an attacker to continuously send bytes without ever sending a
    /// newline or EOF. You can use [`take`] to limit the maximum number of
    /// bytes read.
    ///
    /// [`clear`]: String::clear
    /// [`take`]: crate::io::Read::take
    ///
    /// # Errors
    ///
    /// This function has the same error semantics as [`read_until`] and will
    /// also return an error if the read bytes are not valid UTF-8. If an I/O
    /// error is encountered then `buf` may contain some bytes already read in
    /// the event that all data read so far was valid UTF-8.
    ///
    /// [`read_until`]: BufRead::read_until
    ///
    /// # Examples
    ///
    /// [`xrmt_io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to read all the lines in a byte slice:
    ///
    /// ```
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"foo\nbar");
    /// let mut buf = String::new();
    ///
    /// // cursor is at 'f'
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 4);
    /// assert_eq!(buf, "foo\n");
    /// buf.clear();
    ///
    /// // cursor is at 'b'
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 3);
    /// assert_eq!(buf, "bar");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, "");
    /// ```
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        Guard::new(buf).exec(|v| read_loop(self, b'\n', |b| v.extend_from_slice(b)))
    }
    /// Reads all bytes into `buf` until the delimiter `byte` or EOF is reached.
    ///
    /// This function will read bytes from the underlying stream until the
    /// delimiter or EOF is found. Once found, all bytes up to, and including,
    /// the delimiter (if found) will be appended to `buf`.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// This function is blocking and should be used carefully: it is possible
    /// for an attacker to continuously send bytes without ever sending the
    /// delimiter or EOF.
    ///
    /// # Errors
    ///
    /// This function will ignore all instances of [`ErrorKind::Interrupted`]
    /// and will otherwise return any errors returned by [`fill_buf`].
    ///
    /// If an I/O error is encountered then all bytes read so far will be
    /// present in `buf` and its length will have been adjusted appropriately.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # Examples
    ///
    /// [`xrmt_io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to read all the bytes in a byte slice
    /// in hyphen delimited segments:
    ///
    /// ```
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"lorem-ipsum");
    /// let mut buf = vec![];
    ///
    /// // cursor is at 'l'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 6);
    /// assert_eq!(buf, b"lorem-");
    /// buf.clear();
    ///
    /// // cursor is at 'i'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 5);
    /// assert_eq!(buf, b"ipsum");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, b"");
    /// ```
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        read_loop(self, byte, |b| buf.extend_from_slice(b))
    }
}

#[cfg_attr(not(feature = "strip"), derive(Debug))]
struct Buffer {
    b:      Box<[MaybeUninit<u8>]>,
    pos:    usize,
    init:   usize,
    filled: usize,
}
struct Guard<'a> {
    b: &'a mut Vec<u8>,
    n: usize,
}

impl Buffer {
    #[inline]
    fn with_capacity(len: usize) -> Buffer {
        Buffer {
            b:      Box::new_uninit_slice(len),
            pos:    0usize,
            init:   0usize,
            filled: 0usize,
        }
    }

    #[inline]
    fn discard(&mut self) {
        (self.pos, self.filled) = (0, 0)
    }
    #[inline]
    fn pos(&self) -> usize {
        self.pos
    }
    fn backshift(&mut self) {
        self.b.copy_within(self.pos.., 0);
        self.init = self.init - self.pos;
        self.filled = self.filled - self.pos;
        self.pos = 0;
    }
    #[inline]
    fn buffer(&self) -> &[u8] {
        unsafe { self.b.get_unchecked(self.pos..self.filled).assume_init_ref() }
    }
    #[inline]
    fn filled(&self) -> usize {
        self.filled
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.b.len()
    }
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.filled);
    }
    #[inline]
    fn unconsume(&mut self, n: usize) {
        self.pos = self.pos.saturating_sub(n);
    }
    fn fill_buf(&mut self, mut r: impl Read) -> IoResult<&[u8]> {
        if self.pos >= self.filled {
            let mut b = BorrowedBuf::from(&mut *self.b);
            unsafe { b.set_init(self.init) };
            r.read_buf(b.unfilled())?;
            self.pos = 0;
            self.filled = b.len();
            self.init = b.init_len();
        }
        Ok(self.buffer())
    }
    fn read_more(&mut self, mut r: impl Read) -> IoResult<usize> {
        let mut v = BorrowedBuf::from(unsafe { self.b.get_unchecked_mut(self.filled..) });
        let n = self.init - self.filled;
        unsafe { v.set_init(n) };
        r.read_buf(v.unfilled())?;
        self.filled += v.len();
        self.init += v.init_len() - n;
        Ok(v.len())
    }
    #[inline]
    fn consume_with(&mut self, n: usize, mut f: impl FnMut(&[u8])) -> bool {
        if let Some(i) = self.buffer().get(0..n) {
            f(i);
            self.pos += n;
            true
        } else {
            false
        }
    }
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
    fn exec<F: FnOnce(&mut Vec<u8>) -> IoResult<usize>>(mut self, f: F) -> IoResult<usize> {
        let r = f(self.b);
        if from_utf8(unsafe { self.b.get_unchecked(self.n..) }).is_err() {
            r.and_then(|_| Err(IoError::from(ErrorKind::InvalidData)))
        } else {
            self.n = self.b.len();
            r
        }
    }
}
impl<T: Read> BufReader<T> {
    /// Creates a new `BufReader<R>` with a default buffer capacity. The default
    /// is currently 8 KiB, but may change in the future.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("log.txt")?;
    ///     let reader = BufReader::new(f);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn new(inner: T) -> BufReader<T> {
        BufReader {
            v: inner,
            b: Buffer::with_capacity(BASE_BUF_SIZE),
        }
    }
    /// Creates a new `BufReader<R>` with the specified buffer capacity.
    ///
    /// # Examples
    ///
    /// Creating a buffer with ten bytes of capacity:
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("log.txt")?;
    ///     let reader = BufReader::with_capacity(10, f);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn with_capacity(len: usize, inner: T) -> BufReader<T> {
        BufReader {
            v: inner,
            b: Buffer::with_capacity(len),
        }
    }
}
impl<T: Sized> BufReader<T> {
    /// Unwraps this `BufReader<R>`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost. Therefore,
    /// a following read from the underlying reader may lead to data loss.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        self.v
    }
}
impl<T: ?Sized> BufReader<T> {
    /// Gets a reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.v
    }
    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike [`fill_buf`], this will not attempt to fill the buffer if it is
    /// empty.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{BufReader, BufRead};
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f);
    ///     assert!(reader.buffer().is_empty());
    ///
    ///     if reader.fill_buf()?.len() > 0 {
    ///         assert!(!reader.buffer().is_empty());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        self.b.buffer()
    }
    /// Returns the number of bytes the internal buffer can hold at once.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{BufReader, BufRead};
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f);
    ///
    ///     let capacity = reader.capacity();
    ///     let buffer = reader.fill_buf()?;
    ///     assert!(buffer.len() <= capacity);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.b.capacity()
    }
    /// Gets a mutable reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::BufReader;
    /// use xrmt_stx::fs::File;
    ///
    /// fn main() -> xrmt_stx::IoResult<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.v
    }

    #[inline]
    pub(crate) fn discard(&mut self) {
        self.b.discard()
    }
}
impl<T: ?Sized + Read> BufReader<T> {
    /// Attempt to look ahead `n` bytes.
    ///
    /// `n` must be less than or equal to `capacity`.
    ///
    /// the returned slice may be less than `n` bytes long if
    /// end of file is reached.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use xrmt_stx::io::{Read, BufReader};
    ///
    /// let mut bytes = &b"oh, hello there"[..];
    /// let mut rdr = BufReader::with_capacity(6, &mut bytes);
    /// assert_eq!(rdr.peek(2).unwrap(), b"oh");
    /// let mut buf = [0; 4];
    /// rdr.read(&mut buf[..]).unwrap();
    /// assert_eq!(&buf, b"oh, ");
    /// assert_eq!(rdr.peek(5).unwrap(), b"hello");
    /// let mut s = String::new();
    /// rdr.read_to_string(&mut s).unwrap();
    /// assert_eq!(&s, "hello there");
    /// assert_eq!(rdr.peek(1).unwrap().len(), 0);
    /// ```
    pub fn peek(&mut self, n: usize) -> IoResult<&[u8]> {
        while n > self.b.buffer().len() {
            if self.b.pos() > 0 {
                self.b.backshift();
            }
            let n = self.b.read_more(&mut self.v)?;
            if n == 0 {
                return Ok(&self.b.buffer());
            }
        }
        // Buffer checked above.
        Ok(unsafe { self.b.buffer().get_unchecked(0..n) })
    }
}
impl<T: ?Sized + Seek> BufReader<T> {
    /// Seeks relative to the current position. If the new position lies within
    /// the buffer, the buffer will not be flushed, allowing for more
    /// efficient seeks. This method does not return the location of the
    /// underlying reader, so the caller must track this information
    /// themselves if it is required.
    pub fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        let n = self.b.pos() as u64;
        if offset < 0 {
            if n.checked_sub(unsafe { offset.unchecked_neg() } as u64).is_some() {
                self.b.unconsume(unsafe { offset.unchecked_neg() } as usize);
                return Ok(());
            }
        } else if let Some(v) = n.checked_add(offset as u64) {
            if v <= self.b.filled() as u64 {
                self.b.consume(offset as usize);
                return Ok(());
            }
        }
        self.seek(SeekFrom::Current(offset)).map(drop)
    }
}

impl BufRead for Empty {
    #[inline]
    fn consume(&mut self, _n: usize) {}
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        Ok(&[])
    }
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        Ok(false)
    }
    #[inline]
    fn read_line(&mut self, _buf: &mut String) -> IoResult<usize> {
        Ok(0)
    }
    #[inline]
    fn read_until(&mut self, _byte: u8, _buf: &mut Vec<u8>) -> IoResult<usize> {
        Ok(0)
    }
}
impl BufRead for &[u8] {
    #[inline]
    fn consume(&mut self, amt: usize) {
        *self = &self[amt..]; // Keep the bounds check here as we don't know if
                              // the return result is valid
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        Ok(*self)
    }
}
impl<T: ?Sized + BufRead> BufRead for &mut T {
    #[inline]
    fn consume(&mut self, n: usize) {
        (**self).consume(n)
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        (**self).fill_buf()
    }
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        (**self).has_data_left()
    }
    #[inline]
    fn skip_until(&mut self, byte: u8) -> IoResult<usize> {
        (**self).skip_until(byte)
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).read_line(buf)
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).read_until(byte, buf)
    }
}

impl<T: ?Sized + Seek> Seek for BufReader<T> {
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        let r = (self.b.filled() - self.b.pos()) as u64;
        self.v
            .stream_position()
            .and_then(|pos| pos.checked_sub(r).ok_or_else(|| IoError::from(ErrorKind::InvalidInput)))
    }
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let r = if let SeekFrom::Current(n) = pos {
            let v = (self.b.filled() - self.b.pos()) as i64;
            if let Some(o) = n.checked_sub(v) {
                self.v.seek(SeekFrom::Current(o))?
            } else {
                self.v.seek(SeekFrom::Current(unsafe { v.unchecked_neg() }))?;
                self.b.discard();
                self.v.seek(SeekFrom::Current(n))?
            }
        } else {
            self.v.seek(pos)?
        };
        self.b.discard();
        Ok(r)
    }
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        self.seek_relative(offset)
    }
}
impl<T: ?Sized + Read> Read for BufReader<T> {
    #[inline]
    fn is_read_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.b.pos() == self.b.filled() && buf.len() >= self.capacity() {
            self.b.discard();
            return self.v.read(buf);
        }
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        if self.b.consume_with(buf.len(), |c| buf.copy_from_slice(c)) {
            Ok(())
        } else {
            read_exact(self, buf)
        }
    }
    #[inline]
    fn read_to_end(&mut self, b: &mut Vec<u8>) -> IoResult<usize> {
        let n = {
            let i = self.buffer();
            b.extend_from_slice(i);
            i.len()
        };
        self.b.discard();
        Ok(n + self.v.read_to_end(b)?)
    }
    #[inline]
    fn read_to_string(&mut self, b: &mut String) -> IoResult<usize> {
        if b.is_empty() {
            return Guard::new(b).exec(|i| self.read_to_end(i));
        }
        let mut v = Vec::new();
        self.read_to_end(&mut v)?;
        let s = from_utf8(&v).map_err(|_| IoError::from(ErrorKind::InvalidData))?;
        b.push_str(s);
        Ok(s.len())
    }
    #[inline]
    fn read_buf(&mut self, mut cur: BorrowedCursor<'_>) -> IoResult<()> {
        if self.b.pos() == self.b.filled() && cur.capacity() >= self.capacity() {
            self.b.discard();
            return self.v.read_buf(cur);
        }
        let p = cur.written();
        self.fill_buf()?.read_buf(cur.reborrow())?;
        self.consume(cur.written() - p);
        Ok(())
    }
    #[inline]
    fn read_buf_exact(&mut self, mut cur: BorrowedCursor<'_>) -> IoResult<()> {
        if self.b.consume_with(cur.capacity(), |c| cur.append(c)) {
            return Ok(());
        }
        read_buf_exact(self, cur)
    }
}
impl<T: ?Sized + Read> BufRead for BufReader<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.b.consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        self.b.fill_buf(&mut self.v)
    }
}

impl<T: BufRead> Iterator for Lines<T> {
    type Item = IoResult<String>;

    fn next(&mut self) -> Option<IoResult<String>> {
        let mut b = String::new();
        match self.0.read_line(&mut b) {
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
impl<T: BufRead> Iterator for Split<T> {
    type Item = IoResult<Vec<u8>>;

    fn next(&mut self) -> Option<IoResult<Vec<u8>>> {
        let mut b = Vec::new();
        match self.b.read_until(self.d, &mut b) {
            Ok(0) => None,
            Ok(_) => {
                // Here there's always one left.
                if unsafe { *b.last().unwrap_unchecked() } == self.d {
                    b.pop();
                }
                Some(Ok(b))
            },
            Err(e) => Some(Err(e)),
        }
    }
}

impl<T: BufRead> BufRead for Take<T> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        let a = (amt as u64).min(self.n) as usize;
        self.n -= a as u64;
        self.v.consume(a);
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        if self.n == 0 {
            return Ok(&[]);
        }
        let b = self.v.fill_buf()?;
        let c = (b.len() as u64).min(self.n) as usize;
        // Bounds already checked
        Ok(unsafe { b.get_unchecked(0..c) })
    }
}
impl<T: AsRef<[u8]>> BufRead for Cursor<T> {
    #[inline]
    fn consume(&mut self, n: usize) {
        self.pos += n as u64;
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        Ok(Cursor::split(self).1)
    }
}
impl<T: BufRead, U: BufRead> BufRead for Chain<T, U> {
    #[inline]
    fn consume(&mut self, n: usize) {
        if !self.d {
            self.f.consume(n)
        } else {
            self.s.consume(n)
        }
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        if !self.d {
            match self.f.fill_buf()? {
                v if v.is_empty() => self.d = true,
                v => return Ok(v),
            }
        }
        self.s.fill_buf()
    }
    fn read_until(&mut self, b: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        let mut r = 0usize;
        if !self.d {
            let n = self.f.read_until(b, buf)?;
            r += n;
            match buf.last() {
                Some(v) if *v == b && n != 0 => return Ok(r),
                _ => self.d = true,
            }
        }
        r += self.s.read_until(b, buf)?;
        Ok(r)
    }
}

impl Drop for Guard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.b.set_len(self.n) }
    }
}

impl<A: Allocator> Read for VecDeque<u8, A> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let (ref mut v, _) = self.as_slices();
        let n = v.read(buf)?;
        self.drain(0..n);
        Ok(n)
    }
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> IoResult<()> {
        let (f, b) = self.as_slices();
        match buf.split_at_mut_checked(f.len()) {
            Some((x, y)) => match b.split_at_checked(y.len()) {
                Some((v, _)) => {
                    x.copy_from_slice(f);
                    y.copy_from_slice(v);
                },
                None => {
                    self.clear();
                    return Err(IoError::from(ErrorKind::UnexpectedEof));
                },
            },
            // Bounds already checked.
            None => buf.copy_from_slice(unsafe { f.get_unchecked(0..buf.len()) }),
        }
        self.drain(0..buf.len());
        Ok(())
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        let n = self.len();
        buf.try_reserve(n)?;
        let (f, b) = self.as_slices();
        buf.extend_from_slice(f);
        buf.extend_from_slice(b);
        self.clear();
        Ok(n)
    }
    #[inline]
    fn read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        let (ref mut f, _) = self.as_slices();
        let n = cur.capacity().min(f.len());
        f.read_buf(cur)?;
        self.drain(0..n);
        Ok(())
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        read_string(self, buf)
    }
    #[inline]
    fn read_buf_exact(&mut self, mut cur: BorrowedCursor<'_>) -> IoResult<()> {
        let n = cur.capacity();
        let (f, b) = self.as_slices();
        match f.split_at_checked(cur.capacity()) {
            Some((f, _)) => cur.append(f),
            None => {
                cur.append(f);
                match b.split_at_checked(cur.capacity()) {
                    Some((v, _)) => cur.append(v),
                    None => {
                        cur.append(b);
                        self.clear();
                        return Err(IoError::from(ErrorKind::UnexpectedEof));
                    },
                }
            },
        }
        self.drain(0..n);
        Ok(())
    }
}
impl<A: Allocator> BufRead for VecDeque<u8, A> {
    #[inline]
    fn consume(&mut self, n: usize) {
        self.drain(0..n);
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        let (f, _) = self.as_slices();
        Ok(f)
    }
}
impl<A: Allocator, T: ?Sized + Read> Read for Box<T, A> {
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
    fn read_buf(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).read_buf(cur)
    }
    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).read_to_end(buf)
    }
    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).read_to_string(buf)
    }
    #[inline]
    fn read_buf_exact(&mut self, cur: BorrowedCursor<'_>) -> IoResult<()> {
        (**self).read_buf_exact(cur)
    }
}
impl<A: Allocator, T: ?Sized + Seek> Seek for Box<T, A> {
    #[inline]
    fn rewind(&mut self) -> IoResult<()> {
        (**self).rewind()
    }
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        (**self).stream_len()
    }
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        (**self).stream_position()
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        (**self).seek(pos)
    }
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        (**self).seek_relative(offset)
    }
}
impl<A: Allocator, T: ?Sized + BufRead> BufRead for Box<T, A> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        (**self).fill_buf()
    }
    #[inline]
    fn has_data_left(&mut self) -> IoResult<bool> {
        (**self).has_data_left()
    }
    #[inline]
    fn skip_until(&mut self, byte: u8) -> IoResult<usize> {
        (**self).skip_until(byte)
    }
    #[inline]
    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        (**self).read_line(buf)
    }
    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        (**self).read_until(byte, buf)
    }
}

/// Reads all bytes from a [reader][Read] into a new [`String`].
///
/// This is a convenience function for [`Read::read_to_string`]. Using this
/// function avoids having to create a variable first and provides more type
/// safety since you can only get the buffer out if there were no errors. (If
/// you use [`Read::read_to_string`] you have to remember to check whether the
/// read succeeded because otherwise your buffer will be empty or only partially
/// full.)
///
/// # Performance
///
/// The downside of this function's increased ease of use and type safety is
/// that it gives you less control over performance. For example, you can't
/// pre-allocate memory like you can using [`String::with_capacity`] and
/// [`Read::read_to_string`]. Also, you can't re-use the buffer if an error
/// occurs while reading.
///
/// In many cases, this function's performance will be adequate and the ease of
/// use and type safety tradeoffs will be worth it. However, there are cases
/// where you need more control over performance, and in those cases you should
/// definitely use [`Read::read_to_string`] directly.
///
/// Note that in some special cases, such as when reading files, this function
/// will pre-allocate memory based on the size of the input it is reading. In
/// those cases, the performance should be as good as if you had used
/// [`Read::read_to_string`] with a manually pre-allocated buffer.
///
/// # Errors
///
/// This function forces you to handle errors because the output (the `String`)
/// is wrapped in a [`Result`]. See [`Read::read_to_string`] for the errors
/// that can occur. If any error occurs, you will get an [`Err`], so you
/// don't have to worry about your buffer being empty or partially full.
///
/// # Examples
///
/// ```no_run
/// # use xrmt_stx::io::{self, IoResult};
/// fn main() -> IoResult<()> {
///     let stdin = io::read_to_string(io::stdin())?;
///     println!("Stdin was:");
///     println!("{stdin}");
///     Ok(())
/// }
/// ```
#[inline]
pub fn read_to_string<T: Read>(mut r: T) -> IoResult<String> {
    let mut b = String::new();
    r.read_to_string(&mut b)?;
    Ok(b)
}

#[inline]
pub(crate) fn read_string<T: ?Sized + Read>(r: &mut T, b: &mut String) -> IoResult<usize> {
    Guard::new(b).exec(|v| read_to_end(r, v))
}
pub(crate) fn read_to_end<T: ?Sized + Read>(r: &mut T, b: &mut Vec<u8>) -> IoResult<usize> {
    let (s, c, mut i) = (b.len(), b.capacity(), 0usize);
    loop {
        if b.len() == b.capacity() {
            b.reserve(0x20)
        }
        let mut z = BorrowedBuf::from(b.spare_capacity_mut());
        unsafe { z.set_init(i) };
        let mut v = z.unfilled();
        match r.read_buf(v.reborrow()) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if v.written() == 0 {
            return Ok(b.len() - s);
        }
        i = v.init_mut().len();
        let n = z.filled().len();
        unsafe { b.set_len(n + b.len()) };
        if b.len() == b.capacity() && b.capacity() == c {
            let mut t = [0u8; 32];
            loop {
                match r.read(&mut t) {
                    Ok(0) => return Ok(b.len() - s),
                    Ok(n) if n < 33 => {
                        // Bounds already checked above
                        b.extend_from_slice(unsafe { t.get_unchecked(0..n) });
                        break;
                    },
                    Ok(_) => return Err(IoError::from(ErrorKind::UnexpectedEof)),
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

fn read_loop<T: ?Sized + BufRead, F: FnMut(&[u8])>(r: &mut T, delim: u8, mut func: F) -> IoResult<usize> {
    let mut v = 0usize;
    loop {
        let a = match r.fill_buf() {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let (d, c) = match memchr(delim, a) {
            Some(i) => {
                func(unsafe { a.get_unchecked(0..=i) });
                (true, i + 1)
            },
            None => {
                func(a);
                (false, a.len())
            },
        };
        r.consume(c);
        v += c;
        if d || c == 0 {
            return Ok(v);
        }
    }
}
