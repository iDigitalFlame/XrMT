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

//
// Module assistance with help from the Rust Team std/io code!
//

//! Traits, helpers, and type definitions for core I/O functionality.
//!
//! The `xrmt_stx::io` module contains a number of common things you'll need
//! when doing input and output. The most core part of this module is
//! the [`Read`] and [`Write`] traits, which provide the
//! most general interface for reading and writing input and output.
//!
//! ## Read and Write
//!
//! Because they are traits, [`Read`] and [`Write`] are implemented by a number
//! of other types, and you can implement them for your types too. As such,
//! you'll see a few different types of I/O throughout the documentation in
//! this module: [`File`]s, [`TcpStream`]s, and sometimes even [`Vec<T>`]s. For
//! example, [`Read`] adds a [`read`][`Read::read`] method, which we can use on
//! [`File`]s:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//! use xrmt_stx::fs::File;
//!
//! fn main() -> IoResult<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // read up to 10 bytes
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`Read`] and [`Write`] are so important, implementors of the two traits have
//! a nickname: readers and writers. So you'll sometimes see 'a reader' instead
//! of 'a type that implements the [`Read`] trait'. Much easier!
//!
//! ## Seek and BufRead
//!
//! Beyond that, there are two important traits that are provided: [`Seek`]
//! and [`BufRead`]. Both of these build on top of a reader to control
//! how the reading happens. [`Seek`] lets you control where the next byte is
//! coming from:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//! use xrmt_stx::io::SeekFrom;
//! use xrmt_stx::fs::File;
//!
//! fn main() -> IoResult<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // skip to the last 10 bytes of the file
//!     f.seek(SeekFrom::End(-10))?;
//!
//!     // read up to 10 bytes
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`BufRead`] uses an internal buffer to provide a number of other ways to
//! read, but to show it off, we'll need to talk about buffers in general. Keep
//! reading!
//!
//! ## BufReader and BufWriter
//!
//! Byte-based interfaces are unwieldy and can be inefficient, as we'd need to
//! be making near-constant calls to the operating system. To help with this,
//! `xrmt_stx::io` comes with two structs, [`BufReader`] and [`BufWriter`],
//! which wrap readers and writers. The wrapper uses a buffer, reducing the
//! number of calls and providing nicer methods for accessing exactly what you
//! want.
//!
//! For example, [`BufReader`] works with the [`BufRead`] trait to add extra
//! methods to any reader:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//! use xrmt_stx::io::BufReader;
//! use xrmt_stx::fs::File;
//!
//! fn main() -> IoResult<()> {
//!     let f = File::open("foo.txt")?;
//!     let mut reader = BufReader::new(f);
//!     let mut buffer = String::new();
//!
//!     // read a line into buffer
//!     reader.read_line(&mut buffer)?;
//!
//!     println!("{buffer}");
//!     Ok(())
//! }
//! ```
//!
//! [`BufWriter`] doesn't add any new ways of writing; it just buffers every
//! call to [`write`][`Write::write`]:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//! use xrmt_stx::io::BufWriter;
//! use xrmt_stx::fs::File;
//!
//! fn main() -> IoResult<()> {
//!     let f = File::create("foo.txt")?;
//!     {
//!         let mut writer = BufWriter::new(f);
//!
//!         // write a byte to the buffer
//!         writer.write(&[42])?;
//!
//!     } // the buffer is flushed once writer goes out of scope
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Standard input and output
//!
//! A very common source of input is standard input:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//!
//! fn main() -> IoResult<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!     Ok(())
//! }
//! ```
//!
//! Note that you cannot use the [`?` operator] in functions that do not return
//! a [`Result<T, E>`][`Result`]. Instead, you can call [`.unwrap()`]
//! or `match` on the return value to catch any possible errors:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//!
//! let mut input = String::new();
//!
//! io::stdin().read_line(&mut input).unwrap();
//! ```
//!
//! And a very common source of output is standard output:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//!
//! fn main() -> IoResult<()> {
//!     io::stdout().write(&[42])?;
//!     Ok(())
//! }
//! ```
//!
//! Of course, using [`io::stdout`] directly is less common than something like
//! [`println!`].
//!
//! ## Iterator types
//!
//! A large number of the structures provided by `xrmt_stx::io` are for various
//! ways of iterating over I/O. For example, [`Lines`] is used to split over
//! lines:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//! use xrmt_stx::io::prelude::*;
//! use xrmt_stx::io::BufReader;
//! use xrmt_stx::fs::File;
//!
//! fn main() -> IoResult<()> {
//!     let f = File::open("foo.txt")?;
//!     let reader = BufReader::new(f);
//!
//!     for line in reader.lines() {
//!         println!("{}", line?);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Functions
//!
//! There are a number of [functions][functions-list] that offer access to
//! various features. For example, we can use three of these functions to copy
//! everything from standard input to standard output:
//!
//! ```no_run
//! use xrmt_stx::io::{self, IoResult};
//!
//! fn main() -> IoResult<()> {
//!     io::copy(&mut io::stdin(), &mut io::stdout())?;
//!     Ok(())
//! }
//! ```
//!
//! [functions-list]: #functions-1
//!
//! ## IoResult
//!
//! Last, but certainly not least, is [`IoResult`]. This type is used
//! as the return type of many `xrmt_stx::io` functions that can cause an error,
//! and can be returned from your own functions as well. Many of the examples in
//! this module use the [`?` operator]:
//!
//! ```
//! use xrmt_stx::io::{self, IoResult};
//!
//! fn read_input() -> IoResult<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!
//!     Ok(())
//! }
//! ```
//!
//! The return type of `read_input()`, [`IoResult<()>`][`IoResult`], is a
//! very common type for functions which don't have a 'real' return value, but
//! do want to return errors if they happen. In this case, the only purpose of
//! this function is to read the line and print it, so we use `()`.
//!
//! ## Platform-specific behavior
//!
//! Many I/O functions throughout the standard library are documented to
//! indicate what various library or syscalls they are delegated to. This is
//! done to help applications both understand what's happening under the hood as
//! well as investigate any possibly unclear semantics. Note, however, that this
//! is informative, not a binding contract. The implementation of many of these
//! functions are subject to change over time and may call fewer or more
//! syscalls/library functions.
//!
//! ## I/O Safety
//!
//! Rust follows an I/O safety discipline that is comparable to its memory
//! safety discipline. This means that file descriptors can be *exclusively
//! owned*. (Here, "file descriptor" is meant to subsume similar concepts that
//! exist across a wide range of operating systems even if they might
//! use a different name, such as "handle".) An exclusively owned file
//! descriptor is one that no other code is allowed to access in any way, but
//! the owner is allowed to access and even close it any time. A type that owns
//! its file descriptor should usually close it in its `drop` function. Types
//! like [`File`] own their file descriptor. Similarly, file descriptors
//! can be *borrowed*, granting the temporary right to perform operations on
//! this file descriptor. This indicates that the file descriptor will not be
//! closed for the lifetime of the borrow, but it does *not* imply any right to
//! close this file descriptor, since it will likely be owned by someone else.
//!
//! The platform-specific parts of the Rust standard library expose types that
//! reflect these concepts, see [`os::unix`] and [`os::windows`].
//!
//! To uphold I/O safety, it is crucial that no code acts on file descriptors it
//! does not own or borrow, and no code closes file descriptors it does not own.
//! In other words, a safe function that takes a regular integer, treats it as a
//! file descriptor, and acts on it, is *unsound*.
//!
//! Not upholding I/O safety and acting on a file descriptor without proof of
//! ownership can lead to misbehavior and even Undefined Behavior in code that
//! relies on ownership of its file descriptors: a closed file descriptor could
//! be re-allocated, so the original owner of that file descriptor is now
//! working on the wrong file. Some code might even rely on fully encapsulating
//! its file descriptors with no operations being performed by any other part of
//! the program.
//!
//! Note that exclusive ownership of a file descriptor does *not* imply
//! exclusive ownership of the underlying kernel object that the file descriptor
//! references (also called "open file description" on some operating systems).
//! File descriptors basically work like [`Arc`]: when you receive an owned file
//! descriptor, you cannot know whether there are any other file descriptors
//! that reference the same kernel object. However, when you create a new kernel
//! object, you know that you are holding the only reference to it. Just be
//! careful not to lend it to anyone, since they can obtain a clone and then you
//! can no longer know what the reference count is! In that sense, [`OwnedFd`]
//! is like `Arc` and [`BorrowedFd<'a>`] is like `&'a Arc` (and similar for the
//! Windows types). In particular, given a `BorrowedFd<'a>`, you are not allowed
//! to close the file descriptor -- just like how, given a `&'a Arc`, you are
//! not allowed to decrement the reference count and potentially free the
//! underlying object. There is no equivalent to `Box` for file descriptors in
//! the standard library (that would be a type that guarantees that the
//! reference count is `1`), however, it would be possible for a crate to define
//! a type with those semantics.
//!
//! [`File`]: crate::fs::File
//! [`TcpStream`]: crate::net::TcpStream
//! [`io::stdout`]: stdout
//! [`IoResult`]: self::Result
//! [`?` operator]: ../../book/appendix-02-operators.html
//! [`Result`]: core::result::Result
//! [`.unwrap()`]: core::result::Result::unwrap
//! [`os::unix`]: ../os/unix/io/index.html
//! [`os::windows`]: ../os/windows/io/index.html
//! [`OwnedFd`]: ../os/fd/struct.OwnedFd.html
//! [`BorrowedFd<'a>`]: ../os/fd/struct.BorrowedFd.html
//! [`Arc`]: alloc::sync::Arc
//! [`println!`]: crate::println!
//! [`Vec<T>`]: alloc::vec::Vec

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_io;
extern crate xrmt_winapi;

use alloc::string::String;
use core::cell::UnsafeCell;
use core::convert::{AsRef, From};
use core::marker::Sync;
use core::option::Option::None;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::Ok;

use xrmt_winapi::functions::{CreatePipe, GetLastError, NtFlushBuffersFile, NtReadFile, NtWriteFile};
use xrmt_winapi::stdio;
use xrmt_winapi::structs::OwnedHandle;

use crate::os::Handle;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use xrmt_io::*;

#[doc(hidden)]
#[path = "extra/io.rs"]
pub mod extra;

pub trait OsError {
    fn last_os_error() -> Self;
}

/// A handle to the standard input stream of a process.
///
/// Each handle is a shared reference to a global buffer of input data to this
/// process. A handle can be `lock`'d to gain full access to [`BufRead`] methods
/// (e.g., `.lines()`). Reads to this handle are otherwise locked with respect
/// to other reads.
///
/// This handle implements the `Read` trait, but beware that concurrent reads
/// of `Stdin` must be executed with care.
///
/// Created by the [`io::stdin`] method.
///
/// [`io::stdin`]: stdin
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to read bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::{self, IoResult};
///
/// fn main() -> IoResult<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin(); // We get `Stdin` here.
///     stdin.read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
pub struct Stdin {
    v:   stdio::Stdin,
    buf: UnsafeCell<BufReader<stdio::Stdin>>,
}
/// Read end of an anonymous pipe.
pub struct PipeReader(OwnedHandle);
/// Write end of an anonymous pipe.
pub struct PipeWriter(OwnedHandle);
/// A locked reference to the [`Stdin`] handle.
///
/// This handle implements both the [`Read`] and [`BufRead`] traits, and
/// is constructed via the [`Stdin::lock`] method.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to read bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::io::{self, BufRead};
///
/// fn main() -> IoResult<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin(); // We get `Stdin` here.
///     {
///         let mut handle = stdin.lock(); // We get `StdinLock` here.
///         handle.read_line(&mut buffer)?;
///     } // `StdinLock` is dropped here.
///     Ok(())
/// }
/// ```
pub struct StdinLock<'a>(&'a Stdin);
/// A locked reference to the [`Stdout`] handle.
///
/// This handle implements the [`Write`] trait, and is constructed via
/// the [`Stdout::lock`] method. See its documentation for more.
///
/// By default, the handle is line-buffered when connected to a terminal,
/// meaning it flushes automatically when a newline (`\n`) is encountered. For
/// immediate output, you can manually call the [`flush`] method. When the
/// handle goes out of scope, the buffer is automatically flushed.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// [`flush`]: Write::flush
pub struct StdoutLock<'a>(&'a Stdout);
/// A locked reference to the [`Stderr`] handle.
///
/// This handle implements the [`Write`] trait and is constructed via
/// the [`Stderr::lock`] method. See its documentation for more.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
pub struct StderrLock<'a>(&'a Stderr);
/// A handle to the global standard output stream of the current process.
///
/// Each handle shares a global buffer of data to be written to the standard
/// output stream. Access is also synchronized via a lock and explicit control
/// over locking is available via the [`lock`] method.
///
/// By default, the handle is line-buffered when connected to a terminal,
/// meaning it flushes automatically when a newline (`\n`) is encountered. For
/// immediate output, you can manually call the [`flush`] method. When the
/// handle goes out of scope, the buffer is automatically flushed.
///
/// Created by the [`io::stdout`] method.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// [`lock`]: Stdout::lock
/// [`flush`]: Write::flush
/// [`io::stdout`]: stdout
pub struct Stdout(UnsafeCell<stdio::Stdout>);
/// A handle to the standard error stream of a process.
///
/// For more information, see the [`io::stderr`] method.
///
/// [`io::stderr`]: stderr
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
pub struct Stderr(UnsafeCell<stdio::Stderr>);

impl Stdin {
    /// Locks this handle to the standard input stream, returning a readable
    /// guard.
    ///
    /// The lock is released when the returned lock goes out of scope. The
    /// returned guard also implements the [`Read`] and [`BufRead`] traits for
    /// accessing the underlying data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, BufRead};
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut buffer = String::new();
    ///     let stdin = io::stdin();
    ///     let mut handle = stdin.lock();
    ///
    ///     handle.read_line(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn lock<'a>(&'a self) -> StdinLock<'a> {
        StdinLock(self)
    }
    /// Consumes this handle and returns an iterator over input lines.
    ///
    /// For detailed semantics of this method, see the documentation on
    /// [`BufRead::lines`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    ///
    /// let lines = io::stdin().lines();
    /// for line in lines {
    ///     println!("got a line: {}", line.unwrap());
    /// }
    /// ```
    #[inline]
    pub fn lines<'a>(&'a self) -> Lines<StdinLock<'a>> {
        self.lock().lines()
    }
    /// Locks this handle and reads a line of input, appending it to the
    /// specified buffer.
    ///
    /// For detailed semantics of this method, see the documentation on
    /// [`BufRead::read_line`]. In particular:
    /// * Previous content of the buffer will be preserved. To avoid appending
    ///   to the buffer, you need to [`clear`] it first.
    /// * The trailing newline character, if any, is included in the buffer.
    ///
    /// [`clear`]: String::clear
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, IoResult};
    ///
    /// let mut input = String::new();
    /// match io::stdin().read_line(&mut input) {
    ///     Ok(n) => {
    ///         println!("{n} bytes read");
    ///         println!("{input}");
    ///     }
    ///     Err(error) => println!("error: {error}"),
    /// }
    /// ```
    ///
    /// You can run the example one of two ways:
    ///
    /// - Pipe some text to it, e.g., `printf foo | path/to/executable`
    /// - Give it text interactively by running the executable directly, in
    ///   which case it will wait for the Enter key to be pressed before
    ///   continuing
    #[inline]
    pub fn read_line(&self, buf: &mut String) -> IoResult<usize> {
        self.lock().read_line(buf)
    }

    #[inline]
    fn get() -> Stdin {
        Stdin {
            v:   stdio::Stdin::get(),
            buf: UnsafeCell::new(BufReader::new(stdio::Stdin::get())),
        }
    }
}
impl Stdout {
    /// Locks this handle to the standard output stream, returning a writable
    /// guard.
    ///
    /// The lock is released when the returned lock goes out of scope. The
    /// returned guard also implements the `Write` trait for writing data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::io::{self, Write};
    ///
    /// fn main() -> IoResult<()> {
    ///     let mut stdout = io::stdout().lock();
    ///
    ///     stdout.write_all(b"hello world")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn lock<'a>(&'a self) -> StdoutLock<'a> {
        StdoutLock(self)
    }

    #[inline]
    fn get() -> Stdout {
        Stdout(UnsafeCell::new(stdio::Stdout::get()))
    }
}
impl Stderr {
    /// Locks this handle to the standard error stream, returning a writable
    /// guard.
    ///
    /// The lock is released when the returned lock goes out of scope. The
    /// returned guard also implements the [`Write`] trait for writing data.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::{self, Write};
    ///
    /// fn foo() -> IoResult<()> {
    ///     let stderr = io::stderr();
    ///     let mut handle = stderr.lock();
    ///
    ///     handle.write_all(b"hello world")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn lock<'a>(&'a self) -> StderrLock<'a> {
        StderrLock(self)
    }

    #[inline]
    fn get() -> Stderr {
        Stderr(UnsafeCell::new(stdio::Stderr::get()))
    }
}
impl PipeReader {
    /// Create a new [`PipeReader`] instance that shares the same underlying
    /// file description.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::fs;
    /// use xrmt_stx::io::{pipe, Write};
    /// use xrmt_stx::process::Command;
    /// const NUM_SLOT: u8 = 2;
    /// const NUM_PROC: u8 = 5;
    /// const OUTPUT: &str = "work.txt";
    ///
    /// let mut jobs = vec![];
    /// let (reader, mut writer) = pipe()?;
    ///
    /// // Write NUM_SLOT characters the pipe.
    /// writer.write_all(&[b'|'; NUM_SLOT as usize])?;
    ///
    /// // Spawn several processes that read a character from the pipe, do some work, then
    /// // write back to the pipe. When the pipe is empty, the processes block, so only
    /// // NUM_SLOT processes can be working at any given time.
    /// for _ in 0..NUM_PROC {
    ///     jobs.push(
    ///         Command::new("bash")
    ///             .args(["-c",
    ///                 &format!(
    ///                      "read -n 1\n\
    ///                       echo -n 'x' >> '{OUTPUT}'\n\
    ///                       echo -n '|'",
    ///                 ),
    ///             ])
    ///             .stdin(reader.try_clone()?)
    ///             .stdout(writer.try_clone()?)
    ///             .spawn()?,
    ///     );
    /// }
    ///
    /// // Wait for all jobs to finish.
    /// for mut job in jobs {
    ///     job.wait()?;
    /// }
    ///
    /// // Check our work and clean up.
    /// let xs = fs::read_to_string(OUTPUT)?;
    /// fs::remove_file(OUTPUT)?;
    /// assert_eq!(xs, "x".repeat(NUM_PROC.into()));
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<PipeReader> {
        Ok(self.0.duplicate().map(PipeReader)?)
    }
}
impl PipeWriter {
    /// Create a new [`PipeWriter`] instance that shares the same underlying
    /// file description.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use xrmt_stx::process::Command;
    /// use xrmt_stx::io::{pipe, Read};
    /// let (mut reader, writer) = pipe()?;
    ///
    /// // Spawn a process that writes to stdout and stderr.
    /// let mut peer = Command::new("bash")
    ///     .args([
    ///         "-c",
    ///         "echo -n foo\n\
    ///          echo -n bar >&2"
    ///     ])
    ///     .stdout(writer.try_clone()?)
    ///     .stderr(writer)
    ///     .spawn()?;
    ///
    /// // Read and check the result.
    /// let mut msg = String::new();
    /// reader.read_to_string(&mut msg)?;
    /// assert_eq!(&msg, "foobar");
    ///
    /// peer.wait()?;
    /// ```
    #[inline]
    pub fn try_clone(&self) -> IoResult<PipeWriter> {
        Ok(self.0.duplicate().map(PipeWriter)?)
    }
}

impl Read for Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.buf.get_mut().read(buf)
    }
}
impl Read for &Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        unsafe { &mut *self.buf.get() }.read(buf)
    }
}
impl IsTerminal for Stdin {
    #[inline]
    fn is_terminal(&self) -> bool {
        self.v.is_terminal()
    }
}
impl UnwindSafe for Stdin {}
impl AsRef<Handle> for Stdin {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.v.as_ref()
    }
}
impl RefUnwindSafe for Stdin {}

impl Read for StdinLock<'_> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        unsafe { &mut *self.0.buf.get() }.read(buf)
    }
}
impl BufRead for StdinLock<'_> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        unsafe { &mut *self.0.buf.get() }.consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        unsafe { &mut *self.0.buf.get() }.fill_buf()
    }
}
impl IsTerminal for StdinLock<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        self.0.is_terminal()
    }
}
impl UnwindSafe for StdinLock<'_> {}
impl AsRef<Handle> for StdinLock<'_> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.0.as_ref()
    }
}
impl RefUnwindSafe for StdinLock<'_> {}

impl Write for Stdout {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.0.get_mut().flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.get_mut().write(buf)
    }
}
impl Write for &Stdout {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        unsafe { &mut *self.0.get() }.flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        unsafe { &mut *self.0.get() }.write(buf)
    }
}
impl IsTerminal for Stdout {
    #[inline]
    fn is_terminal(&self) -> bool {
        unsafe { &*self.0.get() }.is_terminal()
    }
}
impl AsRef<Handle> for Stdout {
    #[inline]
    fn as_ref(&self) -> &Handle {
        unsafe { &*self.0.get() }.as_ref()
    }
}
impl RefUnwindSafe for Stdout {}

impl Write for StdoutLock<'_> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        unsafe { &mut *self.0 .0.get() }.flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        unsafe { &mut *self.0 .0.get() }.write(buf)
    }
}
impl IsTerminal for StdoutLock<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        unsafe { &*self.0 .0.get() }.is_terminal()
    }
}
impl UnwindSafe for StdoutLock<'_> {}
impl AsRef<Handle> for StdoutLock<'_> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.0.as_ref()
    }
}
impl RefUnwindSafe for StdoutLock<'_> {}

impl Write for Stderr {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.0.get_mut().flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.get_mut().write(buf)
    }
}
impl Write for &Stderr {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        unsafe { &mut *self.0.get() }.flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        unsafe { &mut *self.0.get() }.write(buf)
    }
}
impl IsTerminal for Stderr {
    #[inline]
    fn is_terminal(&self) -> bool {
        unsafe { &*self.0.get() }.is_terminal()
    }
}
impl AsRef<Handle> for Stderr {
    #[inline]
    fn as_ref(&self) -> &Handle {
        unsafe { &*self.0.get() }.as_ref()
    }
}
impl RefUnwindSafe for Stderr {}

impl Write for StderrLock<'_> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        unsafe { &mut *self.0 .0.get() }.flush()
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        unsafe { &mut *self.0 .0.get() }.write(buf)
    }
}
impl IsTerminal for StderrLock<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        unsafe { &*self.0 .0.get() }.is_terminal()
    }
}
impl UnwindSafe for StderrLock<'_> {}
impl AsRef<Handle> for StderrLock<'_> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.0.as_ref()
    }
}
impl RefUnwindSafe for StderrLock<'_> {}

impl Read for PipeReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        Ok(NtReadFile(&self.0, None, buf, None)?)
    }
}
impl Read for &PipeReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        Ok(NtReadFile(&self.0, None, buf, None)?)
    }
}
impl AsRef<Handle> for PipeReader {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl From<OwnedHandle> for PipeReader {
    #[inline]
    fn from(v: OwnedHandle) -> PipeReader {
        PipeReader(v)
    }
}

impl Write for PipeWriter {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(&self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, None)?)
    }
}
impl Write for &PipeWriter {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(NtFlushBuffersFile(&self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(NtWriteFile(&self.0, None, buf, None)?)
    }
}
impl AsRef<Handle> for PipeWriter {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl From<OwnedHandle> for PipeWriter {
    #[inline]
    fn from(v: OwnedHandle) -> PipeWriter {
        PipeWriter(v)
    }
}

impl From<PipeReader> for OwnedHandle {
    #[inline]
    fn from(v: PipeReader) -> OwnedHandle {
        v.0
    }
}
impl From<PipeWriter> for OwnedHandle {
    #[inline]
    fn from(v: PipeWriter) -> OwnedHandle {
        v.0
    }
}

impl OsError for IoError {
    /// Returns an error representing the last OS error which occurred.
    ///
    /// This function reads the value of `errno` for the target platform (e.g.
    /// `GetLastError` on Windows) and will return a corresponding instance of
    /// [`Error`] for the error code.
    ///
    /// This should be called immediately after a call to a platform function,
    /// otherwise the state of the error value is indeterminate. In particular,
    /// other standard library functions may call platform functions that may
    /// (or may not) reset the error value even if they succeed.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::Error;
    ///
    /// let os_error = Error::last_os_error();
    /// println!("last OS error: {os_error:?}");
    /// ```
    #[inline]
    fn last_os_error() -> IoError {
        IoError::from_raw_os_error(GetLastError() as i32)
    }
}

unsafe impl Sync for Stdin {}
unsafe impl Sync for Stdout {}
unsafe impl Sync for Stderr {}

/// Constructs a new handle to the standard input of the current process.
///
/// Each handle returned is a reference to a shared global buffer whose access
/// is synchronized via a mutex. If you need more explicit control over
/// locking, see the [`Stdin::lock`] method.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to read bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// # Examples
///
/// Using implicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, IoResult};
///
/// fn main() -> IoResult<()> {
///     let mut buffer = String::new();
///     io::stdin().read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
///
/// Using explicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, BufRead};
///
/// fn main() -> IoResult<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin();
///     let mut handle = stdin.lock();
///
///     handle.read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
#[inline]
pub fn stdin() -> Stdin {
    Stdin::get()
}
/// Constructs a new handle to the standard output of the current process.
///
/// Each handle returned is a reference to a shared global buffer whose access
/// is synchronized via a mutex. If you need more explicit control over
/// locking, see the [`Stdout::lock`] method.
///
/// By default, the handle is line-buffered when connected to a terminal,
/// meaning it flushes automatically when a newline (`\n`) is encountered. For
/// immediate output, you can manually call the [`flush`] method. When the
/// handle goes out of scope, the buffer is automatically flushed.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// # Examples
///
/// Using implicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, Write};
///
/// fn main() -> IoResult<()> {
///     io::stdout().write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// Using explicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, Write};
///
/// fn main() -> IoResult<()> {
///     let stdout = io::stdout();
///     let mut handle = stdout.lock();
///
///     handle.write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// Ensuring output is flushed immediately:
///
/// ```no_run
/// use xrmt_stx::io::{self, Write};
///
/// fn main() -> IoResult<()> {
///     let mut stdout = io::stdout();
///     stdout.write_all(b"hello, ")?;
///     stdout.flush()?;                // Manual flush
///     stdout.write_all(b"world!\n")?; // Automatically flushed
///     Ok(())
/// }
/// ```
///
/// [`flush`]: Write::flush
#[inline]
pub fn stdout() -> Stdout {
    Stdout::get()
}
/// Constructs a new handle to the standard error of the current process.
///
/// This handle is not buffered.
///
/// ### Note: Windows Portability Considerations
///
/// When operating in a console, the Windows implementation of this stream does
/// not support non-UTF-8 byte sequences. Attempting to write bytes that are not
/// valid UTF-8 will return an error.
///
/// In a process with a detached console, such as one using
/// `#![windows_subsystem = "windows"]`, or in a child process spawned from such
/// a process, the contained handle will be null. In such cases, the standard
/// library's `Read` and `Write` will do nothing and silently succeed. All other
/// I/O operations, via the standard library or via raw Windows API calls, will
/// fail.
///
/// # Examples
///
/// Using implicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, Write};
///
/// fn main() -> IoResult<()> {
///     io::stderr().write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// Using explicit synchronization:
///
/// ```no_run
/// use xrmt_stx::io::{self, Write};
///
/// fn main() -> IoResult<()> {
///     let stderr = io::stderr();
///     let mut handle = stderr.lock();
///
///     handle.write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
#[inline]
pub fn stderr() -> Stderr {
    Stderr::get()
}
/// Create an anonymous pipe.
///
/// # Behavior
///
/// A pipe is a one-way data channel provided by the OS, which works across
/// processes. A pipe is typically used to communicate between two or more
/// separate processes, as there are better, faster ways to communicate within a
/// single process.
///
/// In particular:
///
/// * A read on a [`PipeReader`] blocks until the pipe is non-empty.
/// * A write on a [`PipeWriter`] blocks when the pipe is full.
/// * When all copies of a [`PipeWriter`] are closed, a read on the
///   corresponding [`PipeReader`] returns EOF.
/// * [`PipeWriter`] can be shared, and multiple processes or threads can write
///   to it at once, but writes (above a target-specific threshold) may have
///   their data interleaved.
/// * [`PipeReader`] can be shared, and multiple processes or threads can read
///   it at once. Any given byte will only get consumed by one reader. There are
///   no guarantees about data interleaving.
/// * Portable applications cannot assume any atomicity of messages larger than
///   a single byte.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `pipe` function on Unix and the
/// `CreatePipe` function on Windows.
///
/// Note that this [may change in the future][changes].
///
/// # Capacity
///
/// Pipe capacity is platform dependent. To quote the Linux [man page]:
///
/// > Different implementations have different limits for the pipe capacity.
/// > Applications should
/// > not rely on a particular capacity: an application should be designed so
/// > that a reading process
/// > consumes data as soon as it is available, so that a writing process does
/// > not remain blocked.
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::process::Command;
/// use xrmt_stx::io::{pipe, Read, Write};
/// let (ping_rx, mut ping_tx) = pipe()?;
/// let (mut pong_rx, pong_tx) = pipe()?;
///
/// // Spawn a process that echoes its input.
/// let mut echo_server = Command::new("cat").stdin(ping_rx).stdout(pong_tx).spawn()?;
///
/// ping_tx.write_all(b"hello")?;
/// // Close to unblock echo_server's reader.
/// drop(ping_tx);
///
/// let mut buf = String::new();
/// // Block until echo_server's writer is closed.
/// pong_rx.read_to_string(&mut buf)?;
/// assert_eq!(&buf, "hello");
///
/// echo_server.wait()?;
/// ```
/// [changes]: crate::io#platform-specific-behavior
/// [man page]: https://man7.org/linux/man-pages/man7/pipe.7.html
#[inline]
pub fn pipe() -> IoResult<(PipeReader, PipeWriter)> {
    Ok(CreatePipe(None, 0x800, true).map(|(r, w)| (PipeReader(r), PipeWriter(w)))?)
}
