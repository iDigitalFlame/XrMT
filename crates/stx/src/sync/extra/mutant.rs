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

extern crate xrmt_winapi;

use core::convert::{AsRef, From};
use core::fmt::{Debug, Formatter, Result};
use core::marker::{Send, Sync};
use core::mem::{transmute, ManuallyDrop};
use core::ops::{Deref, Drop};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::Ok;
use core::time::Duration;

use xrmt_winapi::functions::{duration_option_to_micros, CreateMutex, OpenMutex, QueryMutex, ReleaseMutex, WaitForSingleObject};
use xrmt_winapi::structs::OwnedHandle;
use xrmt_winapi::Win32Result;

use crate::abort_unlikely;
use crate::os::Handle;
use crate::sync::extra::{LazyHandle, LazyValue};

#[repr(transparent)]
pub struct Mutant(OwnedHandle);
#[repr(transparent)]
pub struct MutantHandle(Handle);
pub struct MutantGuard<'a>(&'a Mutant);
#[repr(transparent)]
pub struct MutantConstant(LazyHandle<Mutant>);

impl Mutant {
    #[inline]
    pub fn new(locked: bool) -> Mutant {
        abort_unlikely!(Mutant::new_error(locked))
    }
    #[inline]
    pub fn new_error(locked: bool) -> Win32Result<Mutant> {
        Ok(Mutant(CreateMutex(None, false, locked, None)?))
    }
    #[inline]
    pub fn open(name: impl AsRef<str>) -> Win32Result<Mutant> {
        // 0x1F0003 - FULL_CONTROL
        Ok(Mutant(OpenMutex(0x1F0001, false, name.as_ref())?))
    }
    #[inline]
    pub fn new_with_name(locked: bool, name: impl AsRef<str>) -> Win32Result<Mutant> {
        Ok(Mutant(CreateMutex(None, false, locked, name.as_ref())?))
    }

    #[inline]
    pub fn unlock(&self) {
        let _ = ReleaseMutex(&self.0);
    }
    #[inline]
    pub fn is_locked(&self) -> bool {
        QueryMutex(&self.0).map_or(false, |v| v > 0)
    }
    #[inline]
    pub fn lock(&self, dur: Option<Duration>) -> bool {
        match unsafe { self.lock_raw(dur) } {
            Ok(0) => true,
            _ => false,
        }
    }
    #[inline]
    pub fn lock_guard(&self, dur: Option<Duration>) -> Option<MutantGuard<'_>> {
        match unsafe { self.lock_raw(dur) } {
            Ok(0) => Some(MutantGuard(self)),
            _ => None,
        }
    }

    #[inline]
    pub unsafe fn close(&mut self) {
        let _ = unsafe { self.0.take() }.close();
    }
    #[inline]
    pub unsafe fn lock_raw(&self, dur: Option<Duration>) -> Win32Result<u32> {
        WaitForSingleObject(&self.0, duration_option_to_micros(dur), false)
    }
    #[inline]
    pub unsafe fn lock_handle(&self, dur: Option<Duration>) -> Option<MutantHandle> {
        match unsafe { self.lock_raw(dur) } {
            Ok(0) => Some(MutantHandle(*self.0)),
            _ => None,
        }
    }
}
impl MutantConstant {
    #[inline]
    pub const fn new() -> MutantConstant {
        MutantConstant(LazyHandle::new())
    }

    #[inline]
    pub unsafe fn close(&mut self) {
        if self.0.is_ready() {
            unsafe { self.0.take().close() };
        }
    }
}

impl Debug for Mutant {
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
impl Deref for Mutant {
    type Target = OwnedHandle;

    #[inline]
    fn deref(&self) -> &OwnedHandle {
        &self.0
    }
}
impl UnwindSafe for Mutant {}
impl AsRef<Handle> for Mutant {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl RefUnwindSafe for Mutant {}

impl Drop for MutantHandle {
    #[inline]
    fn drop(&mut self) {
        let _ = ReleaseMutex(self.0);
    }
}
impl Drop for MutantGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.unlock();
    }
}

impl Debug for MutantConstant {
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
impl Deref for MutantConstant {
    type Target = Mutant;

    #[inline]
    fn deref(&self) -> &Mutant {
        self.0.get()
    }
}
impl UnwindSafe for MutantConstant {}
impl AsRef<Handle> for MutantConstant {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.0.get().as_ref()
    }
}
impl RefUnwindSafe for MutantConstant {}

impl LazyValue for Mutant {
    #[inline]
    fn lazy_new() -> isize {
        unsafe { transmute(ManuallyDrop::new(Mutant::new(false))) }
    }
}

impl From<Mutant> for OwnedHandle {
    #[inline]
    fn from(v: Mutant) -> OwnedHandle {
        v.0
    }
}

unsafe impl Send for Mutant {}
unsafe impl Sync for Mutant {}

unsafe impl Send for MutantConstant {}
unsafe impl Sync for MutantConstant {}
