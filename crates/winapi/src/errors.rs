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
#![cfg(target_family = "windows")]

extern crate core;

extern crate xrmt_data;
extern crate xrmt_io;

use core::cmp::{Eq, Ord, PartialEq};
use core::convert::From;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::iter::Iterator;
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Ok};

use xrmt_data::text::{utf16_to_func, write_hex_u32};
use xrmt_io::{ErrorKind, FmtResult, IoError, RawOsError};

use crate::functions::{FindMessage, GetLastError};
use crate::loader::kernel32_or_base_address;
use crate::ntdll;
use crate::structs::WCharSlice;

pub enum Win32Error {
    // Messsage(Box<String>), // n/a
    // ^ I don't think we need this as we barely use it.
    Code(u32),          // n/a (Win32 Error Codes)
    Status(u32),        // n/a (NT Status Codes)
    Alerted,            // 0x00000101 | STATUS_ALERTED
    TimedOut,           // 0x00000102 | STATUS_TIMEOUT (Also: 0xC00000B5)
    IoPending,          // 0x00000103 | STATUS_PENDING (Also: 0x40000013)
    NotAllAssigned,     // 0x00000106 | STATUS_NOT_ALL_ASSIGNED
    InvalidHeader,      // 0x4000000E | STATUS_BAD_INITIAL_PC (Also: 0x40000023, 0xC000000A)
    Other,              // 0xC0000001 | STATUS_UNSUCCESSFUL
    NotImplemented,     // 0xC0000002 | STATUS_NOT_IMPLEMENTED
    InvalidHandle,      // 0xC0000008 | STATUS_INVALID_HANDLE
    InvalidArgument,    // 0xC000000D | STATUS_INVALID_PARAMETER
    InvalidOperation,   // 0xC0000010 | STATUS_INVALID_DEVICE_REQUEST (Also: 0xC0020035)
    EndOfFile,          // 0xC0000011 | STATUS_END_OF_FILE (Also: 0x80000006, 0x80000012, 0x8000001A, 0x8000001E)
    PermissionDenied,   // 0xC0000022 | STATUS_ACCESS_DENIED
    InvalidSize,        // 0xC0000023 | STATUS_BUFFER_TOO_SMALL (Also: 0x80000005, 0xC0000004, 0xC000001F)
    InvalidType,        // 0xC0000024 | STATUS_OBJECT_TYPE_MISMATCH (Also: 0xC0000003)
    InvalidName,        // 0xC0000033 | STATUS_OBJECT_NAME_INVALID (Also: 0xC000003B)
    NotFound,           // 0xC0000034 | STATUS_OBJECT_NAME_NOT_FOUND (Also: 0xC000000E, 0xC000000F, 0xC000003A, 0xC0000135)
    AlreadyExists,      // 0xC0000035 | STATUS_OBJECT_NAME_COLLISION (Also: 0x40000000)
    InvalidImage,       // 0xC000007B | STATUS_INVALID_IMAGE_FORMAT
    IsNotFile,          // 0xC00000BA | STATUS_FILE_IS_A_DIRECTORY
    Interrupted,        // 0x000000C0 | STATUS_USER_APC (Also: 0xC0000240)
    DirectoryNotEmpty,  // 0xC0000101 | STATUS_DIRECTORY_NOT_EMPTY
    IsNotDirectory,     // 0xC0000103 | STATUS_NOT_A_DIRECTORY (Also: 0xC0000039)
    BrokenPipe,         // 0xC000014B | STATUS_PIPE_BROKEN (Also: 0x8000001D)
    InvalidObject,      // 0xC0000232 | STATUS_INVALID_VARIANT
    NetworkUnreachable, // 0xC000023C | STATUS_NETWORK_UNREACHABLE
    HostUnreachable,    // 0xC000023D | STATUS_HOST_UNREACHABLE (Also: 0xC000023E)
    InvalidLibrary,     // 0xC000036F | STATUS_INVALID_IMPORT_OF_NON_DLL
    ResourceBusy,       // 0xC0000708 | STATUS_RESOURCE_IN_USE (Also: 0x80000011)
    TooLarge,           // 0xC0000904 | STATUS_FILE_TOO_LARGE (Also: 0xC0000040, 0xC0000050)
}

pub type Win32Result<T> = Result<T, Win32Error>;

impl Win32Error {
    /// Use this for Win32 (and WinSock) Error Codes
    #[inline]
    pub const fn from_code(c: u32) -> Win32Error {
        match c {
            0x000000C0 => Win32Error::Interrupted,
            0x00000101 => Win32Error::Alerted,
            0x00000102 => Win32Error::TimedOut,
            0x00000103 => Win32Error::IoPending,
            0x00000106 => Win32Error::NotAllAssigned,
            0x40000000 => Win32Error::AlreadyExists,
            0x4000000E | 0x40000023 => Win32Error::InvalidHeader,
            0x80000005 => Win32Error::InvalidSize,
            0x80000006 => Win32Error::EndOfFile,
            0x80000011 => Win32Error::ResourceBusy,
            0x80000012 | 0x8000001A => Win32Error::EndOfFile,
            0x8000001D => Win32Error::BrokenPipe,
            0x8000001E => Win32Error::EndOfFile,
            0xC0000001 => Win32Error::Other,
            0xC0000002 => Win32Error::NotImplemented,
            0xC0000003 => Win32Error::InvalidType,
            0xC0000004 => Win32Error::InvalidSize,
            0xC0000008 => Win32Error::InvalidHandle,
            0xC000000A => Win32Error::InvalidHeader,
            0xC000000D => Win32Error::InvalidArgument,
            0xC000000E | 0xC000000F => Win32Error::NotFound,
            0xC0000010 => Win32Error::InvalidOperation,
            0xC0000011 => Win32Error::EndOfFile,
            0xC000001F => Win32Error::InvalidSize,
            0xC0000022 => Win32Error::PermissionDenied,
            0xC0000023 => Win32Error::InvalidSize,
            0xC0000024 => Win32Error::InvalidType,
            0xC0000033 => Win32Error::InvalidName,
            0xC0000034 => Win32Error::NotFound,
            0xC0000035 => Win32Error::AlreadyExists,
            0xC0000039 => Win32Error::IsNotDirectory,
            0xC000003A => Win32Error::NotFound,
            0xC000003B => Win32Error::InvalidName,
            0xC0000040 | 0xC0000050 => Win32Error::TooLarge,
            0xC000007B => Win32Error::InvalidImage,
            0xC00000B5 => Win32Error::TimedOut,
            0xC00000BA => Win32Error::IsNotFile,
            0xC00000BF => Win32Error::ResourceBusy,
            0xC0000101 => Win32Error::DirectoryNotEmpty,
            0xC0000103 => Win32Error::IsNotDirectory,
            0xC0000135 => Win32Error::NotFound,
            0xC000014B => Win32Error::BrokenPipe,
            0xC0000232 => Win32Error::InvalidObject,
            0xC000023C => Win32Error::NetworkUnreachable,
            0xC000023D | 0xC000023E => Win32Error::HostUnreachable,
            0xC0000240 => Win32Error::Interrupted,
            0xC000036F => Win32Error::InvalidLibrary,
            0xC0000708 => Win32Error::ResourceBusy,
            0xC0000904 => Win32Error::TooLarge,
            0xC0020035 => Win32Error::InvalidOperation,
            // WinSock Errors
            // -----------------------
            // WSA_INVALID_HANDLE
            0x6 => Win32Error::InvalidHandle,
            // WSA_INVALID_PARAMETER
            0x57 => Win32Error::InvalidArgument,
            // WSA_OPERATION_ABORTED
            0x3E3 => Win32Error::Interrupted,
            // WSA_IO_INCOMPLETE | WSA_IO_PENDING
            0x3E4 | 0x3E5 => Win32Error::IoPending,
            // WSAEINTR
            0x2714 => Win32Error::Interrupted,
            // WSAEBADF
            0x2719 => Win32Error::InvalidHandle,
            // WSAEACCES
            0x271D => Win32Error::PermissionDenied,
            // WSAEFAULT
            0x271E => Win32Error::InvalidObject,
            // WSAEINVAL
            0x2726 => Win32Error::InvalidArgument,
            // WSAEWOULDBLOCK | WSAEINPROGRESS | WSAEALREADY
            0x2733 | 0x2734 | 0x2735 => Win32Error::IoPending,
            // WSAENOTSOCK
            0x2736 => Win32Error::InvalidHandle,
            // WSAEMSGSIZE
            0x2738 => Win32Error::TooLarge,
            // WSAENOPROTOOPT
            0x273A => Win32Error::InvalidOperation,
            // WSAEOPNOTSUPP
            0x273D => Win32Error::NotImplemented,
            // WSAEADDRINUSE
            0x2740 => Win32Error::AlreadyExists,
            // WSAENETDOWN | WSAENETUNREACH
            0x2742 | 0x2743 => Win32Error::NetworkUnreachable,
            // WSAENETRESET
            0x2744 => Win32Error::Interrupted,
            // WSAECONNABORTED | WSAECONNRESET
            0x2745 | 0x2746 => Win32Error::Interrupted,
            // WSAESHUTDOWN
            // BUG(dij): Update and change back to 'Ok' if this breaks WinSock
            0x274A => Win32Error::BrokenPipe,
            // WSAETIMEDOUT
            0x274C => Win32Error::TimedOut,
            // WSAENAMETOOLONG
            0x274F => Win32Error::InvalidName,
            0x2750 | 0x2751 => Win32Error::HostUnreachable,
            // WSAENOTEMPTY
            0x2752 => Win32Error::DirectoryNotEmpty,
            // WSANOTINITIALISED
            0x276D => Win32Error::InvalidOperation,
            // WSAENOMORE
            0x2776 => Win32Error::EndOfFile,
            // WSAECANCELLED
            0x2777 => Win32Error::Interrupted,
            // WSASERVICE_NOT_FOUND | WSATYPE_NOT_FOUND
            0x277C | 0x277D => Win32Error::NotFound,
            // WSA_E_NO_MORE
            0x277E => Win32Error::EndOfFile,
            // WSA_E_CANCELLED
            0x277F => Win32Error::Interrupted,
            // WSATRY_AGAIN
            0x2AFA => Win32Error::IoPending,
            _ => Win32Error::Code(c),
        }
    }
    /// Use this for NT Status Codes
    #[inline]
    pub const fn from_status(c: u32) -> Win32Error {
        match c {
            0x000000C0 => Win32Error::Interrupted,
            0x00000101 => Win32Error::Alerted,
            0x00000102 => Win32Error::TimedOut,
            0x00000103 => Win32Error::IoPending,
            0x00000106 => Win32Error::NotAllAssigned,
            0x40000000 => Win32Error::AlreadyExists,
            0x4000000E | 0x40000023 => Win32Error::InvalidHeader,
            0x80000005 => Win32Error::InvalidSize,
            0x80000006 => Win32Error::EndOfFile,
            0x80000011 => Win32Error::ResourceBusy,
            0x80000012 | 0x8000001A => Win32Error::EndOfFile,
            0x8000001D => Win32Error::BrokenPipe,
            0x8000001E => Win32Error::EndOfFile,
            0xC0000001 => Win32Error::Other,
            0xC0000002 => Win32Error::NotImplemented,
            0xC0000003 => Win32Error::InvalidType,
            0xC0000004 => Win32Error::InvalidSize,
            0xC0000008 => Win32Error::InvalidHandle,
            0xC000000A => Win32Error::InvalidHeader,
            0xC000000D => Win32Error::InvalidArgument,
            0xC000000E | 0xC000000F => Win32Error::NotFound,
            0xC0000010 => Win32Error::InvalidOperation,
            0xC0000011 => Win32Error::EndOfFile,
            0xC000001F => Win32Error::InvalidSize,
            0xC0000022 => Win32Error::PermissionDenied,
            0xC0000023 => Win32Error::InvalidSize,
            0xC0000024 => Win32Error::InvalidType,
            0xC0000033 => Win32Error::InvalidName,
            0xC0000034 => Win32Error::NotFound,
            0xC0000035 => Win32Error::AlreadyExists,
            0xC0000039 => Win32Error::IsNotDirectory,
            0xC000003A => Win32Error::NotFound,
            0xC000003B => Win32Error::InvalidName,
            0xC0000040 | 0xC0000050 => Win32Error::TooLarge,
            0xC000007B => Win32Error::InvalidImage,
            0xC00000B5 => Win32Error::TimedOut,
            0xC00000BA => Win32Error::IsNotFile,
            0xC00000BF => Win32Error::ResourceBusy,
            0xC0000101 => Win32Error::DirectoryNotEmpty,
            0xC0000103 => Win32Error::IsNotDirectory,
            0xC0000135 => Win32Error::NotFound,
            0xC000014B => Win32Error::BrokenPipe,
            0xC0000232 => Win32Error::InvalidObject,
            0xC000023C => Win32Error::NetworkUnreachable,
            0xC000023D | 0xC000023E => Win32Error::HostUnreachable,
            0xC0000240 => Win32Error::Interrupted,
            0xC000036F => Win32Error::InvalidLibrary,
            0xC0000708 => Win32Error::ResourceBusy,
            0xC0000904 => Win32Error::TooLarge,
            0xC0020035 => Win32Error::InvalidOperation,
            _ => Win32Error::Status(c),
        }
    }
    #[inline]
    pub const fn from_errorkind(v: ErrorKind) -> Win32Error {
        match v {
            ErrorKind::NotFound => Win32Error::NotFound,
            ErrorKind::PermissionDenied => Win32Error::PermissionDenied,
            ErrorKind::ConnectionReset => Win32Error::Interrupted,
            ErrorKind::HostUnreachable => Win32Error::HostUnreachable,
            ErrorKind::NetworkUnreachable => Win32Error::NetworkUnreachable,
            ErrorKind::ConnectionAborted => Win32Error::Interrupted,
            ErrorKind::AddrInUse => Win32Error::AlreadyExists,
            ErrorKind::AddrNotAvailable => Win32Error::NotFound,
            ErrorKind::NetworkDown => Win32Error::NetworkUnreachable,
            ErrorKind::BrokenPipe => Win32Error::BrokenPipe,
            ErrorKind::AlreadyExists => Win32Error::AlreadyExists,
            ErrorKind::WouldBlock => Win32Error::IoPending,
            ErrorKind::NotADirectory => Win32Error::IsNotDirectory,
            ErrorKind::IsADirectory => Win32Error::IsNotFile,
            ErrorKind::DirectoryNotEmpty => Win32Error::DirectoryNotEmpty,
            // 0xC00000A2 - STATUS_MEDIA_WRITE_PROTECTED
            ErrorKind::ReadOnlyFilesystem => Win32Error::Status(0xC00000A2),
            ErrorKind::StaleNetworkFileHandle => Win32Error::InvalidHandle,
            ErrorKind::InvalidInput => Win32Error::InvalidArgument,
            ErrorKind::InvalidData => Win32Error::InvalidOperation,
            ErrorKind::TimedOut => Win32Error::TimedOut,
            // 0x8000000D - STATUS_PARTIAL_COPY
            ErrorKind::WriteZero => Win32Error::Status(0x8000000D),
            // 0xC000007F - STATUS_DISK_FULL
            ErrorKind::StorageFull => Win32Error::Status(0xC000007F),
            ErrorKind::NotSeekable => Win32Error::InvalidOperation,
            // 0xC0000044 - STATUS_QUOTA_EXCEEDED
            ErrorKind::QuotaExceeded => Win32Error::Status(0xC0000044),
            ErrorKind::FileTooLarge => Win32Error::TooLarge,
            ErrorKind::ResourceBusy => Win32Error::ResourceBusy,
            ErrorKind::ExecutableFileBusy => Win32Error::ResourceBusy,
            ErrorKind::Deadlock => Win32Error::ResourceBusy,
            ErrorKind::InvalidFilename => Win32Error::InvalidName,
            ErrorKind::ArgumentListTooLong => Win32Error::InvalidArgument,
            ErrorKind::Interrupted => Win32Error::Interrupted,
            ErrorKind::Unsupported => Win32Error::NotImplemented,
            ErrorKind::UnexpectedEof => Win32Error::EndOfFile,
            // 0xC0000017 - STATUS_NO_MEMORY
            ErrorKind::OutOfMemory => Win32Error::Status(0xC0000017),
            ErrorKind::InProgress => Win32Error::IoPending,
            _ => Win32Error::Other,
        }
    }

    #[inline]
    pub fn last_error() -> Win32Error {
        Win32Error::from_code(GetLastError())
    }

    #[inline]
    pub fn code(&self) -> u32 {
        match self {
            Win32Error::Code(c) => *c,
            Win32Error::Status(c) => *c,
            Win32Error::Alerted => 0x00000101,
            Win32Error::TimedOut => 0x00000102,
            Win32Error::IoPending => 0x00000103,
            Win32Error::NotAllAssigned => 0x00000106,
            Win32Error::InvalidHeader => 0x4000000E,
            Win32Error::Other => 0xC0000001,
            Win32Error::NotImplemented => 0xC0000002,
            Win32Error::InvalidHandle => 0xC0000008,
            Win32Error::InvalidArgument => 0xC000000D,
            Win32Error::InvalidOperation => 0xC0000010,
            Win32Error::EndOfFile => 0xC0000011,
            Win32Error::PermissionDenied => 0xC0000022,
            Win32Error::InvalidSize => 0xC0000023,
            Win32Error::InvalidType => 0xC0000024,
            Win32Error::InvalidName => 0xC0000033,
            Win32Error::NotFound => 0xC0000034,
            Win32Error::AlreadyExists => 0xC0000035,
            Win32Error::InvalidImage => 0xC000007B,
            Win32Error::IsNotFile => 0xC00000BA,
            Win32Error::Interrupted => 0x000000C0,
            Win32Error::DirectoryNotEmpty => 0xC0000101,
            Win32Error::IsNotDirectory => 0xC0000103,
            Win32Error::BrokenPipe => 0xC000014B,
            Win32Error::InvalidObject => 0xC0000232,
            Win32Error::NetworkUnreachable => 0xC000023C,
            Win32Error::HostUnreachable => 0xC000023D,
            Win32Error::InvalidLibrary => 0xC000036F,
            Win32Error::ResourceBusy => 0xC0000708,
            Win32Error::TooLarge => 0xC0000904,
        }
    }
    #[inline]
    pub fn is_pending(&self) -> bool {
        match self {
            Win32Error::IoPending => true,
            _ => false,
        }
    }

    #[inline]
    fn format_const<'a>(&self) -> Option<&'a str> {
        #[cfg(feature = "strip")]
        {
            None
        }
        #[cfg(not(feature = "strip"))]
        match self {
            Win32Error::Alerted => Some("thread was alerted"),
            Win32Error::TimedOut => Some("operation timed out"),
            Win32Error::IoPending => Some("IO pending"),
            Win32Error::NotAllAssigned => Some("not all privileges were assigned"),
            Win32Error::InvalidHeader => Some("DLL header is invalid"),
            Win32Error::Other => Some("unknown error"),
            Win32Error::NotImplemented => Some("not implemented"),
            Win32Error::InvalidHandle => Some("object handle is invalid"),
            Win32Error::InvalidArgument => Some("invalid argument"),
            Win32Error::InvalidOperation => Some("invalid operation"),
            Win32Error::EndOfFile => Some("end of file"),
            Win32Error::PermissionDenied => Some("permission denied"),
            Win32Error::InvalidSize => Some("supplied buffer is too small"),
            Win32Error::InvalidType => Some("object does not match the requested type"),
            Win32Error::InvalidName => Some("object name was invalid"),
            Win32Error::NotFound => Some("object was not found"),
            Win32Error::AlreadyExists => Some("object already exists"),
            Win32Error::InvalidImage => Some("DLL image is not valid"),
            Win32Error::IsNotFile => Some("object is not a file"),
            Win32Error::Interrupted => Some("interrupt delivered"),
            Win32Error::DirectoryNotEmpty => Some("directory not empty"),
            Win32Error::IsNotDirectory => Some("object is not a directory"),
            Win32Error::BrokenPipe => Some("broken pipe"),
            Win32Error::InvalidObject => Some("object is invalid"),
            Win32Error::NetworkUnreachable => Some("network is unreachable"),
            Win32Error::HostUnreachable => Some("host is unreachable"),
            Win32Error::InvalidLibrary => Some("invalid DLL"),
            Win32Error::ResourceBusy => Some("resource is busy"),
            Win32Error::TooLarge => Some("object is too large"),
            _ => None,
        }
    }
    fn format(&self, f: &mut Formatter<'_>) -> FmtResult {
        if let Some(c) = self.format_const() {
            return f.write_str(c);
        }
        let e = self.code();
        if e == 0 {
            #[cfg(feature = "strip")]
            {
                return f.write_str("0x0");
            }
            #[cfg(not(feature = "strip"))]
            {
                return f.write_str("unknown");
            }
        }
        let mut b = [0u16; 300];
        let n = match self {
            Win32Error::Code(_) => false,
            _ => true,
        };
        match format(e, n, &mut b) {
            Some(v) => {
                utf16_to_func(&v, |c| {
                    let _ = f.write_str(c.as_str());
                });
                Ok(())
            },
            None => write_hex_u32(e, f),
        }
    }
}

impl Eq for Win32Error {}
impl Debug for Win32Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
    }
}
impl Error for Win32Error {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for Win32Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.format(f)
    }
}
impl PartialEq for Win32Error {
    #[inline]
    fn eq(&self, other: &Win32Error) -> bool {
        self.code() == other.code()
    }
}

impl From<u32> for Win32Error {
    #[inline]
    fn from(v: u32) -> Win32Error {
        Win32Error::from_code(v)
    }
}
impl From<IoError> for Win32Error {
    #[inline]
    fn from(v: IoError) -> Win32Error {
        Win32Error::from_errorkind(v.kind())
    }
}
impl From<ErrorKind> for Win32Error {
    #[inline]
    fn from(v: ErrorKind) -> Win32Error {
        Win32Error::from_errorkind(v)
    }
}

impl From<Win32Error> for IoError {
    #[inline]
    fn from(v: Win32Error) -> IoError {
        match v {
            Win32Error::Alerted => IoError::from(ErrorKind::Interrupted),
            Win32Error::TimedOut => IoError::from(ErrorKind::TimedOut),
            Win32Error::IoPending => IoError::from(ErrorKind::InProgress),
            Win32Error::NotAllAssigned => IoError::from(ErrorKind::PermissionDenied),
            Win32Error::InvalidHeader => IoError::from(ErrorKind::InvalidData),
            Win32Error::Other => IoError::from(ErrorKind::Other),
            Win32Error::NotImplemented => IoError::from(ErrorKind::Unsupported),
            Win32Error::InvalidHandle | Win32Error::InvalidArgument | Win32Error::InvalidOperation => IoError::from(ErrorKind::InvalidInput),
            Win32Error::EndOfFile => IoError::from(ErrorKind::UnexpectedEof),
            Win32Error::PermissionDenied => IoError::from(ErrorKind::PermissionDenied),
            Win32Error::InvalidSize | Win32Error::InvalidType => IoError::from(ErrorKind::InvalidInput),
            Win32Error::InvalidName => IoError::from(ErrorKind::InvalidFilename),
            Win32Error::NotFound => IoError::from(ErrorKind::NotFound),
            Win32Error::AlreadyExists => IoError::from(ErrorKind::AlreadyExists),
            Win32Error::InvalidImage => IoError::from(ErrorKind::InvalidData),
            Win32Error::IsNotFile => IoError::from(ErrorKind::IsADirectory),
            Win32Error::Interrupted => IoError::from(ErrorKind::Interrupted),
            Win32Error::DirectoryNotEmpty => IoError::from(ErrorKind::DirectoryNotEmpty),
            Win32Error::IsNotDirectory => IoError::from(ErrorKind::NotADirectory),
            Win32Error::BrokenPipe => IoError::from(ErrorKind::BrokenPipe),
            Win32Error::InvalidObject => IoError::from(ErrorKind::InvalidInput),
            Win32Error::NetworkUnreachable => IoError::from(ErrorKind::NetworkUnreachable),
            Win32Error::HostUnreachable => IoError::from(ErrorKind::HostUnreachable),
            Win32Error::InvalidLibrary => IoError::from(ErrorKind::InvalidData),
            Win32Error::ResourceBusy => IoError::from(ErrorKind::ResourceBusy),
            Win32Error::TooLarge => IoError::from(ErrorKind::FileTooLarge),
            Win32Error::Code(e) => match e {
                // Most WinSock Errors are covered above.
                0x2749 => IoError::from(ErrorKind::NotConnected),
                0x2746 => IoError::from(ErrorKind::ConnectionReset),
                0x2741 => IoError::from(ErrorKind::AddrNotAvailable),
                0x274D => IoError::from(ErrorKind::ConnectionRefused),
                _ => IoError::from_raw_os_error(e as RawOsError),
            },
            _ => IoError::new(ErrorKind::Other, v),
        }
    }
}

fn format<'a>(e: u32, nt: bool, buf: &'a mut [u16; 300]) -> Option<WCharSlice<'a>> {
    if e == 0 {
        return None;
    }
    let h = if nt {
        ntdll().address()
    } else {
        kernel32_or_base_address()
    };
    // 0x409 - English LANG and English SUB
    let r = FindMessage(e, 0x409, *h, buf).ok()?;
    // Look for first '{', then '}' to remove placeholder '{}' values, only if 'r'
    // is less than the slice length.
    let s = if r > 0 && unsafe { *buf.get_unchecked(0) == 0x7B } && r < 300 {
        unsafe { buf.get_unchecked(0..r) }
            .iter()
            .position(|v| *v == 0x7D)
            .map(|v|(v + 2).min(r)) // Skip the space
            .unwrap_or(0)
    } else {
        0
    };
    // Remove the newlines (if any), or any periods '.'
    let b = unsafe {
        // 's' can't be larger than 'r'
        buf.get_unchecked(s..r)
            .iter()
            .position(|v| *v == 0xA || *v == 0xD || *v == 0x2E)
            .map_or_else(|| buf.get_unchecked(s..r), |i| buf.get_unchecked(s..i))
    };
    Some(WCharSlice::from(b))
}
