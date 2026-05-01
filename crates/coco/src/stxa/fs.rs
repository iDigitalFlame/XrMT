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

extern crate core;

extern crate xrmt_stx;

use core::clone::Clone;
use core::convert::{AsRef, From, TryFrom};
use core::default::Default;
use core::mem::transmute;
use core::result::Result::Ok;

use xrmt_stx::fs;
use xrmt_stx::io::{IoError, IoResult, Write};
use xrmt_stx::path::Path;

use crate::stxa::io::{AsyncRead, AsyncWrite};
use crate::stxa::{AsyncFileReader, AsyncFileWriter};

pub struct File {
    pub(super) v: fs::File,
    #[cfg(target_family = "windows")]
    pub(super) p: u64, /* Windows needs to keep track of the file position information
                        * as in OVERLAPPED mode (async), the kernel won't do it for us. */
}
pub struct OpenOptions(fs::OpenOptions);

impl File {
    fn lock(&mut self) {}
}
impl OpenOptions {
    #[inline]
    pub fn new() -> OpenOptions {
        OpenOptions(inner::open_options())
    }

    #[inline]
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        self.0.read(read);
        self
    }
    #[inline]
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        self.0.write(write);
        self
    }
    #[inline]
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        self.0.append(append);
        self
    }
    #[inline]
    pub fn create(&mut self, create: bool) -> &mut OpenOptions {
        self.0.create(create);
        self
    }
    #[inline]
    pub fn open(&self, path: impl AsRef<Path>) -> IoResult<File> {
        File::open(&self.0, path)
    }
    #[inline]
    pub fn truncate(&mut self, truncate: bool) -> &mut OpenOptions {
        self.0.truncate(truncate);
        self
    }
    #[inline]
    pub fn create_new(&mut self, create_new: bool) -> &mut OpenOptions {
        self.0.create_new(create_new);
        self
    }
}

impl Clone for OpenOptions {
    #[inline]
    fn clone(&self) -> OpenOptions {
        OpenOptions(self.0.clone())
    }
}
impl Default for OpenOptions {
    #[inline]
    fn default() -> OpenOptions {
        OpenOptions::new()
    }
}
impl From<fs::OpenOptions> for OpenOptions {
    #[inline]
    fn from(v: fs::OpenOptions) -> OpenOptions {
        OpenOptions(v)
    }
}

impl AsyncRead for File {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        AsyncFileReader::file(self, buf).await
    }
}
impl AsyncWrite for File {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        self.v.flush()?;
        Ok(())
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        AsyncFileWriter::file(self, buf).await
    }
}
impl AsyncRead for &File {
    #[inline]
    async fn async_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        AsyncFileReader::file(unsafe { transmute(self) }, buf).await
    }
}
impl AsyncWrite for &File {
    #[inline]
    async fn async_flush(&mut self) -> IoResult<()> {
        (&self.v).flush()
    }
    #[inline]
    async fn async_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        AsyncFileWriter::file(unsafe { transmute(self) }, buf).await
    }
}

impl TryFrom<fs::File> for File {
    type Error = IoError;

    #[inline]
    fn try_from(v: fs::File) -> IoResult<File> {
        File::new(v)
    }
}

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_stx;
    extern crate xrmt_winapi;

    use core::convert::{AsRef, Into};
    use core::result::Result::Ok;

    use xrmt_stx::fs::{self, OpenOptions};
    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::windows::fs::OpenOptionsExt;
    use xrmt_stx::path::Path;
    use xrmt_winapi::functions::{reopen_file, NtQueryInformationFile};

    use crate::stxa::fs::File;

    impl File {
        #[inline]
        pub(super) fn new(f: fs::File) -> IoResult<File> {
            // 0x40000000 - FILE_SYNCHRONOUS_IO_NONALERT
            let (h, mut p) = (f.into(), 0u64);
            // 0xE - FilePositionInformation
            let _ = NtQueryInformationFile(&h, 0xE, &mut p, 8);
            // Get the current file pos and reopen the file for async ops.
            Ok(File {
                v: reopen_file(h, false, 0x40000000)?.into(),
                p,
            })
        }
        #[inline]
        pub(super) fn open(o: &OpenOptions, p: impl AsRef<Path>) -> IoResult<File> {
            // No need to convert, we already added the flags in.
            Ok(File { v: o.open(p)?, p: 0u64 })
        }
    }

    #[inline]
    pub fn open_options() -> OpenOptions {
        let mut v = OpenOptions::new();
        // 0x40000000 - FILE_FLAG_OVERLAPPED
        v.custom_flags(0x40000000);
        v
    }
}
#[cfg(not(target_family = "windows"))]
mod inner {
    extern crate core;

    extern crate libc;
    extern crate xrmt_stx;

    use core::convert::AsRef;
    use core::result::Result::{Err, Ok};

    use libc::{fcntl, F_GETFL, F_SETFL, O_CLOEXEC, O_NONBLOCK};
    use xrmt_stx::fs::{self, OpenOptions};
    use xrmt_stx::io::{IoError, IoResult};
    use xrmt_stx::os::fd::{AsFd, AsRawFd};
    use xrmt_stx::path::Path;

    use crate::stxa::fs::File;

    impl File {
        #[inline]
        pub(super) fn new(f: fs::File) -> IoResult<File> {
            let h = f.as_fd().as_raw_fd();
            let v = unsafe { fcntl(h, F_GETFL) };
            if v == -1 {
                return Err(IoError::last_os_error());
            }
            if unsafe { fcntl(h, F_SETFL, v | O_NONBLOCK | O_CLOEXEC) } != 0 {
                return Err(IoError::last_os_error());
            }
            Ok(File { v: f })
        }
        #[inline]
        pub(super) fn open(o: &OpenOptions, p: impl AsRef<Path>) -> IoResult<File> {
            File::new(o.open(p)?)
        }
    }

    #[inline]
    pub fn open_options() -> OpenOptions {
        OpenOptions::new()
    }
}
