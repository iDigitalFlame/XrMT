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

use core::borrow::Borrow;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From};
use core::default::Default;
use core::marker::{Copy, Send, Sync};
use core::mem::{forget, transmute};
use core::num::NonZeroUsize;
use core::ops::{Add, Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::result::Result::Ok;

use crate::functions::{close_handle, CloseHandle, DuplicateHandle, DuplicateHandleEx};
use crate::{Win32Result, CURRENT_PROCESS};

#[repr(transparent)]
pub struct Handle(usize);
#[repr(transparent)]
pub struct OwnedHandle(NonZeroHandle);
#[repr(transparent)]
pub struct NonZeroHandle(NonZeroUsize);

/// Helper trait to convert `std` handles into the crate version of Handles.
pub trait IntoOwnedHandle {
    fn into_owned_handle(self) -> OwnedHandle;
}

impl Handle {
    pub const EMPTY: Handle = Handle::new(0usize);
    pub const INVALID: Handle = Handle::new(-1isize as usize);

    #[inline]
    pub const fn new(i: usize) -> Handle {
        Handle(i as usize)
    }

    // Sets the Handle value to '0'
    #[inline]
    pub fn zero(&mut self) {
        self.0 = Handle::EMPTY.0;
    }
    /// Sets the Handle value to '-1'
    #[inline]
    pub fn invalidate(&mut self) {
        self.0 = Handle::INVALID.0;
    }
    #[inline]
    pub fn as_ref(&self) -> &usize {
        &self.0
    }
    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
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
        CloseHandle(self)
    }
    #[inline]
    pub fn duplicate(&self) -> Win32Result<OwnedHandle> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        DuplicateHandle(self, 0, 0x2)
    }
    #[inline]
    pub fn into_duplicate(self, inherit: bool, dst: impl AsRef<Handle>) -> Win32Result<Handle> {
        // 0x2 - DUPLICATE_SAME_ACCESS
        // 0x1 - DUPLICATE_CLOSE_SOURCE
        DuplicateHandleEx(self, CURRENT_PROCESS, dst, 0, inherit, 0x3)
    }

    #[inline]
    pub unsafe fn take(v: OwnedHandle) -> Handle {
        let h = v.as_usize();
        forget(v); // Prevent double close
        Handle(h)
    }
    /// Creates a "shallow" clone of the Handle.
    /// Both handles are the SAME, be sure to drop them carefully!!
    #[inline]
    pub unsafe fn shallow_clone(v: &OwnedHandle) -> OwnedHandle {
        OwnedHandle(v.0)
    }
}
impl OwnedHandle {
    #[inline]
    pub const unsafe fn empty() -> OwnedHandle {
        OwnedHandle(NonZeroHandle::invalid())
    }

    #[inline]
    pub fn as_ref(&self) -> &usize {
        unsafe { transmute(self) }
    }
    #[inline]
    pub fn as_usize(&self) -> usize {
        // NOTE(dij): If we have problems with handles being "empty" or
        //            "invalid" here we can check and swap for a "zero" value
        //            it might be too much for each try so we'll keep this in mind.
        self.0.as_usize()
    }
    #[inline]
    pub fn is_invalid(&self) -> bool {
        // Should cover all invalid values/psuedo Handles.
        self.0.is_invalid()
    }
    #[inline]
    pub fn close(self) -> Win32Result<()> {
        if self.is_invalid() {
            return Ok(());
        }
        // Don't double free
        CloseHandle(*self).map(|_| forget(self))
    }
    #[inline]
    pub fn into_duplicate(self, inherit: bool, dst: impl AsRef<Handle>) -> Win32Result<Handle> {
        // NOTE(dij): We take the Handle here so we don't close the Handle before
        //            duplicating it.
        unsafe { Handle::take(self) }.into_duplicate(inherit, dst)
    }

    /// Takes the OwnedHandle out of it's reference. This can be used to
    /// "take" a Handle when needed. This will replace the source Handle with
    /// an invalid value.
    ///
    /// Be careful to not "take" handles that may be in use.
    #[inline]
    pub unsafe fn take(&mut self) -> OwnedHandle {
        let v = OwnedHandle(self.0);
        self.0.invalidate();
        v
    }
}
impl NonZeroHandle {
    const EMPTY: usize = -32isize as usize;

    #[inline]
    pub(crate) const fn invalid() -> NonZeroHandle {
        NonZeroHandle(unsafe { NonZeroUsize::new_unchecked(NonZeroHandle::EMPTY) })
    }
    #[inline]
    pub(crate) const fn new(v: Handle) -> NonZeroHandle {
        NonZeroHandle(unsafe { NonZeroUsize::new_unchecked(if v.0 == 0 { NonZeroHandle::EMPTY } else { v.0 }) })
    }

    #[inline]
    pub fn get(&self) -> Handle {
        Handle(self.0.get())
    }
    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0.get()
    }
    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0.get() >= NonZeroHandle::EMPTY || self.0.get() == 0 || self.0.get() == *Handle::INVALID
    }

    #[inline]
    fn invalidate(&mut self) {
        self.0 = unsafe { NonZeroUsize::new_unchecked(NonZeroHandle::EMPTY) }
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
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        &self.0
    }
}
impl Default for Handle {
    #[inline]
    fn default() -> Handle {
        Handle::EMPTY
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
impl AsRef<Handle> for Handle {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self
    }
}
impl PartialEq<usize> for Handle {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.0.eq(other)
    }
}
impl PartialOrd<usize> for Handle {
    #[inline]
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        Some(self.0.cmp(other))
    }
}
impl From<NonZeroUsize> for Handle {
    #[inline]
    fn from(v: NonZeroUsize) -> Handle {
        Handle(v.get())
    }
}
impl From<&NonZeroUsize> for Handle {
    #[inline]
    fn from(v: &NonZeroUsize) -> Handle {
        Handle(v.get())
    }
}
impl From<NonZeroHandle> for Handle {
    #[inline]
    fn from(v: NonZeroHandle) -> Handle {
        v.get()
    }
}
impl From<&NonZeroHandle> for Handle {
    #[inline]
    fn from(v: &NonZeroHandle) -> Handle {
        v.get()
    }
}

impl Eq for OwnedHandle {}
impl Drop for OwnedHandle {
    #[inline]
    fn drop(&mut self) {
        if self.is_invalid() {
            return;
        }
        unsafe { close_handle(**self) }
    }
}
impl Deref for OwnedHandle {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        #[cfg(debug_assertions)]
        if self.is_invalid() {
            core::panic!("deref(): Called for invalid OwnedHandle!");
        }
        // Little bit of fuckery, since they are the same size. This should be
        // a zero-cost convert.
        unsafe { transmute(self) }
    }
}
impl DerefMut for OwnedHandle {
    #[inline]
    fn deref_mut(&mut self) -> &mut Handle {
        #[cfg(debug_assertions)]
        if self.is_invalid() {
            core::panic!("deref_mut(): Called for invalid OwnedHandle!");
        }
        unsafe { transmute(self) }
    }
}
impl PartialEq for OwnedHandle {
    #[inline]
    fn eq(&self, other: &OwnedHandle) -> bool {
        self.0.get().eq(&other.0.get())
    }
}
impl From<Handle> for OwnedHandle {
    #[inline]
    fn from(v: Handle) -> OwnedHandle {
        OwnedHandle(NonZeroHandle::new(v))
    }
}
impl AsRef<Handle> for OwnedHandle {
    #[inline]
    fn as_ref(&self) -> &Handle {
        #[cfg(debug_assertions)]
        if self.is_invalid() {
            core::panic!("as_ref(): Called for invalid OwnedHandle!");
        }
        &*self
    }
}
impl Borrow<Handle> for OwnedHandle {
    #[inline]
    fn borrow(&self) -> &Handle {
        #[cfg(debug_assertions)]
        if self.is_invalid() {
            core::panic!("borrow(): Called for invalid OwnedHandle!");
        }
        &*self
    }
}

impl Copy for NonZeroHandle {}
impl Clone for NonZeroHandle {
    #[inline]
    fn clone(&self) -> NonZeroHandle {
        NonZeroHandle(self.0)
    }
}
impl Deref for NonZeroHandle {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        #[cfg(debug_assertions)]
        if self.is_invalid() {
            core::panic!("deref(): Called for invalid NonZeroHandle!");
        }
        unsafe { transmute(self) }
    }
}
impl From<Handle> for NonZeroHandle {
    #[inline]
    fn from(v: Handle) -> NonZeroHandle {
        NonZeroHandle::new(v)
    }
}

impl AsRef<Handle> for Option<Handle> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        match self {
            Some(v) => v,
            None => &CURRENT_PROCESS,
        }
    }
}
impl AsRef<Handle> for Option<&Handle> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        match self {
            Some(v) => v,
            None => &CURRENT_PROCESS,
        }
    }
}

impl AsRef<Handle> for Option<OwnedHandle> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        match self {
            Some(v) => v,
            None => &CURRENT_PROCESS,
        }
    }
}
impl AsRef<Handle> for Option<&OwnedHandle> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        match self {
            Some(v) => v,
            None => &CURRENT_PROCESS,
        }
    }
}

impl AsRef<Handle> for &NonZeroUsize {
    #[inline]
    fn as_ref(&self) -> &Handle {
        unsafe { transmute(self) }
    }
}

impl<T: IntoOwnedHandle> From<T> for OwnedHandle {
    #[inline]
    fn from(v: T) -> OwnedHandle {
        v.into_owned_handle()
    }
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

unsafe impl Send for NonZeroHandle {}
unsafe impl Sync for NonZeroHandle {}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::structs::{Handle, NonZeroHandle, OwnedHandle};

    impl Debug for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "Handle: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Handle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0, f)
        }
    }

    impl Debug for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "OwnedHandle: 0x{:X}", self.0.get())
        }
    }
    impl LowerHex for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0.get(), f)
        }
    }
    impl UpperHex for OwnedHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0.get(), f)
        }
    }

    impl Debug for NonZeroHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for NonZeroHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "NonZeroHandle: 0x{:X}", self.0.get())
        }
    }
    impl LowerHex for NonZeroHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0.get(), f)
        }
    }
    impl UpperHex for NonZeroHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0.get(), f)
        }
    }
}

#[cfg(all(target_family = "windows", feature = "std"))]
mod convert {
    extern crate core;
    extern crate std;

    use core::convert::Into;
    use core::mem::transmute;
    use std::os::windows::io;

    use crate::structs::{IntoOwnedHandle, OwnedHandle};

    impl<T: Into<io::OwnedHandle>> IntoOwnedHandle for T {
        #[inline]
        fn into_owned_handle(self) -> OwnedHandle {
            unsafe { transmute(self.into()) }
        }
    }
}
