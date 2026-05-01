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
use core::cmp::{Eq, PartialEq};
use core::marker::{Copy, Sized};
use core::result::Result::Ok;

use crate::IoResult;

/// Enumeration of possible methods to seek within an I/O object.
///
/// It is used by the [`Seek`] trait.
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// The `Seek` trait provides a cursor which can be moved within a stream of
/// bytes.
///
/// The stream typically has a fixed size, allowing seeking relative to either
/// end or the current offset.
///
/// # Examples
///
/// `File`s implement `Seek`:
///
/// ```no_run
/// use xrmt_stx::io::{self, IoResult};
/// use xrmt_stx::io::prelude::*;
/// use xrmt_stx::fs::File;
/// use xrmt_stx::io::SeekFrom;
///
/// fn main() -> IoResult<()> {
///     let mut f = File::open("foo.txt")?;
///
///     // move the cursor 42 bytes from the start of the file
///     f.seek(SeekFrom::Start(42))?;
///     Ok(())
/// }
/// ```
pub trait Seek {
    /// Seek to an offset, in bytes, in a stream.
    ///
    /// A seek beyond the end of a stream is allowed, but behavior is defined
    /// by the implementation.
    ///
    /// If the seek operation completed successfully,
    /// this method returns the new position from the start of the stream.
    /// That position can be used later with [`SeekFrom::Start`].
    ///
    /// # Errors
    ///
    /// Seeking can fail, for example because it might involve flushing a
    /// buffer.
    ///
    /// Seeking to a negative offset is considered an error.
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64>;

    /// Rewind to the beginning of a stream.
    ///
    /// This is a convenience method, equivalent to `seek(SeekFrom::Start(0))`.
    ///
    /// # Errors
    ///
    /// Rewinding can fail, for example because it might involve flushing a
    /// buffer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use xrmt_stx::io::{Read, Seek, Write};
    /// use xrmt_stx::fs::OpenOptions;
    ///
    /// let mut f = OpenOptions::new()
    ///     .write(true)
    ///     .read(true)
    ///     .create(true)
    ///     .open("foo.txt")?;
    ///
    /// let hello = "Hello!\n";
    /// write!(f, "{hello}")?;
    /// f.rewind()?;
    ///
    /// let mut buf = String::new();
    /// f.read_to_string(&mut buf)?;
    /// assert_eq!(&buf, hello);
    /// # xrmt_stx::IoResult::Ok(())
    /// ```
    #[inline]
    fn rewind(&mut self) -> IoResult<()> {
        self.seek(SeekFrom::Start(0))?;
        Ok(())
    }
    /// Returns the length of this stream (in bytes).
    ///
    /// This method is implemented using up to three seek operations. If this
    /// method returns successfully, the seek position is unchanged (i.e. the
    /// position before calling this method is the same as afterwards).
    /// However, if this method returns an error, the seek position is
    /// unspecified.
    ///
    /// If you need to obtain the length of *many* streams and you don't care
    /// about the seek position afterwards, you can reduce the number of seek
    /// operations by simply calling `seek(SeekFrom::End(0))` and using its
    /// return value (it is also the stream length).
    ///
    /// Note that length of a stream can change over time (for example, when
    /// data is appended to a file). So calling this method multiple times does
    /// not necessarily return the same length each time.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use xrmt_stx::{
    ///     io::{self, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///
    ///     let len = f.stream_len()?;
    ///     println!("The file is currently {len} bytes long");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn stream_len(&mut self) -> IoResult<u64> {
        let (o, n) = (self.stream_position()?, self.seek(SeekFrom::End(0))?);
        if o != n {
            self.seek(SeekFrom::Start(o))?;
        }
        Ok(n)
    }
    /// Returns the current seek position from the start of the stream.
    ///
    /// This is equivalent to `self.seek(SeekFrom::Current(0))`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use xrmt_stx::{
    ///     io::{self, BufRead, BufReader, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = BufReader::new(File::open("foo.txt")?);
    ///
    ///     let before = f.stream_position()?;
    ///     f.read_line(&mut String::new())?;
    ///     let after = f.stream_position()?;
    ///
    ///     println!("The first line was {} bytes long", after - before);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn stream_position(&mut self) -> IoResult<u64> {
        self.seek(SeekFrom::Current(0))
    }
    /// Seeks relative to the current position.
    ///
    /// This is equivalent to `self.seek(SeekFrom::Current(offset))` but
    /// doesn't return the new position which can allow some implementations
    /// such as [`BufReader`] to perform more efficient seeks.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use xrmt_stx::{
    ///     io::{self, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     f.seek_relative(10)?;
    ///     assert_eq!(f.stream_position()?, 10);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`BufReader`]: crate::io::BufReader
    #[inline]
    fn seek_relative(&mut self, offset: i64) -> IoResult<()> {
        self.seek(SeekFrom::Current(offset))?;
        Ok(())
    }
}

impl Eq for SeekFrom {}
impl Copy for SeekFrom {}
impl Clone for SeekFrom {
    #[inline]
    fn clone(&self) -> SeekFrom {
        *self
    }
}
impl PartialEq for SeekFrom {
    #[inline]
    fn eq(&self, other: &SeekFrom) -> bool {
        match (self, other) {
            (SeekFrom::End(x), SeekFrom::End(y)) => x.eq(&y),
            (SeekFrom::Start(x), SeekFrom::Start(y)) => x.eq(&y),
            (SeekFrom::Current(x), SeekFrom::Current(y)) => x.eq(&y),
            _ => false,
        }
    }
}

impl<T: Seek + ?Sized> Seek for &mut T {
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
