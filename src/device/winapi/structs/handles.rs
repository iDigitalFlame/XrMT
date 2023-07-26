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

use core::mem;
use core::ops::{Add, Deref};

use crate::device::winapi::{self, Win32Result};
use crate::util::stx::prelude::*;

pub const INVALID: Handle = Handle(0);

#[repr(transparent)]
pub struct Handle(pub usize);
#[repr(transparent)]
pub struct OwnedHandle(Handle);

pub trait AsHandle {
    fn as_handle(&self) -> Handle;
}

impl Handle {
    #[inline]
    pub fn take(v: OwnedHandle) -> Handle {
        let h = v.0;
        mem::forget(v); // Prevent double close
        h
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0 == 0 || self.0 as isize == -1
    }
    #[inline]
    pub fn close(self) -> Win32Result<()> {
        if self.is_invalid() {
            return Ok(());
        }
        winapi::CloseHandle(self)
    }
    #[inline]
    pub fn duplicate(&self) -> Win32Result<OwnedHandle> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        winapi::DuplicateHandle(*self, 0, 0x2)
    }
    #[inline]
    pub fn into_duplicate(self, inherit: bool, dst_proc: impl AsHandle) -> Win32Result<Handle> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        // 0x1 - DUPLICATE_CLOSE_SOURCE
        winapi::DuplicateHandleEx(self, winapi::CURRENT_PROCESS, dst_proc, 0, inherit, 0x3)
    }
}
impl OwnedHandle {
    #[inline]
    pub(crate) const fn empty() -> OwnedHandle {
        OwnedHandle(Handle(0))
    }

    #[inline]
    pub fn into_duplicate(self, inherit: bool, dst_proc: impl AsHandle) -> Win32Result<Handle> {
        // NOTE(dij): We duplicate the function here so we don't close the Handle before
        //            duplicating it.
        Handle::take(self).into_duplicate(inherit, dst_proc)
    }

    #[inline]
    pub(crate) fn set(&mut self, v: usize) {
        self.0 .0 = v
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
impl AsHandle for Handle {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self
    }
}
impl Default for Handle {
    #[inline]
    fn default() -> Handle {
        Handle(0)
    }
}
impl PartialEq for Handle {
    #[inline]
    fn eq(&self, other: &Handle) -> bool {
        self.0 == other.0
    }
}
impl Add<usize> for Handle {
    type Output = usize;

    #[inline]
    fn add(self, rhs: usize) -> usize {
        self.0 + rhs
    }
}

impl Eq for OwnedHandle {}
impl Drop for OwnedHandle {
    #[inline]
    fn drop(&mut self) {
        if self.0.is_invalid() {
            return;
        }
        winapi::close_handle(self.0)
    }
}
impl Deref for OwnedHandle {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        &self.0
    }
}
impl AsHandle for OwnedHandle {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0
    }
}
impl Default for OwnedHandle {
    #[inline]
    fn default() -> OwnedHandle {
        OwnedHandle::empty()
    }
}
impl PartialEq for OwnedHandle {
    #[inline]
    fn eq(&self, other: &OwnedHandle) -> bool {
        self.0 .0 == other.0 .0
    }
}
impl From<Handle> for OwnedHandle {
    #[inline]
    fn from(v: Handle) -> OwnedHandle {
        OwnedHandle(v)
    }
}

impl<T: AsHandle> AsHandle for &T {
    #[inline]
    fn as_handle(&self) -> Handle {
        (*self).as_handle()
    }
}

unsafe impl Send for Handle {}
unsafe impl Send for OwnedHandle {}

unsafe impl Sync for Handle {}
unsafe impl Sync for OwnedHandle {}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::{Handle, OwnedHandle};
    use crate::util::stx::prelude::*;

    impl Debug for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Handle: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }

    impl Debug for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "OwnedHandle: 0x{:X}", self.0 .0)
        }
    }
    impl LowerHex for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0 .0, f)
        }
    }
    impl UpperHex for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0 .0, f)
        }
    }
}
