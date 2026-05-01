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

//! OS-specific functionality.

#![no_implicit_prelude]

pub use self::handle::*;
pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "."]
mod inner {
    #[path = "os"]
    pub mod windows {
        pub mod ffi;
        pub mod fs;
        pub mod io;
        pub mod process;

        pub mod raw {
            extern crate xrmt_winapi;

            use xrmt_winapi::structs::Handle;

            pub type HANDLE = Handle;
            pub type SOCKET = Handle;
        }
        pub mod thread {}
        pub mod prelude {
            pub use crate::os::windows::ffi::{OsStrExt, OsStringExt};
            pub use crate::os::windows::fs::{FileExt, MetadataExt, OpenOptionsExt};
            pub use crate::os::windows::io::{AsHandle, AsRawHandle, AsRawSocket, AsSocket, BorrowedHandle, BorrowedSocket, FromRawHandle, FromRawSocket, HandleOrInvalid, IntoRawHandle, IntoRawSocket, OwnedHandle, OwnedSocket, RawHandle, RawSocket};
        }
    }
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;
    pub use std::os::*;
}

#[cfg(target_family = "windows")]
mod handle {
    extern crate xrmt_winapi;

    pub type Handle = xrmt_winapi::structs::Handle;
}
#[cfg(not(target_family = "windows"))]
mod handle {
    extern crate core;

    extern crate libc;

    use core::clone::Clone;
    use core::cmp::{Eq, PartialEq};
    use core::convert::{AsRef, From};
    use core::default::Default;
    use core::fmt::{Debug, Formatter, Result};
    use core::marker::Copy;
    use core::mem::{forget, replace, transmute};
    use core::ops::{Deref, DerefMut, Drop};

    use libc::close;

    use crate::os::fd::{OwnedFd, RawFd};

    #[repr(transparent)]
    pub struct Handle(i32);
    #[repr(transparent)]
    pub struct OwnedHandle(Handle);

    /// Helper trait to convert `std` handles into the crate version of Handles.
    pub trait AsFdRef {
        fn as_ref(&self) -> &Handle;
    }

    impl Handle {
        pub const EMPTY: Handle = Handle::new(0i32);

        #[inline]
        pub const fn new(v: i32) -> Handle {
            Handle(v)
        }

        #[inline]
        pub fn set(&mut self, v: i32) {
            self.0 = v
        }
        #[inline]
        pub fn is_invalid(&self) -> bool {
            self.0 == 0 || self.0 == -1
        }

        #[inline]
        pub unsafe fn take(v: OwnedHandle) -> Handle {
            let h = v.0;
            forget(v); // Prevent double close
            h
        }
    }
    impl OwnedHandle {
        #[inline]
        pub const fn new(v: i32) -> OwnedHandle {
            OwnedHandle(Handle(v))
        }

        #[inline]
        pub unsafe fn take(v: &mut OwnedHandle) -> Handle {
            replace(&mut v.0, Handle::EMPTY)
        }
    }

    impl Eq for Handle {}
    impl Copy for Handle {}
    impl Clone for Handle {
        #[inline]
        fn clone(&self) -> Handle {
            Handle(self.0)
        }
    }
    impl Deref for Handle {
        type Target = i32;

        #[inline]
        fn deref(&self) -> &i32 {
            &self.0
        }
    }
    impl Debug for Handle {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl Default for Handle {
        #[inline]
        fn default() -> Handle {
            Handle(0i32)
        }
    }
    impl DerefMut for Handle {
        #[inline]
        fn deref_mut(&mut self) -> &mut i32 {
            &mut self.0
        }
    }
    impl From<i32> for Handle {
        #[inline]
        fn from(v: i32) -> Handle {
            Handle(v)
        }
    }
    impl From<u32> for Handle {
        #[inline]
        fn from(v: u32) -> Handle {
            Handle(v as i32)
        }
    }
    impl PartialEq for Handle {
        #[inline]
        fn eq(&self, other: &Handle) -> bool {
            self.0.eq(&other.0)
        }
    }
    impl From<Handle> for RawFd {
        #[inline]
        fn from(v: Handle) -> RawFd {
            unsafe { transmute(v) }
        }
    }
    impl From<Handle> for OwnedFd {
        #[inline]
        fn from(v: Handle) -> OwnedFd {
            unsafe { transmute(v) }
        }
    }
    impl AsRef<Handle> for Handle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            self
        }
    }

    impl Drop for OwnedHandle {
        #[inline]
        fn drop(&mut self) {
            if self.0 .0 != 0 {
                let _ = unsafe { close(self.0 .0) };
            }
        }
    }
    impl Deref for OwnedHandle {
        type Target = Handle;

        #[inline]
        fn deref(&self) -> &Handle {
            &self.0
        }
    }
    impl From<OwnedHandle> for RawFd {
        #[inline]
        fn from(v: OwnedHandle) -> RawFd {
            unsafe { transmute(v) }
        }
    }
    impl From<Handle> for OwnedHandle {
        #[inline]
        fn from(v: Handle) -> OwnedHandle {
            OwnedHandle(v)
        }
    }
    impl From<OwnedHandle> for OwnedFd {
        #[inline]
        fn from(v: OwnedHandle) -> OwnedFd {
            unsafe { transmute(v) }
        }
    }
    impl AsRef<Handle> for OwnedHandle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }

    impl<T: super::fd::AsFd> AsFdRef for T {
        #[inline]
        fn as_ref(&self) -> &Handle {
            unsafe { transmute(self) }
        }
    }
    impl<'a, T: super::fd::AsFd> From<&'a T> for &'a Handle {
        #[inline]
        fn from(v: &'a T) -> &'a Handle {
            unsafe { transmute(v) }
        }
    }
}
