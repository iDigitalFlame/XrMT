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
#![cfg(target_family = "windows")]

use crate::device::winapi::{self, AsHandle, Handle};
use crate::io::{self, BufRead, BufReader, Error, ErrorKind, Lines, Read, Write};
use crate::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub(super) use self::inner::write_console;

pub struct Stdin(Handle);
pub struct Stdout(Handle);
pub struct Stderr(Handle);
pub struct StdinLock<'a> {
    buf: BufReader<&'a Stdin>,
    i:   &'a Stdin,
}
pub struct StdoutLock<'a>(&'a Stdout);
pub struct StderrLock<'a>(&'a Stderr);

impl Stdin {
    #[inline]
    pub fn get() -> Stdin {
        Stdin(winapi::GetCurrentProcessPEB().process_params().standard_input)
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn lock<'a>(&'a self) -> StdinLock<'a> {
        StdinLock {
            i:   self,
            buf: BufReader::new(self),
        }
    }
    #[inline]
    pub fn lines<'a>(&'a self) -> Lines<StdinLock<'a>> {
        self.lock().lines()
    }
    #[inline]
    pub fn read_line(&self, buf: &mut String) -> io::Result<usize> {
        self.lock().read_line(buf)
    }
}
impl Stdout {
    #[inline]
    pub fn get() -> Stdout {
        Stdout(winapi::GetCurrentProcessPEB().process_params().standard_output)
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn lock<'a>(&'a self) -> StdoutLock<'a> {
        StdoutLock(self)
    }
}
impl Stderr {
    #[inline]
    pub fn get() -> Stderr {
        Stderr(winapi::GetCurrentProcessPEB().process_params().standard_error)
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn lock<'a>(&'a self) -> StderrLock<'a> {
        StderrLock(self)
    }
}

impl Read for Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtReadFile(self.0, None, buf, None).map_err(Error::from)
    }
}
impl Read for &Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtReadFile(self.0, None, buf, None).map_err(Error::from)
    }
}
impl Read for StdinLock<'_> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.buf.read(buf)
    }
}
impl BufRead for StdinLock<'_> {
    #[inline]
    fn consume(&mut self, amt: usize) {
        self.buf.consume(amt)
    }
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.buf.fill_buf()
    }
}

impl Write for Stdout {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0, buf).map_err(Error::from)
    }
}
impl Write for &Stdout {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0, buf).map_err(Error::from)
    }
}
impl Write for StdoutLock<'_> {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0 .0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0 .0, buf).map_err(Error::from)
    }
}

impl Write for Stderr {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0, buf).map_err(Error::from)
    }
}
impl Write for &Stderr {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0, buf).map_err(Error::from)
    }
}
impl Write for StderrLock<'_> {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        winapi::NtFlushBuffersFile(self.0 .0).map_err(Error::from)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.0.is_invalid() {
            return Err(ErrorKind::Unsupported.into());
        }
        inner::write_console(self.0 .0, buf).map_err(Error::from)
    }
}

impl AsHandle for Stdin {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0
    }
}
impl AsHandle for StdinLock<'_> {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.i.0
    }
}

impl AsHandle for Stdout {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0
    }
}
impl AsHandle for StdoutLock<'_> {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0 .0
    }
}

impl AsHandle for Stderr {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0
    }
}
impl AsHandle for StderrLock<'_> {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0 .0
    }
}

#[cfg(feature = "bugs")]
mod inner {
    use core::cmp;

    use crate::device::winapi::{self, Handle, Win32Result};
    use crate::prelude::*;

    pub fn write_console(h: Handle, buf: &[u8]) -> Win32Result<usize> {
        let mut n = 0u32;
        let r = unsafe {
            WriteConsoleA(
                h,
                buf.as_ptr(),
                cmp::min(buf.len(), 0xFFFFFFFF) as u32,
                &mut n,
                0,
            )
        };
        if r == 0 {
            Err(winapi::last_error())
        } else {
            Ok(n as usize)
        }
    }

    // This is used to allow for free use of 'print{,ln}' when debugging and
    // will not deadlock in sensitive areas.
    #[link(name = "kernel32")]
    extern "stdcall" {
        fn WriteConsoleA(h: Handle, s: *const u8, n: u32, w: *mut u32, r: u32) -> u32;
    }
}
#[cfg(not(feature = "bugs"))]
mod inner {
    use crate::device::winapi::{self, Handle, Win32Result};
    use crate::prelude::*;

    #[inline]
    pub fn write_console(h: Handle, buf: &[u8]) -> Win32Result<usize> {
        if !winapi::is_min_windows_8() {
            winapi::WriteConsole(h, buf)
        } else {
            winapi::NtWriteFile(h, None, buf, None)
        }
    }
}
