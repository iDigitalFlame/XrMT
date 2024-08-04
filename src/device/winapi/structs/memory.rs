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

use core::intrinsics::size_of;
use core::ops::Deref;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use crate::device::winapi::{self, AsHandle, CharPtr, Handle, WCharPtr, Win32Result};
use crate::prelude::*;

pub enum ReadInto<'a, T> {
    Pointer(*mut T),
    Direct(&'a mut T),
}
pub enum WriteFrom<'a, T> {
    Null,
    Direct(&'a T),
    Pointer(*const T),
}

#[repr(transparent)]
pub struct Region(pub usize);

impl Region {
    #[inline]
    pub const fn empty() -> Region {
        Region(0)
    }

    #[inline]
    pub fn into<T>(v: *const T) -> Region {
        v.into()
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.0 as *const u8
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0 as *mut u8
    }
    #[inline]
    pub fn free(self, proc: impl AsHandle) -> Win32Result<()> {
        if self.is_invalid() {
            return Ok(());
        }
        // 0x8000 - MEM_RELEASE
        winapi::NtFreeVirtualMemory(proc, self, 0, 0x8000)
    }
    #[inline]
    pub fn read<T>(&self, proc: impl AsHandle, to: ReadInto<T>) -> Win32Result<usize> {
        winapi::NtReadVirtualMemory(proc, Region(self.0), size_of::<T>(), to)
    }
    #[inline]
    pub fn write<T>(&self, proc: impl AsHandle, from: WriteFrom<T>) -> Win32Result<usize> {
        winapi::NtWriteVirtualMemory(proc, Region(self.0), size_of::<T>(), from)
    }

    #[inline]
    pub unsafe fn cast<T>(&self) -> &T {
        &*(self.0 as *const T) as &T
    }
    #[inline]
    pub unsafe fn as_slice(&self, size: usize) -> &[u8] {
        from_raw_parts(self.0 as *const u8, size)
    }
    #[inline]
    pub unsafe fn as_slice_mut(&self, size: usize) -> &mut [u8] {
        from_raw_parts_mut(self.0 as *mut u8, size)
    }
}

impl Eq for Region {}
impl Copy for Region {}
impl Clone for Region {
    #[inline]
    fn clone(&self) -> Region {
        Region(self.0)
    }
}
impl Deref for Region {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        &self.0
    }
}
impl Default for Region {
    #[inline]
    fn default() -> Region {
        Region(0usize)
    }
}
impl PartialEq for Region {
    #[inline]
    fn eq(&self, other: &Region) -> bool {
        self.0 == other.0
    }
}

impl Into<Region> for usize {
    #[inline]
    fn into(self) -> Region {
        Region(self)
    }
}
impl Into<Region> for Handle {
    #[inline]
    fn into(self) -> Region {
        Region(self.0 as usize)
    }
}
impl Into<Region> for CharPtr {
    #[inline]
    fn into(self) -> Region {
        Region(self.as_ptr() as usize)
    }
}
impl Into<Region> for WCharPtr {
    #[inline]
    fn into(self) -> Region {
        Region(self.as_ptr() as usize)
    }
}
impl<T> Into<Region> for *mut T {
    #[inline]
    fn into(self) -> Region {
        Region(self as usize)
    }
}
impl<T> Into<Region> for *const T {
    #[inline]
    fn into(self) -> Region {
        Region(self as usize)
    }
}

#[inline]
pub fn read_memory<P, T>(proc: impl AsHandle, ptr: *mut P, to: ReadInto<T>) -> Win32Result<usize> {
    Region::into(ptr).read(proc, to)
}
#[inline]
pub fn write_memory<P, T>(proc: impl AsHandle, ptr: *const P, from: WriteFrom<T>) -> Win32Result<usize> {
    Region::into(ptr).write(proc, from)
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use crate::device::winapi::Region;
    use crate::prelude::*;

    impl Debug for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "MemoryRegion: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
