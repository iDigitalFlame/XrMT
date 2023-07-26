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

extern crate alloc;
extern crate core;

use alloc::string::{String, ToString};
use core::convert::From;
use core::fmt::{self, Debug, Display, Formatter};
use core::option::Option;
use core::option::Option::None;
use core::{error, result};

use crate::util::ToStr;

#[derive(Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
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
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    StorageFull,
    NotSeekable,
    FileTooLarge,
    ResourceBusy,
    CrossesDevices,
    TooManyLinks,
    InvalidFilename,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    Other,
}

pub struct Error {
    message: String,
    kind:    ErrorKind,
}

pub type RawOsError = i32;
pub type Result<T> = result::Result<T, Error>;

impl Error {
    #[inline]
    pub fn last_os_error() -> Error {
        inner::last_error()
    }
    #[inline]
    pub fn from_raw_os_error(code: RawOsError) -> Error {
        inner::from_code(code)
    }
    #[inline]
    pub fn new(kind: ErrorKind, m: impl ToString) -> Error {
        let r = m.to_string();
        Error {
            kind,
            message: if r.is_empty() {
                if cfg!(feature = "implant") {
                    ((kind as u8) + 0x90).into_string()
                } else {
                    kind.to_string()
                }
            } else {
                r
            },
        }
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
        f.write_str(&self.message)
    }
}
impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            message: if cfg!(feature = "implant") {
                ((v as u8) + 0x90).into_string()
            } else {
                v.to_string()
            },
        }
    }
}

impl ToString for ErrorKind {
    #[inline]
    fn to_string(&self) -> String {
        #[cfg(feature = "implant")]
        {
            ((*self as u8) as u16 + 1500).into_string()
        }
        #[cfg(not(feature = "implant"))]
        match self {
            ErrorKind::NotFound => "file not found",
            ErrorKind::PermissionDenied => "permission denied",
            ErrorKind::ConnectionRefused => "connection refused",
            ErrorKind::ConnectionReset => "connection reset",
            ErrorKind::HostUnreachable => "host is unreachable",
            ErrorKind::NetworkUnreachable => "network is unreachable",
            ErrorKind::ConnectionAborted => "connect was aborted",
            ErrorKind::NotConnected => "not connected",
            ErrorKind::AddrInUse => "address in use",
            ErrorKind::AddrNotAvailable => "address not available",
            ErrorKind::NetworkDown => "network is down",
            ErrorKind::BrokenPipe => "broken pipe",
            ErrorKind::AlreadyExists => "already exists",
            ErrorKind::WouldBlock => "would block",
            ErrorKind::NotADirectory => "not a directory",
            ErrorKind::IsADirectory => "is a directory",
            ErrorKind::DirectoryNotEmpty => "directory is not empty",
            ErrorKind::ReadOnlyFilesystem => "read only filesystem",
            ErrorKind::InvalidInput => "invalid argument",
            ErrorKind::InvalidData => "invalid data/input",
            ErrorKind::TimedOut => "timeout",
            ErrorKind::WriteZero => "zero write",
            ErrorKind::StorageFull => "storage full",
            ErrorKind::NotSeekable => "file is not seekable",
            ErrorKind::FileTooLarge => "file is too large",
            ErrorKind::ResourceBusy => "resource is busy",
            ErrorKind::CrossesDevices => "operation crosses devices",
            ErrorKind::TooManyLinks => "too many links",
            ErrorKind::InvalidFilename => "invalid filename",
            ErrorKind::Interrupted => "operation interupted",
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::UnexpectedEof => "unexpected EOF",
            ErrorKind::OutOfMemory => "out of memory",
            ErrorKind::Other => "other",
        }
        .to_string()
    }
}

#[cfg(unix)]
mod inner {}
#[cfg(windows)]
mod inner {
    extern crate core;

    use core::convert::Into;

    use super::Error;
    use crate::device::winapi;

    #[inline]
    pub(super) fn last_error() -> Error {
        winapi::last_error().into()
    }
    #[inline]
    pub(super) fn from_code(code: i32) -> Error {
        winapi::Win32Error::Code(code as u32).into()
    }
}
