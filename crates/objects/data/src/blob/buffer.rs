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

extern crate alloc;
extern crate core;

use alloc::alloc::Global;
use alloc::collections::{TryReserveError, TryReserveErrorKind};
use core::alloc::{Allocator, Layout};
use core::cmp::Ord;
use core::convert::From;
use core::hint::{assert_unchecked, unlikely};
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, SizedTypeProperties};
use core::ops::Drop;
use core::option::Option::{self, None, Some};
use core::ptr::{read, without_provenance_mut, NonNull, Unique};
use core::result::Result::{self, Err, Ok};
use core::slice::{from_raw_parts, from_raw_parts_mut};

use crate::blob::failure_alloc;

pub struct Buffer<T, A: Allocator = Global> {
    len:   usize,
    ptr:   Unique<u8>,
    alloc: A,
    _p:    PhantomData<T>,
}

pub type BasicBuffer<A> = Buffer<u8, A>;

impl<T> Buffer<T> {
    #[inline]
    pub const fn new() -> Buffer<T> {
        Buffer::new_in(Global)
    }

    #[inline]
    pub fn with_size(len: usize) -> Buffer<T> {
        Buffer::with_size_in(len, Global)
    }

    #[inline]
    pub fn into_raw_parts(self) -> (*mut T, usize) {
        let (p, n, _) = self.into_raw_parts_with_alloc();
        (p, n)
    }
}
impl<T, A: Allocator> Buffer<T, A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Buffer<T, A> {
        Buffer {
            alloc,
            len: 0,
            ptr: unsafe { Unique::new_unchecked(without_provenance_mut(T::LAYOUT.align())) },
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn with_size_in(len: usize, alloc: A) -> Buffer<T, A> {
        match Self::try_with_size_in(len, alloc) {
            Ok(v) => v,
            Err(_) => failure_alloc(),
        }
    }
    #[inline]
    pub fn try_with_size_in(len: usize, alloc: A) -> Result<Buffer<T, A>, TryReserveError> {
        let mut v = Buffer::new_in(alloc);
        if T::SIZE > 0 && len > 0 {
            let _ = v.allocate(len)?;
            unsafe { assert_unchecked(v.is_space_free(len)) };
        }
        Ok(v)
    }

    #[inline]
    pub unsafe fn from_raw_parts_in(ptr: *mut T, len: usize, alloc: A) -> Buffer<T, A> {
        Buffer {
            ptr: unsafe { Unique::new_unchecked(ptr as *mut u8) },
            len,
            alloc,
            _p: PhantomData,
        }
    }

    #[inline]
    pub const fn size(&self) -> usize {
        if T::IS_ZST {
            usize::MAX
        } else {
            self.len
        }
    }
    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr() as *const T
    }
    #[inline]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr.as_ptr() as *mut T
    }

    #[inline]
    pub fn allocator(&self) -> &A {
        &self.alloc
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe { from_raw_parts(self.as_ptr(), self.len) }
    }
    #[inline]
    pub fn shrink(&mut self, len: usize) {
        if unlikely(self.set(len).is_err()) {
            failure_alloc()
        }
    }
    #[inline]
    pub fn resize(&mut self, len: usize) {
        if unlikely(self.try_resize(len).is_err()) {
            failure_alloc()
        }
    }
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe { from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }
    #[inline]
    pub fn resize_sliced(&mut self, len: usize, add: usize) {
        if unlikely(self.try_resize_sliced(len, add).is_err()) {
            failure_alloc()
        }
    }
    #[inline]
    pub fn into_raw_parts_with_alloc(self) -> (*mut T, usize, A) {
        let v = ManuallyDrop::new(self);
        unsafe { (v.ptr.as_ptr() as *mut T, v.len, read(&v.alloc)) }
    }
    #[inline]
    pub fn try_resize(&mut self, len: usize) -> Result<(), TryReserveError> {
        if T::IS_ZST {
            return Ok(());
        }
        if !self.is_space_free(len) {
            let _ = self.grow(len)?;
        }
        unsafe { assert_unchecked(self.is_space_free(len)) };
        Ok(())
    }
    #[inline]
    pub fn try_shrink(&mut self, len: usize) -> Result<(), TryReserveError> {
        self.set(len)
    }
    #[inline]
    pub fn try_resize_sliced(&mut self, len: usize, add: usize) -> Result<(), TryReserveError> {
        if T::IS_ZST {
            return Ok(());
        }
        let n = len.checked_add(add).ok_or_else(overflow)?;
        if !self.is_space_free(n) {
            let r = n.max(self.len * 2);
            let _ = self.grow(cap(T::SIZE).max(r))?;
        }
        unsafe { assert_unchecked(self.is_space_free(n)) };
        Ok(())
    }

    #[inline]
    pub unsafe fn as_ptr_of<U>(&self) -> *const U {
        self.as_ptr() as *const U
    }
    #[inline]
    pub unsafe fn as_mut_ptr_of<U>(&mut self) -> *mut U {
        self.as_mut_ptr() as *mut U
    }

    #[inline]
    fn deallocate(&mut self) {
        let (p, l) = match self.current() {
            Some(v) => v,
            None => return,
        };
        unsafe { self.alloc.deallocate(p, l) };
    }
    #[inline]
    fn is_space_free(&self, len: usize) -> bool {
        self.size().checked_sub(len).is_some()
    }
    #[inline]
    fn current(&self) -> Option<(NonNull<u8>, Layout)> {
        if T::IS_ZST || self.len == 0 {
            None
        } else {
            let l = unsafe { Layout::from_size_align_unchecked(T::SIZE.unchecked_mul(self.len), Self::LAYOUT.align()) };
            Some((self.ptr.as_non_null_ptr(), l))
        }
    }
    fn set(&mut self, len: usize) -> Result<(), TryReserveError> {
        let (p, l) = match self.current() {
            Some(v) => v,
            None => return Ok(()),
        };
        if len == 0 {
            unsafe {
                self.alloc.deallocate(p, l);
            }
            self.ptr = unsafe { Unique::new_unchecked(without_provenance_mut(l.align())) };
            self.len = 0;
        } else {
            self.ptr = Unique::from(unsafe {
                let v = Layout::from_size_align_unchecked(T::SIZE.unchecked_mul(len), T::LAYOUT.align());
                self.alloc.shrink(p, l, v).map_err(|_| allocate_error(v))?.cast()
            });
            self.len = len;
        }
        Ok(())
    }
    #[cold]
    fn grow(&mut self, len: usize) -> Result<(), TryReserveError> {
        let (p, l) = match self.current() {
            Some(v) => v,
            None => return self.allocate(len),
        };
        let (n, _) = Self::LAYOUT.repeat(len).map_err(|_| overflow())?;
        self.ptr = Unique::from(unsafe {
            assert_unchecked(n.align() == l.align());
            self.alloc.grow(p, l, n).map_err(|_| allocate_error(n))?.cast()
        });
        self.len = len;
        Ok(())
    }
    #[inline]
    fn allocate(&mut self, len: usize) -> Result<(), TryReserveError> {
        let (l, _) = Self::LAYOUT.repeat(len).map_err(|_| overflow())?;
        self.ptr = Unique::from(self.alloc.allocate(l).map_err(|_| allocate_error(l))?.cast());
        self.len = len;
        Ok(())
    }
}

impl<T, A: Allocator> Drop for Buffer<T, A> {
    #[inline]
    fn drop(&mut self) {
        self.deallocate();
    }
}

#[inline]
fn cap(v: usize) -> usize {
    match v {
        1 => 8,
        0..=0x400 => 4,
        _ => 1,
    }
}
#[cold]
fn overflow() -> TryReserveError {
    TryReserveError::from(TryReserveErrorKind::CapacityOverflow)
}
#[cold]
fn allocate_error(l: Layout) -> TryReserveError {
    TryReserveError::from(TryReserveErrorKind::AllocError {
        layout:         l,
        non_exhaustive: (),
    })
}
