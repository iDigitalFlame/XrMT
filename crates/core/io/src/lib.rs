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
//! this module: `File`s, `TcpStream`s, and sometimes even `Vec<T>`s. For
//! example, [`Read`] adds a [`read`][`Read::read`] method, which we can use on
//! `File`s:
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
//! Of course, using `io::stdout` directly is less common than something like
//! `println!`.
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
//! like `File` own their file descriptor. Similarly, file descriptors
//! can be *borrowed*, granting the temporary right to perform operations on
//! this file descriptor. This indicates that the file descriptor will not be
//! closed for the lifetime of the borrow, but it does *not* imply any right to
//! close this file descriptor, since it will likely be owned by someone else.
//!
//! The platform-specific parts of the Rust standard library expose types that
//! reflect these concepts, see `os::unix` and `os::windows`.
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
//! File descriptors basically work like `Arc`: when you receive an owned file
//! descriptor, you cannot know whether there are any other file descriptors
//! that reference the same kernel object. However, when you create a new kernel
//! object, you know that you are holding the only reference to it. Just be
//! careful not to lend it to anyone, since they can obtain a clone and then you
//! can no longer know what the reference count is! In that sense, `OwnedFd`
//! is like `Arc` and `BorrowedFd<'a>` is like `&'a Arc` (and similar for the
//! Windows types). In particular, given a `BorrowedFd<'a>`, you are not allowed
//! to close the file descriptor -- just like how, given a `&'a Arc`, you are
//! not allowed to decrement the reference count and potentially free the
//! underlying object. There is no equivalent to `Box` for file descriptors in
//! the standard library (that would be a type that guarantees that the
//! reference count is `1`), however, it would be possible for a crate to define
//! a type with those semantics.
//!
//! [`IoResult`]: self::Result
//! [`?` operator]: ../../book/appendix-02-operators.html
//! [`Result`]: core::result::Result
//! [`.unwrap()`]: core::result::Result::unwrap

#![no_implicit_prelude]
#![no_std]
#![cfg_attr(
    all(not(feature = "std")),
    allow(incomplete_features, internal_features),
    feature(
        allocator_api,
        borrowed_buf_init,
        core_io,
        core_io_borrowed_buf,
        maybe_uninit_fill,
        slice_internals,
        specialization
    )
)]

extern crate core;

pub use core::error::Error as CoreError;
pub use core::fmt::{Error as FmtError, Write as FmtWrite};

pub type IoError = Error;
pub type FmtResult = core::fmt::Result;
pub type IoResult<T> = Result<T>;

pub use self::io::*;

#[cfg(feature = "std")]
mod io {
    extern crate std;
    pub use std::io::*;
}
#[cfg(not(feature = "std"))]
#[path = "."]
mod io {
    pub(crate) const BASE_BUF_SIZE: usize = if cfg!(target_os = "espidf") { 0x200 } else { 0x2000 };

    mod borrow;
    mod copy;
    mod error;
    mod read;
    mod seek;
    mod write;

    #[cfg(feature = "alloc")]
    pub use self::alloc::*;
    pub use self::borrow::*;
    pub use self::copy::*;
    pub use self::error::*;
    pub use self::read::*;
    pub use self::seek::*;
    pub use self::write::*;

    /// Trait to determine if a descriptor/handle refers to a terminal/tty.
    pub trait IsTerminal {
        /// Returns `true` if the descriptor/handle refers to a terminal/tty.
        ///
        /// On platforms where Rust does not know how to detect a terminal yet,
        /// this will return `false`. This will also return `false` if
        /// an unexpected error occurred, such as from passing an
        /// invalid file descriptor.
        ///
        /// # Platform-specific behavior
        ///
        /// On Windows, in addition to detecting consoles, this currently uses
        /// some heuristics to detect older msys/cygwin/mingw
        /// pseudo-terminals based on device name: devices with names
        /// starting with `msys-` or `cygwin-` and ending in `-pty` will be
        /// considered terminals. Note that this [may change in the
        /// future]
        ///
        /// # Examples
        ///
        /// An example of a type for which `IsTerminal` is implemented is
        /// `Stdin`:
        ///
        /// ```no_run
        /// use xrmt_stx::io::{self, IsTerminal, Write};
        ///
        /// fn main() -> IoResult<()> {
        ///     let stdin = io::stdin();
        ///
        ///     // Indicate that the user is prompted for input, if this is a terminal.
        ///     if stdin.is_terminal() {
        ///         print!("> ");
        ///         io::stdout().flush()?;
        ///     }
        ///
        ///     let mut name = String::new();
        ///     let _ = stdin.read_line(&mut name)?;
        ///
        ///     println!("Hello {}", name.trim_end());
        ///
        ///     Ok(())
        /// }
        /// ```
        ///
        /// The example can be run in two ways:
        ///
        /// - If you run this example by piping some text to it, e.g. `echo
        ///   "foo" | path/to/executable` it will print: `Hello foo`.
        /// - If you instead run the example interactively by running
        ///   `path/to/executable` directly, it will prompt for input.
        fn is_terminal(&self) -> bool;
    }

    #[cfg(feature = "alloc")]
    mod alloc {
        extern crate alloc;

        mod read;
        mod write;

        pub(crate) use alloc::string::String;
        pub(crate) use alloc::vec::Vec;

        pub use self::read::*;
        pub use self::write::*;
    }
}

pub mod prelude {
    #[cfg(feature = "alloc")]
    pub use crate::BufRead;
    pub use crate::{Read, Seek, Write};
}
