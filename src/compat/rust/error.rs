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

use alloc::string::{String, ToString};
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{From, Into};
use core::fmt::{self, Debug, Display, Formatter, Write};
use core::hash::{Hash, Hasher};
use core::marker::Copy;
use core::option::Option;
use core::option::Option::None;
use core::result::Result::Ok;
use core::{error, result};

use crate::device::winapi::{self, Win32Error};

#[non_exhaustive]
pub enum ErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    HostUnreachable,
    NetworkUnreachable,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    NetworkDown,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    ReadOnlyFilesystem,
    FilesystemLoop,
    StaleNetworkFileHandle,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    StorageFull,
    NotSeekable,
    FileTooLarge,
    ResourceBusy,
    ExecutableFileBusy,
    Deadlock,
    CrossesDevices,
    TooManyLinks,
    InvalidFilename,
    ArgumentListTooLong,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    Other,
    Uncategorized,
}

pub struct Error {
    kind:    ErrorKind,
    message: String,
}

pub type RawOsError = i32;
pub type Result<T> = result::Result<T, Error>;

impl Error {
    #[inline]
    pub fn last_os_error() -> Error {
        winapi::last_error().into()
    }
    #[inline]
    pub fn from_raw_os_error(code: RawOsError) -> Error {
        Win32Error::from_code(code as u32).into()
    }
    #[inline]
    pub fn new(kind: ErrorKind, m: impl Display) -> Error {
        Error { kind, message: m.to_string() }
    }

    #[inline]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    #[inline]
    pub fn raw_os_error(&self) -> Option<RawOsError> {
        None
    }
}

impl Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.kind, f)?;
        if self.message.is_empty() {
            return Ok(());
        }
        f.write_char(' ')?;
        f.write_str(&self.message)
    }
}
impl error::Error for Error {
    #[inline]
    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}
impl From<ErrorKind> for Error {
    #[inline]
    fn from(v: ErrorKind) -> Error {
        Error {
            kind:    v,
            message: String::new(),
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
impl Copy for ErrorKind {}
impl Clone for ErrorKind {
    #[inline]
    fn clone(&self) -> ErrorKind {
        *self
    }
}
impl Hash for ErrorKind {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}
impl Debug for ErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for ErrorKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::NotFound => f.write_str("not found"),
            ErrorKind::PermissionDenied => f.write_str("permission denied"),
            ErrorKind::ConnectionRefused => f.write_str("connection refused"),
            ErrorKind::ConnectionReset => f.write_str("connection reset"),
            ErrorKind::HostUnreachable => f.write_str("host unreachable"),
            ErrorKind::NetworkUnreachable => f.write_str("network unreachable"),
            ErrorKind::ConnectionAborted => f.write_str("connection aborted"),
            ErrorKind::NotConnected => f.write_str("not connected"),
            ErrorKind::AddrInUse => f.write_str("addr in use"),
            ErrorKind::AddrNotAvailable => f.write_str("addr not available"),
            ErrorKind::NetworkDown => f.write_str("network down"),
            ErrorKind::BrokenPipe => f.write_str("broken pipe"),
            ErrorKind::AlreadyExists => f.write_str("already exists"),
            ErrorKind::WouldBlock => f.write_str("would block"),
            ErrorKind::NotADirectory => f.write_str("not a directory"),
            ErrorKind::IsADirectory => f.write_str("is a directory"),
            ErrorKind::DirectoryNotEmpty => f.write_str("directory not empty"),
            ErrorKind::ReadOnlyFilesystem => f.write_str("read only filesystem"),
            ErrorKind::FilesystemLoop => f.write_str("filesystem loop"),
            ErrorKind::StaleNetworkFileHandle => f.write_str("stale network file handle"),
            ErrorKind::InvalidInput => f.write_str("invalid input"),
            ErrorKind::InvalidData => f.write_str("invalid data"),
            ErrorKind::TimedOut => f.write_str("timed out"),
            ErrorKind::WriteZero => f.write_str("write zzero"),
            ErrorKind::StorageFull => f.write_str("storage full"),
            ErrorKind::NotSeekable => f.write_str("not seekable"),
            ErrorKind::FileTooLarge => f.write_str("file too large"),
            ErrorKind::ResourceBusy => f.write_str("resource busy"),
            ErrorKind::ExecutableFileBusy => f.write_str("executable file busy"),
            ErrorKind::Deadlock => f.write_str("deadlock"),
            ErrorKind::CrossesDevices => f.write_str("crosses devices"),
            ErrorKind::TooManyLinks => f.write_str("too many links"),
            ErrorKind::InvalidFilename => f.write_str("invalid filename"),
            ErrorKind::ArgumentListTooLong => f.write_str("argument list too long"),
            ErrorKind::Interrupted => f.write_str("interrupted"),
            ErrorKind::Unsupported => f.write_str("unsupported"),
            ErrorKind::UnexpectedEof => f.write_str("unexpected eof"),
            ErrorKind::OutOfMemory => f.write_str("out of memory"),
            ErrorKind::Other => f.write_str("other"),
            ErrorKind::Uncategorized => f.write_str("uncategorized"),
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
