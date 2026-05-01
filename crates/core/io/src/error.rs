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
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::From;
use core::fmt::{Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::marker::Copy;
use core::mem::{discriminant, transmute};
use core::option::Option::{self, None, Some};
use core::result;
use core::result::Result::Ok;

use crate::io::error::inner::ErrorExtra;
use crate::{CoreError, FmtResult, FmtWrite};

/// A list specifying general categories of I/O error.
///
/// This list is intended to grow over time and it is not recommended to
/// exhaustively match against it.
///
/// It is used with the [`Error`] type.
///
/// # Handling errors and matching on `ErrorKind`
///
/// In application code, use `match` for the `ErrorKind` values you are
/// expecting; use `_` to match "all other errors".
///
/// In comprehensive and thorough tests that want to verify that a test doesn't
/// return any known incorrect error kind, you may want to cut-and-paste the
/// current full list of errors from here into your test code, and then match
/// `_` as the correct case. This seems counterintuitive, but it will make your
/// tests more robust. In particular, if you want to verify that your code does
/// produce an unrecognized error kind, the robust solution is to check for all
/// the recognized error kinds and fail in those cases.
#[non_exhaustive]
#[repr(u8)]
pub enum ErrorKind {
    /// An entity was not found, often a file.
    NotFound,
    /// The operation lacked the necessary privileges to complete.
    PermissionDenied,
    /// The connection was refused by the remote server.
    ConnectionRefused,
    /// The connection was reset by the remote server.
    ConnectionReset,
    /// The remote host is not reachable.
    HostUnreachable,
    /// The network containing the remote host is not reachable.
    NetworkUnreachable,
    /// The connection was aborted (terminated) by the remote server.
    ConnectionAborted,
    /// The network operation failed because it was not connected yet.
    NotConnected,
    /// A socket address could not be bound because the address is already in
    /// use elsewhere.
    AddrInUse,
    /// A nonexistent interface was requested or the requested address was not
    /// local.
    AddrNotAvailable,
    /// The system's networking is down.
    NetworkDown,
    /// The operation failed because a pipe was closed.
    BrokenPipe,
    /// An entity already exists, often a file.
    AlreadyExists,
    /// The operation needs to block to complete, but the blocking operation was
    /// requested to not occur.
    WouldBlock,
    /// A filesystem object is, unexpectedly, not a directory.
    ///
    /// For example, a filesystem path was specified where one of the
    /// intermediate directory components was, in fact, a plain file.
    NotADirectory,
    /// The filesystem object is, unexpectedly, a directory.
    ///
    /// A directory was specified when a non-directory was expected.
    IsADirectory,
    /// A non-empty directory was specified where an empty directory was
    /// expected.
    DirectoryNotEmpty,
    /// The filesystem or storage medium is read-only, but a write operation was
    /// attempted.
    ReadOnlyFilesystem,
    /// Loop in the filesystem or IO subsystem; often, too many levels of
    /// symbolic links.
    ///
    /// There was a loop (or excessively long chain) resolving a filesystem
    /// object or file IO object.
    ///
    /// On Unix this is usually the result of a symbolic link loop; or, of
    /// exceeding the system-specific limit on the depth of symlink
    /// traversal.
    FilesystemLoop,
    /// Stale network file handle.
    ///
    /// With some network filesystems, notably NFS, an open file (or directory)
    /// can be invalidated by problems with the network or server.
    StaleNetworkFileHandle,
    /// A parameter was incorrect.
    InvalidInput,
    /// Data not valid for the operation were encountered.
    ///
    /// Unlike [`InvalidInput`], this typically means that the operation
    /// parameters were valid, however the error was caused by malformed
    /// input data.
    ///
    /// For example, a function that reads a file into a string will error with
    /// `InvalidData` if the file's contents are not valid UTF-8.
    ///
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    InvalidData,
    /// The I/O operation's timeout expired, causing it to be canceled.
    TimedOut,
    /// An error returned when an operation could not be completed because a
    /// call to [`write`] returned [`Ok(0)`].
    ///
    /// This typically means that an operation could only succeed if it wrote a
    /// particular number of bytes but only a smaller number of bytes could be
    /// written.
    ///
    /// [`write`]: crate::Write::write
    WriteZero,
    /// The underlying storage (typically, a filesystem) is full.
    ///
    /// This does not include out of quota errors.
    StorageFull,
    /// Seek on unseekable file.
    ///
    /// Seeking was attempted on an open file handle which is not suitable for
    /// seeking - for example, on Unix, a named pipe opened with
    /// `File::open`.
    NotSeekable,
    /// Filesystem quota or some other kind of quota was exceeded.
    QuotaExceeded,
    /// File larger than allowed or supported.
    ///
    /// This might arise from a hard limit of the underlying filesystem or file
    /// access API, or from an administratively imposed resource limitation.
    /// Simple disk full, and out of quota, have their own errors.
    FileTooLarge,
    /// Resource is busy.
    ResourceBusy,
    /// Executable file is busy.
    ///
    /// An attempt was made to write to a file which is also in use as a running
    /// program.  (Not all operating systems detect this situation.)
    ExecutableFileBusy,
    /// Deadlock (avoided).
    ///
    /// A file locking operation would result in deadlock.  This situation is
    /// typically detected, if at all, on a best-effort basis.
    Deadlock,
    /// Cross-device or cross-filesystem (hard) link or rename.
    CrossesDevices,
    /// Too many (hard) links to the same filesystem object.
    ///
    /// The filesystem does not support making so many hardlinks to the same
    /// file.
    TooManyLinks,
    /// A filename was invalid.
    ///
    /// This error can also cause if it exceeded the filename length limit.
    InvalidFilename,
    /// Program argument list too long.
    ///
    /// When trying to run an external program, a system or process limit on the
    /// size of the arguments would have been exceeded.
    ArgumentListTooLong,
    /// This operation was interrupted.
    ///
    /// Interrupted operations can typically be retried.
    Interrupted,
    /// This operation is unsupported on this platform.
    ///
    /// This means that the operation can never succeed.
    Unsupported,
    /// An error returned when an operation could not be completed because an
    /// "end of file" was reached prematurely.
    ///
    /// This typically means that an operation could only succeed if it read a
    /// particular number of bytes but only a smaller number of bytes could be
    /// read.
    UnexpectedEof,
    /// An operation could not be completed, because it failed
    /// to allocate enough memory.
    OutOfMemory,
    /// The operation was partially successful and needs to be checked
    /// later on due to not blocking.
    InProgress,
    /// A custom error that does not fall under any other I/O error kind.
    ///
    /// This can be used to construct your own [`Error`]s that do not match any
    /// [`ErrorKind`].
    ///
    /// This [`ErrorKind`] is not used by the standard library.
    ///
    /// Errors from the standard library that do not fall under any of the I/O
    /// error kinds cannot be `match`ed on, and will only match a wildcard (`_`)
    /// pattern. New [`ErrorKind`]s might be added in the future for some of
    /// those.
    Other,
    /// Any I/O error from the standard library that's not part of this list.
    ///
    /// Errors that are `Uncategorized` now may move to a different or a new
    /// [`ErrorKind`] variant in the future. It is not recommended to match
    /// an error against `Uncategorized`; use a wildcard match (`_`) instead.
    Uncategorized,
}

/// The error type for I/O operations of the [`Read`], [`Write`], [`Seek`],
/// and associated traits.
///
/// Errors mostly originate from the underlying OS, but custom instances of
/// `Error` can be created with crafted error messages and a particular
/// value of [`ErrorKind`].
///
/// [`Read`]: crate::Read
/// [`Write`]: crate::Write
/// [`Seek`]: crate::Seek
pub struct Error {
    kind:  u32,
    extra: ErrorExtra,
}
/// The type of raw OS error codes returned by [`Error::raw_os_error`].
///
/// This is an [`i32`] on all currently supported platforms, but platforms
/// added in the future (such as UEFI) may use a different primitive type like
/// [`usize`]. Use `as`or [`into`] conversions where applicable to ensure
/// maximum portability.
///
/// [`into`]: core::convert::Into::into
pub type RawOsError = i32;
/// A specialized [`Result`] type for I/O operations.
///
/// This type is broadly used across [`xrmt_io`] for any operation which may
/// produce an error.
///
/// This typedef is generally used to avoid writing out [`Error`] directly
/// and is otherwise a direct mapping to [`Result`].
///
/// While usual Rust style is to import types directly, aliases of [`Result`]
/// often are not, to make it easier to distinguish between them. [`Result`] is
/// generally assumed to be [`xrmt_stx::result::Result`][`Result`], and so users
/// of this alias will generally use `IoResult` instead of shadowing the
/// [prelude]'s import of [`xrmt_stx::result::Result`][`Result`].
///
/// [`xrmt_io`]: crate
/// [prelude]: crate::prelude
///
/// # Examples
///
/// A convenience function that bubbles an `IoResult` to its caller:
///
/// ```
/// use xrmt_stx::io::{self, IoResult};
///
/// fn get_string() -> IoResult<String> {
///     let mut buffer = String::new();
///
///     io::stdin().read_line(&mut buffer)?;
///
///     Ok(buffer)
/// }
/// ```
pub type Result<T> = result::Result<T, Error>;

impl Error {
    /// Returns an error representing the last OS error which occurred.
    ///
    /// This function reads the value of `errno` for the target platform
    /// (e.g. `GetLastError` on Windows) and will return a
    /// corresponding instance of [`Error`] for the error code.
    ///
    /// This should be called immediately after a call to a platform
    /// function, otherwise the state of the error value is
    /// indeterminate. In particular, other standard library
    /// functions may call platform functions that may (or may not)
    /// reset the error value even if they succeed.
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
    pub fn from_raw_os_error(code: RawOsError) -> Error {
        Error {
            kind:  code as u32,
            extra: ErrorExtra::none(),
        }
    }

    /// Returns the corresponding [`ErrorKind`] for this error.
    ///
    /// This may be a value set by Rust code constructing custom `io::Error`s,
    /// or if this `io::Error` was sourced from the operating system,
    /// it will be a value inferred from the system's error encoding.
    /// See `last_os_error` for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::io::{Error, ErrorKind};
    ///
    /// fn print_error(err: Error) {
    ///     println!("{:?}", err.kind());
    /// }
    ///
    /// fn main() {
    ///     // As no error has (visibly) occurred, this may print anything!
    ///     // It likely prints a placeholder for unidentified (non-)errors.
    ///     print_error(Error::last_os_error());
    ///     // Will print "AddrInUse".
    ///     print_error(Error::new(ErrorKind::AddrInUse, "oh no!"));
    /// }
    /// ```
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        self.as_kind().unwrap_or(ErrorKind::Uncategorized)
    }
    /// Creates a new instance of an [`Error`] from a particular OS error
    /// code.
    ///
    /// # Examples
    ///
    /// On Linux:
    ///
    /// ```
    /// # if cfg!(target_os = "linux") {
    /// use xrmt_stx::io::{self, IoResult};
    ///
    /// let error = io::Error::from_raw_os_error(22);
    /// assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    /// # }
    /// ```
    ///
    /// On Windows:
    ///
    /// ```
    /// # if cfg!(windows) {
    /// use xrmt_stx::io::{self, IoResult};
    ///
    /// let error = io::Error::from_raw_os_error(10022);
    /// assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    /// # }
    /// ```
    #[inline]
    pub fn raw_os_error(&self) -> Option<RawOsError> {
        if self.extra.is_none() {
            Some(self.kind as RawOsError)
        } else {
            None
        }
    }

    #[inline]
    fn as_kind(&self) -> Option<ErrorKind> {
        if self.kind & 0xFFFFFF == 0 {
            ErrorKind::from_u8(unsafe { self.kind.unchecked_shr(24) as u8 })
        } else {
            None
        }
    }
}
impl ErrorKind {
    #[inline]
    fn from_u8(v: u8) -> Option<ErrorKind> {
        match v {
            0x00 => Some(ErrorKind::NotFound),
            0x01 => Some(ErrorKind::PermissionDenied),
            0x02 => Some(ErrorKind::ConnectionRefused),
            0x03 => Some(ErrorKind::ConnectionReset),
            0x04 => Some(ErrorKind::HostUnreachable),
            0x05 => Some(ErrorKind::NetworkUnreachable),
            0x06 => Some(ErrorKind::ConnectionAborted),
            0x07 => Some(ErrorKind::NotConnected),
            0x08 => Some(ErrorKind::AddrInUse),
            0x09 => Some(ErrorKind::AddrNotAvailable),
            0x0A => Some(ErrorKind::NetworkDown),
            0x0B => Some(ErrorKind::BrokenPipe),
            0x0C => Some(ErrorKind::AlreadyExists),
            0x0D => Some(ErrorKind::WouldBlock),
            0x0E => Some(ErrorKind::NotADirectory),
            0x0F => Some(ErrorKind::IsADirectory),
            0x10 => Some(ErrorKind::DirectoryNotEmpty),
            0x11 => Some(ErrorKind::ReadOnlyFilesystem),
            0x12 => Some(ErrorKind::FilesystemLoop),
            0x13 => Some(ErrorKind::StaleNetworkFileHandle),
            0x14 => Some(ErrorKind::InvalidInput),
            0x15 => Some(ErrorKind::InvalidData),
            0x16 => Some(ErrorKind::TimedOut),
            0x17 => Some(ErrorKind::WriteZero),
            0x18 => Some(ErrorKind::StorageFull),
            0x19 => Some(ErrorKind::NotSeekable),
            0x1A => Some(ErrorKind::QuotaExceeded),
            0x1B => Some(ErrorKind::FileTooLarge),
            0x1C => Some(ErrorKind::ResourceBusy),
            0x1D => Some(ErrorKind::ExecutableFileBusy),
            0x1E => Some(ErrorKind::Deadlock),
            0x1F => Some(ErrorKind::CrossesDevices),
            0x20 => Some(ErrorKind::TooManyLinks),
            0x21 => Some(ErrorKind::InvalidFilename),
            0x22 => Some(ErrorKind::ArgumentListTooLong),
            0x23 => Some(ErrorKind::Interrupted),
            0x24 => Some(ErrorKind::Unsupported),
            0x25 => Some(ErrorKind::UnexpectedEof),
            0x26 => Some(ErrorKind::OutOfMemory),
            0x27 => Some(ErrorKind::InProgress),
            0x28 => Some(ErrorKind::Other),
            0x29 => Some(ErrorKind::Uncategorized),
            _ => None,
        }
    }

    #[cfg(not(feature = "strip"))]
    #[inline]
    fn as_str(&self) -> &str {
        match self {
            ErrorKind::AddrInUse => "address in use",
            ErrorKind::AddrNotAvailable => "address not available",
            ErrorKind::AlreadyExists => "entity already exists",
            ErrorKind::ArgumentListTooLong => "argument list too long",
            ErrorKind::BrokenPipe => "broken pipe",
            ErrorKind::ConnectionAborted => "connection aborted",
            ErrorKind::ConnectionRefused => "connection refused",
            ErrorKind::ConnectionReset => "connection reset",
            ErrorKind::CrossesDevices => "cross-device link or rename",
            ErrorKind::Deadlock => "deadlock",
            ErrorKind::DirectoryNotEmpty => "directory not empty",
            ErrorKind::ExecutableFileBusy => "executable file busy",
            ErrorKind::FilesystemLoop => "filesystem loop or indirection limit (e.g. symlink loop)",
            ErrorKind::FileTooLarge => "file too large",
            ErrorKind::HostUnreachable => "host unreachable",
            ErrorKind::InProgress => "in progress",
            ErrorKind::Interrupted => "operation interrupted",
            ErrorKind::InvalidData => "invalid data",
            ErrorKind::InvalidFilename => "invalid filename",
            ErrorKind::InvalidInput => "invalid input parameter",
            ErrorKind::IsADirectory => "is a directory",
            ErrorKind::NetworkDown => "network down",
            ErrorKind::NetworkUnreachable => "network unreachable",
            ErrorKind::NotADirectory => "not a directory",
            ErrorKind::NotConnected => "not connected",
            ErrorKind::NotFound => "entity not found",
            ErrorKind::NotSeekable => "seek on unseekable file",
            ErrorKind::Other => "other error",
            ErrorKind::OutOfMemory => "out of memory",
            ErrorKind::PermissionDenied => "permission denied",
            ErrorKind::QuotaExceeded => "quota exceeded",
            ErrorKind::ReadOnlyFilesystem => "read-only filesystem or storage medium",
            ErrorKind::ResourceBusy => "resource busy",
            ErrorKind::StaleNetworkFileHandle => "stale network file handle",
            ErrorKind::StorageFull => "no storage space",
            ErrorKind::TimedOut => "timed out",
            ErrorKind::TooManyLinks => "too many links",
            ErrorKind::Uncategorized => "uncategorized error",
            ErrorKind::UnexpectedEof => "unexpected end of file",
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::WouldBlock => "operation would block",
            ErrorKind::WriteZero => "write zero",
        }
    }
}

impl Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.as_kind() {
            Some(v) => Display::fmt(&v, f)?,
            None => write(self.kind, f)?,
        }
        if let Some(v) = self.get_ref() {
            f.write_char(' ')?;
            Display::fmt(v, f)?;
        }
        Ok(())
    }
}
impl CoreError for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn CoreError + 'static)> {
        self.get_ref().map(|v| unsafe { transmute(v) })
    }
}
impl From<ErrorKind> for Error {
    #[inline]
    fn from(v: ErrorKind) -> Error {
        Error {
            kind:  unsafe { (v as u32).unchecked_shl(24) },
            extra: ErrorExtra::none(),
        }
    }
}

impl Eq for ErrorKind {}
impl Ord for ErrorKind {
    #[inline]
    fn cmp(&self, other: &ErrorKind) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
impl Hash for ErrorKind {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        discriminant(self).hash(h);
    }
}
impl Copy for ErrorKind {}
impl Clone for ErrorKind {
    #[inline]
    fn clone(&self) -> ErrorKind {
        *self
    }
}
impl Debug for ErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}
impl Display for ErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        #[cfg(feature = "strip")]
        {
            write(*self as u32, f)
        }
        #[cfg(not(feature = "strip"))]
        {
            f.write_str(self.as_str())
        }
    }
}
impl PartialEq for ErrorKind {
    #[inline]
    fn eq(&self, other: &ErrorKind) -> bool {
        (*self as u8).eq(&(*other as u8))
    }
}
impl PartialOrd for ErrorKind {
    #[inline]
    fn partial_cmp(&self, other: &ErrorKind) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

#[inline]
fn hex(v: u8) -> u8 {
    match v & 0xF {
        0x0..=0x9 => (v & 0xF) + 0x30,
        0xA..=0xF => (v & 0xF) - 0x37,
        _ => core::unreachable!(),
    }
}
fn write(v: u32, f: &mut Formatter<'_>) -> FmtResult {
    let mut b = [0x30u8, 0x30u8];
    // Unsafe is used here as the compiler might not 100% know what we're doing.
    // All the access is in bounds always as it's manual.
    //
    // None of the shifts can overflow as the values are checked and are always
    // u32's
    match v {
        0 => f.write_str(unsafe { core::mem::transmute(b.get_unchecked(0..1)) }),
        _ if v <= 0xF => unsafe {
            *b.get_unchecked_mut(0) = hex(v as u8);
            f.write_str(core::mem::transmute(b.get_unchecked(0..1)))
        },
        _ if v <= 0xFF => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(core::mem::transmute(b.as_slice()))
        },
        _ if v <= 0xFFFF => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(12) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(8) as u8);
            f.write_str(core::mem::transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(core::mem::transmute(b.as_slice()))
        },
        _ => unsafe {
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(28) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(24) as u8);
            f.write_str(core::mem::transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(20) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(16) as u8);
            f.write_str(core::mem::transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(12) as u8);
            *b.get_unchecked_mut(1) = hex(v.unchecked_shr(8) as u8);
            f.write_str(core::mem::transmute(b.as_slice()))?;
            *b.get_unchecked_mut(0) = hex(v.unchecked_shr(4) as u8);
            *b.get_unchecked_mut(1) = hex(v as u8);
            f.write_str(core::mem::transmute(b.as_slice()))
        },
    }
}

#[cfg(feature = "alloc")]
mod inner {
    extern crate alloc;
    extern crate core;

    use alloc::boxed::Box;
    use alloc::collections::TryReserveError;
    use core::convert::{AsMut, AsRef, From, Into};
    use core::marker::{Send, Sync};
    use core::option::Option::{self, None, Some};
    use core::result::Result::{self, Err, Ok};

    use crate::{CoreError, Error, ErrorKind};

    pub(super) struct ErrorExtra(Option<Box<Inner>>);

    struct Inner(Box<dyn CoreError + Send + Sync>);

    impl Error {
        /// Creates a new I/O error from a known kind of error as well as an
        /// arbitrary error payload.
        ///
        /// This function is used to generically create I/O errors which do not
        /// originate from the OS itself. The `error` argument is an arbitrary
        /// payload which will be contained in this [`Error`].
        ///
        /// Note that this function allocates memory on the heap.
        /// If no extra payload is required, use the `From` conversion from
        /// `ErrorKind`.
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        ///
        /// // errors can be created from strings
        /// let custom_error = Error::new(ErrorKind::Other, "oh no!");
        ///
        /// // errors can also be created from other errors
        /// let custom_error2 = Error::new(ErrorKind::Interrupted, custom_error);
        ///
        /// // creating an error without payload (and without memory allocation)
        /// let eof_error = Error::from(ErrorKind::UnexpectedEof);
        /// ```
        #[inline]
        pub fn new(kind: ErrorKind, e: impl Into<Box<dyn CoreError + Send + Sync>>) -> Error {
            Error {
                kind:  unsafe { (kind as u32).unchecked_shl(24) },
                extra: ErrorExtra(Some(Box::new(Inner(e.into())))),
            }
        }

        /// Returns a reference to the inner error wrapped by this error (if
        /// any).
        ///
        /// If this [`Error`] was constructed via [`new`] then this function
        /// will return [`Some`], otherwise it will return [`None`].
        ///
        /// [`new`]: Error::new
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        ///
        /// fn print_error(err: &Error) {
        ///     if let Some(inner_err) = err.get_ref() {
        ///         println!("Inner error: {inner_err:?}");
        ///     } else {
        ///         println!("No inner error");
        ///     }
        /// }
        ///
        /// fn main() {
        ///     // Will print "No inner error".
        ///     print_error(&Error::last_os_error());
        ///     // Will print "Inner error: ...".
        ///     print_error(&Error::new(ErrorKind::Other, "oh no!"));
        /// }
        /// ```
        #[inline]
        pub fn get_ref(&self) -> Option<&(dyn CoreError + Send + Sync)> {
            self.extra.0.as_ref().map(|v| v.as_ref().0.as_ref())
        }
        /// Consumes the `Error`, returning its inner error (if any).
        ///
        /// If this [`Error`] was constructed via [`new`] or [`other`],
        /// then this function will return [`Some`],
        /// otherwise it will return [`None`].
        ///
        /// [`new`]: Error::new
        /// [`other`]: Error::other
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        ///
        /// fn print_error(err: Error) {
        ///     if let Some(inner_err) = err.into_inner() {
        ///         println!("Inner error: {inner_err}");
        ///     } else {
        ///         println!("No inner error");
        ///     }
        /// }
        ///
        /// fn main() {
        ///     // Will print "No inner error".
        ///     print_error(Error::last_os_error());
        ///     // Will print "Inner error: ...".
        ///     print_error(Error::new(ErrorKind::Other, "oh no!"));
        /// }
        /// ```
        #[inline]
        pub fn into_inner(self) -> Option<Box<dyn CoreError + Send + Sync>> {
            self.extra.0.map(|v| v.0)
        }
        /// Returns a mutable reference to the inner error wrapped by this error
        /// (if any).
        ///
        /// If this [`Error`] was constructed via [`new`] then this function
        /// will return [`Some`], otherwise it will return [`None`].
        ///
        /// [`new`]: Error::new
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        /// use xrmt_stx::{error, fmt};
        /// use xrmt_stx::fmt::Display;
        ///
        /// #[derive(Debug)]
        /// struct MyError {
        ///     v: String,
        /// }
        ///
        /// impl MyError {
        ///     fn new() -> MyError {
        ///         MyError {
        ///             v: "oh no!".to_string()
        ///         }
        ///     }
        ///
        ///     fn change_message(&mut self, new_message: &str) {
        ///         self.v = new_message.to_string();
        ///     }
        /// }
        ///
        /// impl error::Error for MyError {}
        ///
        /// impl Display for MyError {
        ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ///         write!(f, "MyError: {}", self.v)
        ///     }
        /// }
        ///
        /// fn change_error(mut err: Error) -> Error {
        ///     if let Some(inner_err) = err.get_mut() {
        ///         inner_err.downcast_mut::<MyError>().unwrap().change_message("I've been changed!");
        ///     }
        ///     err
        /// }
        ///
        /// fn print_error(err: &Error) {
        ///     if let Some(inner_err) = err.get_ref() {
        ///         println!("Inner error: {inner_err}");
        ///     } else {
        ///         println!("No inner error");
        ///     }
        /// }
        ///
        /// fn main() {
        ///     // Will print "No inner error".
        ///     print_error(&change_error(Error::last_os_error()));
        ///     // Will print "Inner error: ...".
        ///     print_error(&change_error(Error::new(ErrorKind::Other, MyError::new())));
        /// }
        /// ```
        #[inline]
        pub fn get_mut(&mut self) -> Option<&mut (dyn CoreError + Send + Sync)> {
            self.extra.0.as_mut().map(|v| ErrorExtra::_ref(v.as_mut()))
        }
        /// Attempts to downcast the custom boxed error to `E`.
        ///
        /// If this [`Error`] contains a custom boxed error,
        /// then it would attempt downcasting on the boxed error,
        /// otherwise it will return [`Err`].
        ///
        /// If the custom boxed error has the same type as `E`, it will return
        /// [`Ok`], otherwise it will also return [`Err`].
        ///
        /// This method is meant to be a convenience routine for calling
        /// `Box<dyn Error + Sync + Send>::downcast` on the custom boxed error,
        /// returned by [`Error::into_inner`].
        ///
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::fmt;
        /// use xrmt_stx::io::{self, IoResult};
        /// use xrmt_stx::error::Error;
        ///
        /// #[derive(Debug)]
        /// enum E {
        ///     Io(io::Error),
        ///     SomeOtherVariant,
        /// }
        ///
        /// impl fmt::Display for E {
        ///    // ...
        /// }
        /// impl Error for E {}
        ///
        /// impl From<io::Error> for E {
        ///     fn from(err: io::Error) -> E {
        ///         err.downcast::<E>()
        ///             .unwrap_or_else(E::Io)
        ///     }
        /// }
        ///
        /// impl From<E> for io::Error {
        ///     fn from(err: E) -> io::Error {
        ///         match err {
        ///             E::Io(io_error) => io_error,
        ///             e => io::Error::new(io::ErrorKind::Other, e),
        ///         }
        ///     }
        /// }
        ///
        /// # fn main() {
        /// let e = E::SomeOtherVariant;
        /// // Convert it to an io::Error
        /// let io_error = io::Error::from(e);
        /// // Cast it back to the original variant
        /// let e = E::from(io_error);
        /// assert!(matches!(e, E::SomeOtherVariant));
        ///
        /// let io_error = io::Error::from(io::ErrorKind::AlreadyExists);
        /// // Convert it to E
        /// let e = E::from(io_error);
        /// // Cast it back to the original variant
        /// let io_error = io::Error::from(e);
        /// assert_eq!(io_error.kind(), io::ErrorKind::AlreadyExists);
        /// assert!(io_error.get_ref().is_none());
        /// assert!(io_error.raw_os_error().is_none());
        /// # }
        /// ```
        #[inline]
        pub fn downcast<E: CoreError + Send + Sync + 'static>(self) -> Result<E, Error> {
            if self
                .extra
                .0
                .as_ref()
                .map(|v| v.as_ref().0.as_ref().is::<E>())
                .unwrap_or(false)
            {
                // Already checked value, no need to verify again.
                Ok(unsafe { *self.into_inner().unwrap_unchecked().downcast::<E>().unwrap_unchecked() })
            } else {
                Err(self)
            }
        }
    }
    impl ErrorExtra {
        #[inline]
        pub fn none() -> ErrorExtra {
            ErrorExtra(None)
        }
        #[inline]
        pub fn is_none(&self) -> bool {
            self.0.is_none()
        }

        #[inline]
        fn _ref(v: &mut Inner) -> &mut (dyn CoreError + Send + Sync) {
            v.0.as_mut()
        }
    }

    impl From<TryReserveError> for Error {
        #[inline]
        fn from(_v: TryReserveError) -> Error {
            Error::from(ErrorKind::OutOfMemory)
        }
    }
}
#[cfg(not(feature = "alloc"))]
mod inner {
    extern crate core;

    use core::marker::{Send, Sync};
    use core::option::Option::{self, None};
    use core::result::Result::{self, Err};

    use crate::{CoreError, Error};

    pub(super) struct ErrorExtra(());

    impl Error {
        /// Returns a reference to the inner error wrapped by this error (if
        /// any).
        ///
        /// If this [`Error`] was constructed via [`new`] then this function
        /// will return [`Some`], otherwise it will return [`None`].
        ///
        /// [`new`]: Error::new
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        ///
        /// fn print_error(err: &Error) {
        ///     if let Some(inner_err) = err.get_ref() {
        ///         println!("Inner error: {inner_err:?}");
        ///     } else {
        ///         println!("No inner error");
        ///     }
        /// }
        ///
        /// fn main() {
        ///     // Will print "No inner error".
        ///     print_error(&Error::last_os_error());
        ///     // Will print "Inner error: ...".
        ///     print_error(&Error::new(ErrorKind::Other, "oh no!"));
        /// }
        /// ```
        #[inline]
        pub fn get_ref(&self) -> Option<&(dyn CoreError + Send + Sync)> {
            None
        }
        /// Returns a mutable reference to the inner error wrapped by this error
        /// (if any).
        ///
        /// If this [`Error`] was constructed via [`new`] then this function
        /// will return [`Some`], otherwise it will return [`None`].
        ///
        /// [`new`]: Error::new
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::io::{Error, ErrorKind};
        /// use xrmt_stx::{error, fmt};
        /// use xrmt_stx::fmt::Display;
        ///
        /// #[derive(Debug)]
        /// struct MyError {
        ///     v: String,
        /// }
        ///
        /// impl MyError {
        ///     fn new() -> MyError {
        ///         MyError {
        ///             v: "oh no!".to_string()
        ///         }
        ///     }
        ///
        ///     fn change_message(&mut self, new_message: &str) {
        ///         self.v = new_message.to_string();
        ///     }
        /// }
        ///
        /// impl error::Error for MyError {}
        ///
        /// impl Display for MyError {
        ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ///         write!(f, "MyError: {}", self.v)
        ///     }
        /// }
        ///
        /// fn change_error(mut err: Error) -> Error {
        ///     if let Some(inner_err) = err.get_mut() {
        ///         inner_err.downcast_mut::<MyError>().unwrap().change_message("I've been changed!");
        ///     }
        ///     err
        /// }
        ///
        /// fn print_error(err: &Error) {
        ///     if let Some(inner_err) = err.get_ref() {
        ///         println!("Inner error: {inner_err}");
        ///     } else {
        ///         println!("No inner error");
        ///     }
        /// }
        ///
        /// fn main() {
        ///     // Will print "No inner error".
        ///     print_error(&change_error(Error::last_os_error()));
        ///     // Will print "Inner error: ...".
        ///     print_error(&change_error(Error::new(ErrorKind::Other, MyError::new())));
        /// }
        /// ```
        #[inline]
        pub fn get_mut(&mut self) -> Option<&mut (dyn CoreError + Send + Sync)> {
            None
        }
        /// Attempts to downcast the custom boxed error to `E`.
        ///
        /// If this [`Error`] contains a custom boxed error,
        /// then it would attempt downcasting on the boxed error,
        /// otherwise it will return [`Err`].
        ///
        /// If the custom boxed error has the same type as `E`, it will return
        /// [`Ok`], otherwise it will also return [`Err`].
        ///
        /// This method is meant to be a convenience routine for calling
        /// `Box<dyn Error + Sync + Send>::downcast` on the custom boxed error,
        /// returned by [`Error::into_inner`].
        ///
        ///
        /// # Examples
        ///
        /// ```
        /// use xrmt_stx::fmt;
        /// use xrmt_stx::io::{self, IoResult};
        /// use xrmt_stx::error::Error;
        ///
        /// #[derive(Debug)]
        /// enum E {
        ///     Io(io::Error),
        ///     SomeOtherVariant,
        /// }
        ///
        /// impl fmt::Display for E {
        ///    // ...
        /// }
        /// impl Error for E {}
        ///
        /// impl From<io::Error> for E {
        ///     fn from(err: io::Error) -> E {
        ///         err.downcast::<E>()
        ///             .unwrap_or_else(E::Io)
        ///     }
        /// }
        ///
        /// impl From<E> for io::Error {
        ///     fn from(err: E) -> io::Error {
        ///         match err {
        ///             E::Io(io_error) => io_error,
        ///             e => io::Error::new(io::ErrorKind::Other, e),
        ///         }
        ///     }
        /// }
        ///
        /// # fn main() {
        /// let e = E::SomeOtherVariant;
        /// // Convert it to an io::Error
        /// let io_error = io::Error::from(e);
        /// // Cast it back to the original variant
        /// let e = E::from(io_error);
        /// assert!(matches!(e, E::SomeOtherVariant));
        ///
        /// let io_error = io::Error::from(io::ErrorKind::AlreadyExists);
        /// // Convert it to E
        /// let e = E::from(io_error);
        /// // Cast it back to the original variant
        /// let io_error = io::Error::from(e);
        /// assert_eq!(io_error.kind(), io::ErrorKind::AlreadyExists);
        /// assert!(io_error.get_ref().is_none());
        /// assert!(io_error.raw_os_error().is_none());
        /// # }
        /// ```
        #[inline]
        pub fn downcast<E: CoreError + Send + Sync + 'static>(self) -> Result<E, Error> {
            Err(self)
        }
    }
    impl ErrorExtra {
        #[inline]
        pub fn none() -> ErrorExtra {
            ErrorExtra(())
        }
        #[inline]
        pub fn is_none(&self) -> bool {
            true
        }
    }
}
