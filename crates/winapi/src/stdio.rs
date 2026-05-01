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

extern crate xrmt_io;

use core::convert::{AsRef, From};
use core::fmt::Arguments;
use core::option::Option::None;
use core::result::Result::{Err, Ok};

use xrmt_io::{ErrorKind, FmtError, FmtResult, FmtWrite, IoError, IoResult, Read, Write};

use crate::functions::{is_terminal, GetCurrentProcessPEB, NtFlushBuffersFile, NtReadFile};
use crate::structs::Handle;
use crate::Win32Result;

pub struct Stdin(Handle);
pub struct Stdout(Handle);
pub struct Stderr(Handle);

impl Stdin {
    #[inline]
    pub fn get() -> Stdin {
        Stdin(GetCurrentProcessPEB().process_params().standard_input)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
    #[inline]
    pub fn is_terminal(&self) -> bool {
        is_terminal(self)
    }
}
impl Stdout {
    #[inline]
    pub fn get() -> Stdout {
        Stdout(GetCurrentProcessPEB().process_params().standard_output)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.0.is_invalid()
    }
    #[inline]
    pub fn is_terminal(&self) -> bool {
        is_terminal(self)
    }
    // This is here to prevent needing the "fmt::Write" trait in scope for
    // "print/println".
    #[doc(hidden)]
    #[inline]
    pub fn write_fmt(&mut self, args: Arguments<'_>) -> FmtResult {
        FmtWrite::write_fmt(self, args)
    }
}
impl Stderr {
    #[inline]
    pub fn get() -> Stderr {
        Stderr(GetCurrentProcessPEB().process_params().standard_error)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
    #[inline]
    pub fn is_terminal(&self) -> bool {
        is_terminal(self)
    }
    // This is here to prevent needing the "fmt::Write" trait in scope for
    // "eprint/eprintln".
    #[doc(hidden)]
    #[inline]
    pub fn write_fmt(&mut self, args: Arguments<'_>) -> FmtResult {
        FmtWrite::write_fmt(self, args)
    }
}

impl Read for Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtReadFile(self.0, None, buf, None)?)
    }
}
impl Read for &Stdin {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtReadFile(self.0, None, buf, None)?)
    }
}

impl Write for Stdout {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtFlushBuffersFile(self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(inner::write_console(self.0, buf)?)
    }
}
impl Write for &Stdout {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtFlushBuffersFile(self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(inner::write_console(self.0, buf)?)
    }
}
impl FmtWrite for Stdout {
    #[inline]
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.write(s.as_bytes()).map_err(|_| FmtError)?;
        Ok(())
    }
}
impl FmtWrite for &Stdout {
    #[inline]
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.write(s.as_bytes()).map_err(|_| FmtError)?;
        Ok(())
    }
}

impl Write for Stderr {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtFlushBuffersFile(self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(inner::write_console(self.0, buf)?)
    }
}
impl Write for &Stderr {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(NtFlushBuffersFile(self.0)?)
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.0.is_invalid() {
            return Err(IoError::from(ErrorKind::Unsupported));
        }
        Ok(inner::write_console(self.0, buf)?)
    }
}
impl FmtWrite for Stderr {
    #[inline]
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.write(s.as_bytes()).map_err(|_| FmtError)?;
        Ok(())
    }
}
impl FmtWrite for &Stderr {
    #[inline]
    fn write_str(&mut self, s: &str) -> FmtResult {
        self.write(s.as_bytes()).map_err(|_| FmtError)?;
        Ok(())
    }
}

impl AsRef<Handle> for Stdin {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl AsRef<Handle> for Stdout {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl AsRef<Handle> for Stderr {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

#[inline]
pub fn write_stderr(buf: &[u8]) -> Win32Result<usize> {
    inner::write_console(GetCurrentProcessPEB().process_params().standard_error, buf)
}
#[inline]
pub fn write_stdout(buf: &[u8]) -> Win32Result<usize> {
    inner::write_console(GetCurrentProcessPEB().process_params().standard_output, buf)
}

#[macro_export]
macro_rules! stderr {
    ($($arg:tt)*) => {{
        let _ = core::writeln!($crate::stdio::Stderr::get(), $($arg)*);
    }};
}
#[macro_export]
macro_rules! stdout {
    ($($arg:tt)*) => {{
        let _ = core::writeln!($crate::stdio::Stdout::get(), $($arg)*);
    }};
}

#[cfg(feature = "bugs")]
mod inner {
    extern crate core;

    use core::result::Result::{Err, Ok};

    use crate::functions::len_to_u32;
    use crate::structs::Handle;
    use crate::{Win32Error, Win32Result};

    #[inline]
    pub(super) fn write_console(h: Handle, buf: &[u8]) -> Win32Result<usize> {
        if h.is_invalid() {
            return Err(Win32Error::InvalidHandle);
        }
        let mut n = 0u32;
        if unsafe { WriteConsoleA(h, buf.as_ptr(), len_to_u32(buf.len()), &mut n, 0) } == 0 {
            Err(Win32Error::last_error())
        } else {
            Ok(n as usize)
        }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        /// This is used to allow for free use of 'print{,ln}' when debugging
        /// and will not deadlock in sensitive areas.
        unsafe fn WriteConsoleA(h: Handle, s: *const u8, n: u32, w: *mut u32, r: u32) -> u32;
    }
}
#[cfg(not(feature = "bugs"))]
mod inner {
    extern crate core;

    use core::option::Option::None;
    use core::result::Result::Err;

    use crate::functions::{NtWriteFile, WriteConsole};
    use crate::info::is_min_windows_8;
    use crate::structs::Handle;
    use crate::{Win32Error, Win32Result};

    #[inline]
    pub(super) fn write_console(h: Handle, buf: &[u8]) -> Win32Result<usize> {
        if h.is_invalid() {
            return Err(Win32Error::InvalidHandle);
        }
        if !is_min_windows_8() {
            WriteConsole(h, buf)
        } else {
            NtWriteFile(h, None, buf, None)
        }
    }
}
