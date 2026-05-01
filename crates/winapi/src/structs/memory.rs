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

use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, Into};
use core::default::Default;
use core::marker::Copy;
use core::mem::{forget, size_of};
use core::ops::{Deref, DerefMut, Drop};
use core::result::Result::Ok;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use crate::functions::{NtFreeVirtualMemory, NtQueryVirtualMemory, NtReadVirtualMemory, NtWriteVirtualMemory};
use crate::structs::{CharPtr, Handle, WCharPtr};
use crate::{Win32Result, CURRENT_PROCESS};

pub enum ReadInto<'a, T> {
    /// Into the memory at this address.
    Pointer(*mut T),
    /// Directly into this Object
    Direct(&'a mut T),
}
pub enum WriteFrom<'a, T> {
    Null,
    /// Directly from this Object
    Direct(&'a T),
    /// From the memory at this address.
    Pointer(*const T),
}

#[repr(C)]
pub struct MemoryBasicInfo {
    pub base_address:    Region,
    pub base_allocation: Region,
    pub base_protection: u32,
    pub partition:       u16,
    pub size:            usize,
    pub state:           u32,
    pub protection:      u32,
    pub page_type:       u32,
}
#[repr(transparent)]
pub struct Region(pub usize);
pub struct OwnedRegion(Region);

impl Region {
    pub const EMPTY: Region = Region::empty();

    #[inline]
    pub const fn empty() -> Region {
        Region(0usize)
    }

    #[inline]
    pub fn into<T>(v: *const T) -> Region {
        v.into()
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
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
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }
    #[inline]
    pub fn as_ptr_of<T>(&self) -> *const T {
        self.0 as *const T
    }
    #[inline]
    pub fn as_mut_ptr_of<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    #[inline]
    pub fn free(self, proc: impl AsRef<Handle>) -> Win32Result<()> {
        if self.is_invalid() {
            return Ok(());
        }
        // 0x8000 - MEM_RELEASE
        NtFreeVirtualMemory(proc, self, 0, 0x8000)
    }
    #[inline]
    pub fn query(&self, proc: impl AsRef<Handle>) -> Win32Result<MemoryBasicInfo> {
        let mut i = MemoryBasicInfo::default();
        // 0x0 - MemoryBasicInfo
        NtQueryVirtualMemory(proc, self, 0, &mut i, size_of::<MemoryBasicInfo>() as u32)?;
        Ok(i)
    }
    #[inline]
    pub fn read<T>(&self, proc: impl AsRef<Handle>, to: ReadInto<T>) -> Win32Result<usize> {
        NtReadVirtualMemory(proc, Region(self.0), size_of::<T>(), to)
    }
    #[inline]
    pub fn write<T>(&self, proc: impl AsRef<Handle>, from: WriteFrom<T>) -> Win32Result<usize> {
        NtWriteVirtualMemory(proc, Region(self.0), size_of::<T>(), from)
    }

    #[inline]
    pub unsafe fn take(v: OwnedRegion) -> Region {
        let r = v.0;
        forget(v);
        r
    }

    #[inline]
    pub unsafe fn cast<T>(&self) -> &T {
        unsafe { &*(self.0 as *const T) as &T }
    }
    #[inline]
    pub unsafe fn cast_mut<T>(&self) -> &mut T {
        unsafe { &mut *(self.0 as *mut T) as &mut T }
    }
    /// OwnedRegions are Regions with drop glue to free them when dropped.
    ///
    /// Use 'Region::take' to remove the drop glue.
    ///
    /// This is unsafe as the caller ASSUMES this is in the CURRENT_PROCESS.
    #[inline]
    pub unsafe fn to_owned(self) -> OwnedRegion {
        OwnedRegion(self)
    }
    #[inline]
    pub unsafe fn cast_into<'a, T>(self) -> &'a T {
        unsafe { &*(self.0 as *const T) as &T }
    }
    #[inline]
    pub unsafe fn as_slice(&self, size: usize) -> &[u8] {
        unsafe { from_raw_parts(self.0 as *const u8, size) }
    }
    #[inline]
    pub unsafe fn cast_into_mut<'a, T>(self) -> &'a mut T {
        unsafe { &mut *(self.0 as *mut T) as &mut T }
    }
    #[inline]
    pub unsafe fn as_slice_mut(&self, size: usize) -> &mut [u8] {
        unsafe { from_raw_parts_mut(self.0 as *mut u8, size) }
    }
    #[inline]
    pub unsafe fn as_slice_at(&self, pos: usize, size: usize) -> &[u8] {
        unsafe { from_raw_parts((self.0 as *const u8).add(pos), size) }
    }
    #[inline]
    pub unsafe fn as_slice_at_mut(&self, pos: usize, size: usize) -> &mut [u8] {
        unsafe { from_raw_parts_mut((self.0 as *mut u8).add(pos), size) }
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

impl Drop for OwnedRegion {
    #[inline]
    fn drop(&mut self) {
        let _ = self.0.free(CURRENT_PROCESS);
    }
}
impl Deref for OwnedRegion {
    type Target = Region;

    #[inline]
    fn deref(&self) -> &Region {
        &self.0
    }
}
impl DerefMut for OwnedRegion {
    #[inline]
    fn deref_mut(&mut self) -> &mut Region {
        &mut self.0
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
        Region(*self as usize)
    }
}
impl Into<Region> for &Region {
    #[inline]
    fn into(self) -> Region {
        *self
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
impl Into<Region> for CharPtr<'_> {
    #[inline]
    fn into(self) -> Region {
        Region(self.as_ptr() as usize)
    }
}
impl Into<Region> for WCharPtr<'_> {
    #[inline]
    fn into(self) -> Region {
        Region(self.as_ptr() as usize)
    }
}

impl Default for MemoryBasicInfo {
    #[inline]
    fn default() -> MemoryBasicInfo {
        MemoryBasicInfo {
            size:            0usize,
            state:           0u32,
            page_type:       0u32,
            partition:       0u16,
            protection:      0u32,
            base_address:    Region::empty(),
            base_allocation: Region::empty(),
            base_protection: 0u32,
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::structs::Region;

    impl Debug for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "Region: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
