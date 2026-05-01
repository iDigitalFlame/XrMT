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
#![cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "solaris"),
    not(target_vendor = "apple")
))]

extern crate core;

use core::marker::{Send, Sync};
use core::panic::{RefUnwindSafe, UnwindSafe};

pub use self::inner::Semaphore;

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_winapi;

    use core::convert::{AsRef, From};
    use core::option::Option::None;
    use core::result::Result::{Err, Ok};
    use core::time::Duration;

    use xrmt_winapi::functions::{duration_to_micros, CreateSemaphore, OpenSemaphore, QuerySemaphore, ReleaseSemaphore, WaitForSingleObject};
    use xrmt_winapi::structs::OwnedHandle;
    use xrmt_winapi::INFINITE;

    use crate::io::{ErrorKind, IoError, IoResult};
    use crate::os::Handle;

    pub struct Semaphore(OwnedHandle);

    impl Semaphore {
        #[inline]
        pub fn new() -> IoResult<Semaphore> {
            Semaphore::new_with_count(u32::MAX, 0)
        }
        #[inline]
        pub fn open(n: impl AsRef<str>) -> IoResult<Semaphore> {
            // 0x1F0003 - FULL_CONTROL
            Ok(Semaphore(OpenSemaphore(0x1F0003, false, n.as_ref())?))
        }
        #[inline]
        pub fn new_with_limit(limit: u32) -> IoResult<Semaphore> {
            Semaphore::new_with_count(limit, 0)
        }
        #[inline]
        pub fn new_with_count(limit: u32, count: u32) -> IoResult<Semaphore> {
            Ok(Semaphore(CreateSemaphore(None, false, count, limit, None)?))
        }
        #[inline]
        pub fn new_with_name(limit: u32, n: impl AsRef<str>) -> IoResult<Semaphore> {
            Semaphore::new_with_name_and_count(limit, 0, n)
        }
        #[inline]
        pub fn new_with_name_and_count(limit: u32, count: u32, n: impl AsRef<str>) -> IoResult<Semaphore> {
            Ok(Semaphore(CreateSemaphore(
                None,
                false,
                count,
                limit,
                n.as_ref(),
            )?))
        }

        #[inline]
        pub fn limit(&self) -> u32 {
            QuerySemaphore(self).map_or(0, |v| v.1)
        }
        #[inline]
        pub fn current(&self) -> u32 {
            QuerySemaphore(self).map_or(0, |v| v.0)
        }
        #[inline]
        pub fn stats(&self) -> (u32, u32) {
            QuerySemaphore(self).unwrap_or((0, 0))
        }
        #[inline]
        pub fn wait(&self) -> IoResult<()> {
            let _ = WaitForSingleObject(self, INFINITE, false)?;
            Ok(())
        }
        #[inline]
        pub fn release(&self) -> IoResult<u32> {
            Ok(ReleaseSemaphore(self, 1)?)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) -> IoResult<()> {
            match WaitForSingleObject(&self.0, duration_to_micros(d), false)? {
                0xC0 => Err(IoError::from(ErrorKind::Interrupted)), // STATUS_USER_APC
                0 => Ok(()),
                _ => Err(IoError::from(ErrorKind::TimedOut)),
            }
        }
    }

    impl AsRef<Handle> for Semaphore {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }
}
#[cfg(not(target_family = "windows"))]
mod inner {
    extern crate core;

    extern crate libc;

    use core::convert::AsRef;
    use core::result::Result::Ok;
    use core::time::Duration;

    use libc::write;

    use crate::io::IoResult;
    use crate::sync::extra::Event;

    pub struct Semaphore(Event);

    impl Semaphore {
        #[inline]
        pub fn new() -> IoResult<Semaphore> {
            Semaphore::new_with_count(u32::MAX, 0)
        }

        #[inline]
        pub fn new_with_limit(limit: u32) -> IoResult<Semaphore> {
            Semaphore::new_with_count(limit, 0)
        }
        #[inline]
        pub fn new_with_count(_limit: u32, count: u32) -> IoResult<Semaphore> {
            Ok(Semaphore(Event::new_semaphore(count)?))
        }

        #[inline]
        pub fn wait(&self) -> IoResult<()> {
            self.0.wait();
            Ok(())
        }
        #[inline]
        pub fn release(&self) -> IoResult<u32> {
            self.write(1u64);
            Ok(0)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) -> IoResult<()> {
            self.0.wait_for(d);
            Ok(())
        }

        #[inline]
        fn write(&self, n: u64) {
            let v = n.to_ne_bytes();
            let _ = unsafe { write(**self.0.as_ref(), v.as_ptr() as _, 8) };
        }
    }
}

impl UnwindSafe for Semaphore {}
impl RefUnwindSafe for Semaphore {}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}
