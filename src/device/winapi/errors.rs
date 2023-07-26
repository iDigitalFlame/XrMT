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

#![no_implicit_prelude]
#![cfg(windows)]

use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};

use crate::device::winapi;
use crate::device::winapi::loader::{kernel32, ntdll};
use crate::util::stx::io::{self, ErrorKind};
use crate::util::stx::prelude::*;
use crate::util::ToStr;

#[derive(PartialEq, Eq)]
pub enum Win32Error {
    Code(u32),
    Messsage(String),
    IoPending,          // 0x00000103
    InvalidOperation,   // 0xC0000001
    NotImplemented,     // 0xC0000002
    InvalidHandle,      // 0xC0000008
    InvalidHeader,      // 0xC000000A
    InvalidArgument,    // 0xC000000D
    PermissionDenied,   // 0xC0000022
    FileNotFound,       // 0xC0000034
    InvalidImage,       // 0xC000007B
    InvalidForward,     // 0xC00000FF
    DirectoryNotEmpty,  // 0xC0000101
    BrokenPipe,         // 0xC000014B
    InvalidDLL,         // 0xC0000251
    InvalidAddress,     // 0xC01E05E4
    UnexpectedKeySize,  // 0xC0000023
    UnexpectedKeyType,  // 0xC0000024
    TimedOut,           // 0xC00000B5
    ResourceBusy,       // 0xC00000BF
    StorageFull,        // 0xC000007F
    ReadOnlyFilesystem, // 0xC00000A2
    InvalidFilename,    // 0xC0000033
}

pub type Win32Result<T> = Result<T, Win32Error>;

impl Win32Error {
    #[inline]
    pub fn code(&self) -> u32 {
        match self {
            Win32Error::IoPending => 0x103,
            Win32Error::TimedOut => 0xC00000B5,
            Win32Error::InvalidDLL => 0xC0000251,
            Win32Error::BrokenPipe => 0xC000014B,
            Win32Error::StorageFull => 0xC000007F,
            Win32Error::ResourceBusy => 0xC00000BF,
            Win32Error::FileNotFound => 0xC0000034,
            Win32Error::InvalidImage => 0xC000007B,
            Win32Error::InvalidHeader => 0xC000000A,
            Win32Error::InvalidHandle => 0xC0000008,
            Win32Error::InvalidAddress => 0xC01E05E4,
            Win32Error::NotImplemented => 0xC0000002,
            Win32Error::InvalidForward => 0xC00000FF,
            Win32Error::InvalidArgument => 0xC000000D,
            Win32Error::InvalidFilename => 0xC0000033,
            Win32Error::InvalidOperation => 0xC0000001,
            Win32Error::PermissionDenied => 0xC0000022,
            Win32Error::DirectoryNotEmpty => 0xC0000101,
            Win32Error::UnexpectedKeySize => 0xC0000023,
            Win32Error::UnexpectedKeyType => 0xC0000024,
            Win32Error::ReadOnlyFilesystem => 0xC00000A2,
            Win32Error::Code(c) => *c,
            _ => 0,
        }
    }

    fn dynamic_mesage(&self) -> String {
        match self {
            Win32Error::Messsage(v) => return v.to_string(),
            Win32Error::Code(e) => return format_error(*e, false).unwrap_or_else(|| e.into_string()),
            _ => (),
        }
        let e = self.code();
        if e == 0 {
            return if cfg!(feature = "implant") {
                "-0x0".to_string()
            } else {
                "winapi unknown error".to_string()
            };
        }
        format_error(e, true).unwrap_or_else(|| e.into_string())
    }
    #[inline]
    fn static_message<'a>(&self) -> Option<&'a str> {
        #[cfg(feature = "implant")]
        {
            None
        }
        #[cfg(not(feature = "implant"))]
        match self {
            Win32Error::IoPending => Some("io pending"),
            Win32Error::BrokenPipe => Some("broken pipe"),
            Win32Error::FileNotFound => Some("file not found"),
            Win32Error::NotImplemented => Some("not implemented"),
            Win32Error::InvalidArgument => Some("invalid argument"),
            Win32Error::PermissionDenied => Some("permission denied"),
            Win32Error::InvalidOperation => Some("invalid operation"),
            Win32Error::UnexpectedKeySize => Some("unexpected key size"),
            Win32Error::DirectoryNotEmpty => Some("directory not empty"),
            Win32Error::UnexpectedKeyType => Some("unexpected key type"),
            Win32Error::InvalidForward => Some("invalid forward function"),
            _ => None,
        }
    }
}

impl Error for Win32Error {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for Win32Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for Win32Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.static_message() {
            Some(s) => f.write_str(s),
            None => f.write_str(&self.dynamic_mesage()),
        }
    }
}

impl From<Win32Error> for io::Error {
    #[inline]
    fn from(v: Win32Error) -> io::Error {
        match v {
            Win32Error::TimedOut => ErrorKind::TimedOut.into(),
            Win32Error::IoPending => ErrorKind::Interrupted.into(),
            Win32Error::BrokenPipe => ErrorKind::BrokenPipe.into(),
            Win32Error::FileNotFound => ErrorKind::NotFound.into(),
            Win32Error::StorageFull => ErrorKind::StorageFull.into(),
            Win32Error::ResourceBusy => ErrorKind::ResourceBusy.into(),
            Win32Error::NotImplemented => ErrorKind::Unsupported.into(),
            Win32Error::InvalidArgument => ErrorKind::InvalidInput.into(),
            Win32Error::InvalidOperation => ErrorKind::InvalidData.into(),
            Win32Error::DirectoryNotEmpty => ErrorKind::Unsupported.into(),
            Win32Error::InvalidFilename => ErrorKind::InvalidFilename.into(),
            Win32Error::PermissionDenied => ErrorKind::PermissionDenied.into(),
            Win32Error::ReadOnlyFilesystem => ErrorKind::ReadOnlyFilesystem.into(),
            Win32Error::Code(e) => match e {
                0x271D => ErrorKind::PermissionDenied.into(),
                0x2726 => ErrorKind::InvalidInput.into(),
                0x2733 => ErrorKind::WouldBlock.into(),
                0x2740 => ErrorKind::AddrInUse.into(),
                0x2741 => ErrorKind::AddrNotAvailable.into(),
                0x2742 => ErrorKind::NetworkDown.into(),
                0x2743 => ErrorKind::NetworkUnreachable.into(),
                0x2745 => ErrorKind::ConnectionAborted.into(),
                0x2746 => ErrorKind::ConnectionReset.into(),
                0x2749 => ErrorKind::NotConnected.into(),
                0x274C => ErrorKind::TimedOut.into(),
                0x274D => ErrorKind::ConnectionRefused.into(),
                0x2751 => ErrorKind::HostUnreachable.into(),
                _ => io::Error::new(io::ErrorKind::Other, v),
            },
            _ => io::Error::new(io::ErrorKind::Other, v),
        }
    }
}

#[inline]
pub(super) fn nt_error(e: u32) -> Win32Error {
    match e {
        0x00000103 => Win32Error::IoPending,
        0xC0000001 => Win32Error::InvalidOperation,
        0xC0000002 => Win32Error::NotImplemented,
        0xC000000D => Win32Error::InvalidArgument,
        0xC0000022 => Win32Error::PermissionDenied,
        0xC0000033 => Win32Error::InvalidFilename,
        0xC0000034 => Win32Error::FileNotFound,
        0xC000007F => Win32Error::StorageFull,
        0xC00000A2 => Win32Error::ReadOnlyFilesystem,
        0xC00000B5 => Win32Error::TimedOut,
        0xC00000BF => Win32Error::ResourceBusy,
        0xC0000101 => Win32Error::DirectoryNotEmpty,
        0xC000014B => Win32Error::BrokenPipe,
        _ => format_error(e, true).map_or_else(|| Win32Error::Code(e), |v| Win32Error::Messsage(v)),
    }
}

fn format_error(e: u32, nt: bool) -> Option<String> {
    if e == 0 {
        return None;
    }
    winapi::init_kernel32();
    if kernel32::FormatMessage == false {
        return None;
    }
    let mut buf: [u16; 300] = [0; 300];
    let mut r = unsafe {
        // 0x3A00 - FORMAT_MESSAGE_ARGUMENT_ARRAY | FORMAT_MESSAGE_FROM_HMODULE |
        //          FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS
        // 0x409  - English LANG and English SUB
        winapi::syscall!(
            *kernel32::FormatMessage,
            extern "stdcall" fn(u32, usize, u32, u32, *mut u16, u32, usize) -> u32,
            if nt { 0x3A00 } else { 0x3200 },
            if nt { ntdll::address() } else { 0 },
            e,
            0x409,
            buf.as_mut_ptr(),
            300,
            0
        )
    } as usize;
    if r == 0 {
        return None;
    }
    // Remove trailing newline or empty space.
    while r > 0 && (buf[r] == 13 || buf[r] == 10 || buf[r] == 0) {
        r -= 1;
    }
    let mut i = buf[0..r].iter();
    // Strip any newlines and remove placeholder '{}' values to make the message
    // more readable.
    match (i.position(|v| *v == 10), i.position(|v| *v == 10)) {
        (None, None) => Some(winapi::utf16_to_str(&buf[0..r])),
        (Some(j), None) => Some(winapi::utf16_to_str(&buf[j + 1..r])),
        (None, Some(k)) => Some(winapi::utf16_to_str(&buf[0..k])),
        (Some(j), Some(k)) => Some(winapi::utf16_to_str(&buf[j + 1..k + j])),
    }
}
