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

extern crate xrmt_io;

use alloc::alloc::Global;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::bstr::ByteString;
use alloc::collections::TryReserveError;
use alloc::ffi::CString;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::{IntoIter, Vec};
use core::alloc::Allocator;
use core::array;
use core::borrow::{Borrow, BorrowMut};
use core::bstr::ByteStr;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsMut, AsRef, From};
use core::default::Default;
use core::ffi::CStr;
use core::fmt::{Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::hint::unlikely;
use core::iter::{repeat_n, ExactSizeIterator, Extend, FromIterator, IntoIterator, Iterator};
use core::marker::{Copy, Sized};
use core::mem::{drop, transmute, ManuallyDrop, MaybeUninit, SizedTypeProperties};
use core::ops::{Deref, DerefMut, Drop, FnMut, FnOnce, Index, IndexMut};
use core::option::Option::{self, None, Some};
use core::ptr::{copy, copy_nonoverlapping, drop_in_place, read, write, write_bytes};
use core::result::Result::{self, Err, Ok};
use core::slice::{from_raw_parts, from_raw_parts_mut, Iter, SliceIndex};
use core::str::from_utf8;

use xrmt_io::{FmtResult, IoResult, Write};

use crate::blob::{failure_alloc, failure_oob, failure_too_large};
use crate::text::{str_to_utf16_vec, utf16_display, utf16_to_string, utf16_to_vec, utf8_to_lossy};
use crate::{Buffer, Slice, VecLike};

pub struct Blob<T, const N: usize = 128, A: Allocator = Global> {
    len:   usize,
    heap:  Buffer<T, A>, // We're on the stack if size == 0
    slice: [MaybeUninit<T>; N],
}

struct LenGuard<'a> {
    ptr: &'a mut usize,
    new: usize,
}

trait BlobExtend<T, I> {
    fn _extend(&mut self, i: I);
}

impl<'a> LenGuard<'a> {
    #[inline]
    fn new(v: &'a mut usize) -> LenGuard<'a> {
        LenGuard { new: *v, ptr: v }
    }

    #[inline]
    fn add(&mut self) {
        self.new += 1;
    }
}
impl<T, const N: usize> Blob<T, N> {
    #[inline]
    pub const fn new() -> Blob<T, N> {
        Blob::new_in(Global)
    }

    #[inline]
    pub fn with_capacity(cap: usize) -> Blob<T, N> {
        Blob::with_capacity_in(cap, Global)
    }
    #[inline]
    pub fn try_with_capacity(cap: usize) -> Result<Blob<T, N>, TryReserveError> {
        Blob::try_with_capacity_in(cap, Global)
    }

    #[inline]
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize, cap: usize) -> Blob<T, N> {
        unsafe { Blob::from_raw_parts_in(ptr, len, cap, Global) }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (*mut T, usize, usize) {
        let (p, n, c, _) = self.into_raw_parts_with_alloc();
        (p, n, c)
    }
}
impl<T: Clone, const N: usize> Blob<T, N> {
    #[inline]
    pub fn with_values(v: &[T]) -> Blob<T, N> {
        Blob::with_values_in(v, Global)
    }
}
impl<T, const N: usize, A: Allocator> Blob<T, N, A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Blob<T, N, A> {
        Blob {
            len:   0usize,
            heap:  Buffer::new_in(alloc),
            slice: [const { MaybeUninit::uninit() }; N],
        }
    }

    #[inline]
    pub fn with_capacity_in(cap: usize, alloc: A) -> Blob<T, N, A> {
        match Blob::try_with_capacity_in(cap, alloc) {
            Ok(v) => v,
            Err(_) => failure_alloc(),
        }
    }
    #[inline]
    pub fn with_iter_in(i: impl Iterator<Item = T>, alloc: A) -> Blob<T, N, A> {
        let mut b = Blob::new_in(alloc);
        b._extend(i);
        b
    }
    #[inline]
    pub fn with_func_in(len: usize, f: impl FnMut() -> T, alloc: A) -> Blob<T, N, A> {
        let mut b = Blob::new_in(alloc);
        b.resize_func(len, f);
        b
    }
    #[inline]
    pub fn try_with_capacity_in(cap: usize, alloc: A) -> Result<Blob<T, N, A>, TryReserveError> {
        Ok(Blob {
            len:   0usize,
            heap:  if cap > N {
                Buffer::try_with_size_in(cap, alloc)?
            } else {
                Buffer::new_in(alloc)
            },
            slice: [const { MaybeUninit::uninit() }; N],
        })
    }

    #[inline]
    pub unsafe fn from_raw_parts_in(ptr: *mut T, len: usize, cap: usize, alloc: A) -> Blob<T, N, A> {
        Blob {
            len,
            heap: unsafe { Buffer::from_raw_parts_in(ptr, cap, alloc) },
            slice: [const { MaybeUninit::uninit() }; N],
        }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        unsafe { from_raw_parts(self.as_ptr(), self.len) }
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub const fn capacity(&self) -> usize {
        if self.is_on_heap() {
            self.heap.size()
        } else {
            N
        }
    }
    #[inline]
    pub const fn is_on_heap(&self) -> bool {
        self.heap.size() > 0
    }
    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        if self.is_on_heap() {
            self.heap.as_ptr()
        } else {
            self.slice.as_ptr() as *const T
        }
    }
    #[inline]
    pub const fn len_as_bytes(&self) -> usize {
        self.len * T::SIZE
    }
    #[inline]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        if self.is_on_heap() {
            self.heap.as_mut_ptr()
        } else {
            self.slice.as_mut_ptr() as *mut T
        }
    }
    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    #[inline]
    pub fn clear(&mut self) {
        unsafe { drop_in_place(self.as_mut_slice()) };
        self.len = 0;
    }
    #[inline]
    pub fn push(&mut self, v: T) {
        self.check(1); // Enforces BCE and capacity.
        unsafe { write(self.as_mut_ptr().add(self.len), v) };
        self.len += 1;
    }
    #[inline]
    pub fn allocator(&self) -> &A {
        self.heap.allocator()
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        if self.is_on_heap() && self.capacity() > self.len {
            self.heap.shrink(self.len);
        }
    }
    #[inline]
    pub fn into_vec(self) -> Vec<T, A> {
        match self.try_into_vec() {
            Ok(v) => v,
            Err(_) => failure_alloc(),
        }
    }
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let v = unsafe { read(self.as_ptr().add(self.len - 1)) };
        self.len -= 1;
        Some(v)
    }
    /// "Drains" the [`Blob`], which will discard all data before `pos` and any
    /// remaining data after `pos` will be at the start of the Blob.
    ///
    ///
    /// This is similar to Seeking forward a buffer.
    pub fn drain(&mut self, pos: usize) {
        if pos > self.len || pos == 0 {
            return;
        }
        let n = self.len.saturating_sub(pos);
        unsafe {
            // Drop the "before pos" data.
            drop_in_place(from_raw_parts_mut(self.as_mut_ptr(), pos));
            // Copy the rest to the start of the slice.
            copy(self.as_ptr().add(pos), self.as_mut_ptr(), n);
            write_bytes(self.as_mut_ptr().add(n), 0, n); // Clear the leftovers
        }
        self.len = n;
        self.shrink_to_fit();
    }
    #[inline]
    pub fn leak<'a>(self) -> &'a mut [T] {
        let mut v = ManuallyDrop::new(self);
        if !v.is_on_heap() {
            v.swap_heap();
        }
        unsafe { from_raw_parts_mut(v.as_mut_ptr(), v.len) }
    }
    #[inline]
    pub fn reserve(&mut self, add: usize) {
        if !self.is_on_heap() {
            if self.len.checked_add(add).is_some_and(|v| v < N) {
                return;
            }
            self.swap_heap();
        }
        self.heap.resize_sliced(self.len, add);
    }
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        if len > self.len {
            return;
        }
        unsafe {
            drop_in_place(from_raw_parts_mut(
                self.as_mut_ptr().add(len),
                self.len - len,
            ));
        }
        self.len = len;
    }
    #[inline]
    pub fn shrink_to(&mut self, cap: usize) {
        if self.is_on_heap() && self.capacity() > cap {
            self.heap.shrink(cap.max(self.len));
        }
    }
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        match self.try_remove(index) {
            Some(v) => v,
            None => failure_oob(index, self.len),
        }
    }
    #[inline]
    pub fn reserve_exact(&mut self, add: usize) {
        if unlikely(self.try_reserve_exact(add).is_err()) {
            failure_alloc()
        }
    }
    pub fn insert(&mut self, index: usize, v: T) {
        if index > self.len {
            failure_oob(index, self.len);
        }
        self.check(1);
        unsafe {
            let p = self.as_mut_ptr().add(index);
            if index < self.len {
                copy(p, p.add(1), self.len - index);
            }
            write(p, v);
        }
        self.len += 1;
    }
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> T {
        if index >= self.len {
            failure_oob(index, self.len);
        }
        unsafe {
            let v = read(self.as_ptr().add(index));
            copy(
                self.as_ptr().add(self.len - 1),
                self.as_mut_ptr().add(index),
                1,
            );
            self.len -= 1;
            v
        }
    }
    #[inline]
    pub fn try_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let v = unsafe {
            let p = self.as_mut_ptr().add(index);
            let v = read(p);
            copy(p.add(1), p, self.len - index - 1);
            v
        };
        self.len -= 1;
        Some(v)
    }
    #[inline]
    pub fn into_raw_parts_with_alloc(self) -> (*mut T, usize, usize, A) {
        let mut v = ManuallyDrop::new(self);
        if !v.is_on_heap() {
            v.swap_heap();
        }
        let (p, c) = (v.heap.as_mut_ptr(), v.heap.size());
        (p, v.len, c, unsafe { read(v.heap.allocator()) })
    }
    pub fn resize_func(&mut self, len: usize, mut f: impl FnMut() -> T) {
        if len <= self.len {
            self.truncate(len);
            return;
        }
        self.check(len); // Will resize and swap to heap if needed.
                         // After this we're fine to access the ptr.
        let n = len - self.len;
        unsafe {
            let p = self.as_mut_ptr().add(self.len);
            let mut g = LenGuard::new(&mut self.len);
            for i in 0..n {
                write(p.add(i), f());
                g.add();
            }
            drop(g);
        }
    }
    #[inline]
    pub fn try_into_vec(mut self) -> Result<Vec<T, A>, TryReserveError> {
        if !self.is_on_heap() {
            self.try_swap_heap()?;
        }
        let mut v = ManuallyDrop::new(self);
        unsafe {
            Ok(Vec::from_raw_parts_in(
                v.as_mut_ptr(),
                v.len,
                v.heap.size(),
                read(v.allocator()),
            ))
        }
    }
    #[inline]
    pub fn pop_if(&mut self, f: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let v = self.last_mut()?;
        if f(v) {
            self.pop()
        } else {
            None
        }
    }
    #[inline]
    pub fn spare_capacity_mut(&mut self) -> Option<&mut [MaybeUninit<T>]> {
        if !self.is_on_heap() {
            return None;
        }
        unsafe {
            Some(from_raw_parts_mut(
                self.heap.as_mut_ptr().add(self.len) as *mut MaybeUninit<T>,
                self.heap.size() - self.len,
            ))
        }
    }
    #[inline]
    pub fn try_reserve(&mut self, add: usize) -> Result<(), TryReserveError> {
        if !self.is_on_heap() {
            if self.len.checked_add(add).is_some_and(|v| v < N) {
                return Ok(());
            }
            self.try_swap_heap()?;
        }
        self.heap.try_resize_sliced(self.len, add)
    }
    #[inline]
    pub fn append<B: Allocator, const X: usize>(&mut self, v: &mut Blob<T, X, B>) {
        self.check(v.len);
        unsafe { copy(v.as_ptr(), self.as_mut_ptr().add(self.len), v.len) };
        self.len += v.len;
        v.len = 0; // Truncate length of the other, it shouldn't need to drop.
    }
    #[inline]
    pub fn try_reserve_exact(&mut self, add: usize) -> Result<(), TryReserveError> {
        if !self.is_on_heap() {
            if self.len.checked_add(add).is_some_and(|v| v < N) {
                return Ok(());
            }
            self.try_swap_heap()?;
        }
        self.heap.try_resize(self.len + add)
    }

    #[inline]
    pub unsafe fn as_slice_of<U>(&self) -> &[U] {
        unsafe { from_raw_parts(self.as_ptr() as *const U, (self.len * T::SIZE) / (U::SIZE)) }
    }
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        if (self.is_on_heap() && len > self.heap.size()) || (!self.is_on_heap() || len > N) {
            return;
        }
        self.len = len;
    }
    #[inline]
    pub unsafe fn write_ptr<U>(&mut self, v: &U) {
        let n = U::SIZE.div_ceil(T::SIZE); // Number of 'T' needed, rounded up.
        self.check(n);
        unsafe {
            let p = self.as_mut_ptr().add(self.len) as *mut u8;
            write_bytes(p, 0, n * T::SIZE); // Zero the section.
            copy_nonoverlapping((v as *const U) as *const u8, p, U::SIZE); // Copy over the data
        }
        self.len += n;
    }
    #[inline]
    pub unsafe fn as_ptr_of<U>(&self) -> *const U {
        self.as_ptr() as *const U
    }
    #[inline]
    pub unsafe fn write<U: Sized>(&mut self, v: U) {
        unsafe {
            let p = v;
            self.write_ptr(&p)
        }
    }
    #[inline]
    pub unsafe fn as_mut_ptr_of<U>(&mut self) -> *mut U {
        self.as_mut_ptr() as *mut U
    }
    #[inline]
    pub unsafe fn set_len_as_bytes(&mut self, len: usize) {
        unsafe { self.set_len(len / T::SIZE) }
    }
    #[inline]
    pub unsafe fn read<U: Sized>(&self, pos: usize) -> Option<U> {
        if pos >= self.len {
            return None;
        }
        let (n, r) = (U::SIZE.div_ceil(T::SIZE), self.len - pos);
        if n == 0 || r < n {
            return None;
        }
        unsafe { Some(read(self.as_ptr().add(pos) as *const U)) }
    }
    #[inline]
    pub unsafe fn read_into<U: Sized>(&self, pos: usize, dst: &mut U) -> bool {
        if pos >= self.len {
            return false;
        }
        let (n, r) = (U::SIZE.div_ceil(T::SIZE), self.len - pos);
        if n == 0 || r < n {
            return false;
        }
        unsafe { copy_nonoverlapping(self.as_ptr().add(pos) as *const U, dst, 1) };
        true
    }

    #[inline]
    fn swap_heap(&mut self) {
        if unlikely(self.try_swap_heap().is_err()) {
            failure_alloc();
        }
    }
    #[inline]
    fn add(&mut self, v: &[T]) {
        let n = v.len();
        self.check(n); // Will swap to heap if needed.
        unsafe { copy_nonoverlapping(v.as_ptr(), self.as_mut_ptr().add(self.len), n) };
        self.len += n;
    }
    #[inline]
    fn check(&mut self, len: usize) {
        let (n, e) = self.len.overflowing_add(len);
        if unlikely(e) {
            failure_too_large();
        }
        if n < N {
            return;
        }
        if !self.is_on_heap() {
            self.swap_heap();
        }
        self.heap.resize_sliced(self.len, len);
    }
    fn add_iter(&mut self, mut i: impl Iterator<Item = T>) {
        let (s, _) = i.size_hint();
        self.check(s);
        while let Some(v) = i.next() {
            if self.len == self.capacity() {
                let (s, _) = i.size_hint();
                self.check(s.saturating_add(1));
            }
            unsafe { write(self.as_mut_ptr().add(self.len), v) };
            self.len += 1;
        }
    }
    #[inline]
    fn try_swap_heap(&mut self) -> Result<(), TryReserveError> {
        let _ = self.heap.try_resize_sliced(self.len, 0)?;
        unsafe {
            // Copy Slice -> Heap
            copy_nonoverlapping(
                self.slice.as_ptr() as *mut T,
                self.heap.as_mut_ptr(),
                self.len,
            );
            // Zero Slice without triggering Drop
            write_bytes(self.slice.as_mut_ptr() as *mut T, 0, self.len);
            // This is safe as the slice won't be accessed again.
        }
        Ok(())
    }
    #[inline]
    fn add_exact(&mut self, s: usize, i: impl Iterator<Item = T>) {
        self.check(s);
        unsafe {
            let p = self.as_mut_ptr().add(self.len);
            for (n, v) in i.enumerate() {
                if unlikely(n > s) {
                    self.check(s); // Add more additional space
                }
                write(p.add(n), v);
                self.len += 1;
            }
        }
    }
}
impl<T: Clone + Default, const N: usize> Blob<T, N> {
    #[inline]
    pub fn with_size(size: usize) -> Blob<T, N> {
        Blob::with_size_in(size, Global)
    }
}
impl<T: Clone, const N: usize, A: Allocator> Blob<T, N, A> {
    #[inline]
    pub fn with_values_in(v: &[T], alloc: A) -> Blob<T, N, A> {
        let mut b = Blob::with_capacity_in(v.len(), alloc);
        b._extend(v.iter());
        b
    }

    #[inline]
    pub fn extend_from_slice(&mut self, v: &[T]) {
        self._extend(v.iter())
    }
    #[inline]
    pub fn resize_with(&mut self, len: usize, v: T) {
        if len <= self.len {
            self.truncate(len);
            return;
        }
        self._extend(repeat_n(v, len - self.len));
    }
}
impl<T, const N: usize, A: Allocator + Clone> Blob<T, N, A> {
    #[inline]
    pub fn allocator_clone(&self) -> A {
        self.heap.allocator().clone()
    }
}
impl<T: Clone + Default, const N: usize, A: Allocator> Blob<T, N, A> {
    #[inline]
    pub fn with_size_in(size: usize, alloc: A) -> Blob<T, N, A> {
        let mut b = Blob::with_capacity_in(size, alloc);
        b.resize(size);
        b
    }

    #[inline]
    pub fn resize(&mut self, len: usize) {
        self.resize_with(len, T::default())
    }
    #[inline]
    pub fn resize_as_bytes(&mut self, len: usize) {
        self.resize_with(len / T::SIZE, T::default())
    }
}

impl<const N: usize> Blob<u8, N> {
    #[inline]
    pub fn from_cstr(v: &[u8]) -> Blob<u8, N> {
        Blob::from_cstr_in(v, Global)
    }
    #[inline]
    pub fn from_utf16(v: &[u16]) -> Blob<u8, N> {
        Blob::from_utf16_in(v, Global)
    }
    #[inline]
    pub fn from_str(v: impl AsRef<str>) -> Blob<u8, N> {
        Blob::from_str_in(v, Global)
    }

    /// Convert UTF16 to UTF8 by truncating the values to UTF8 limits. No
    /// checking is made to ensure the values are correct.
    ///
    /// Recommended only for UTF16 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf16_unchecked(v: &[u16]) -> Blob<u8, N> {
        unsafe { Blob::from_utf16_unchecked_in(v, Global) }
    }
}
impl<const N: usize, A: Allocator> Blob<u8, N, A> {
    #[inline]
    pub fn from_cstr_in(v: &[u8], alloc: A) -> Blob<u8, N, A> {
        let mut b = Blob::with_values_in(v, alloc);
        if let Some(i) = b.iter().position(|i| *i == 0) {
            b.truncate(i);
        }
        b
    }
    #[inline]
    pub fn from_utf16_in(v: &[u16], alloc: A) -> Blob<u8, N, A> {
        let mut b = Blob::with_capacity_in(v.len(), alloc);
        let _ = utf16_to_vec(&mut b, v);
        b
    }
    #[inline]
    pub fn from_str_in(v: impl AsRef<str>, alloc: A) -> Blob<u8, N, A> {
        Blob::with_values_in(v.as_ref().as_bytes(), alloc)
    }

    /// Convert UTF16 to UTF8 by truncating the values to UTF8 limits. No
    /// checking is made to ensure the values are correct.
    ///
    /// Recommended only for UTF16 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf16_unchecked_in(v: &[u16], alloc: A) -> Blob<u8, N, A> {
        let n = match v.iter().position(|i| *i == 0) {
            Some(i) => i,
            None => v.len(),
        };
        let mut b = Blob::with_size_in(n, alloc);
        for i in 0..b.len {
            unsafe { *b.get_unchecked_mut(i) = *v.get_unchecked(i) as u8 };
        }
        b
    }

    #[inline]
    pub fn as_cstr(&self) -> &str {
        match self.iter().position(|v| *v == 0) {
            // BCE Checked above
            Some(i) => unsafe { transmute(self.as_slice().get_unchecked(0..i)) },
            None => unsafe { transmute(self.as_slice()) },
        }
    }
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        from_utf8(self.as_slice()).ok()
    }
    #[inline]
    pub fn as_str_lossy(&self) -> Cow<'_, str> {
        utf8_to_lossy(self.as_slice())
    }

    #[inline]
    pub unsafe fn as_str_unchecked(&self) -> &str {
        unsafe { transmute(self.as_slice()) }
    }
}

impl<const N: usize> Blob<u16, N> {
    #[inline]
    pub fn from_str_utf16(v: impl AsRef<str>) -> Blob<u16, N> {
        Blob::from_str_utf16_in(v, Global)
    }

    /// Convert UTF8 to UTF16 by extending the values to UTF16 widths while
    /// keeping the higher bytes zero. No checking is made to ensure the
    /// values are correct.
    ///
    /// Recommended only for UTF8 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf8_unchecked(v: &[u8]) -> Blob<u16, N> {
        unsafe { Blob::from_utf8_unchecked_in(v, Global) }
    }
}
impl<const N: usize, A: Allocator> Blob<u16, N, A> {
    #[inline]
    pub fn from_str_utf16_in(v: impl AsRef<str>, alloc: A) -> Blob<u16, N, A> {
        let s = v.as_ref();
        let mut b = Blob::with_capacity_in(s.len(), alloc);
        str_to_utf16_vec(&mut b, s);
        b
    }

    /// Convert UTF8 to UTF16 by extending the values to UTF16 widths while
    /// keeping the higher bytes zero. No checking is made to ensure the
    /// values are correct.
    ///
    /// Recommended only for UTF8 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf8_unchecked_in(v: &[u8], alloc: A) -> Blob<u16, N, A> {
        let n = match v.iter().position(|i| *i == 0) {
            Some(i) => i,
            None => v.len(),
        };
        let mut b = Blob::with_size_in(n, alloc);
        for i in 0..b.len {
            unsafe { *b.get_unchecked_mut(i) = *v.get_unchecked(i) as u16 };
        }
        b
    }

    #[inline]
    pub fn as_str(&self) -> String {
        utf16_to_string(self.as_slice())
    }
    #[inline]
    pub fn as_utf8<const X: usize>(&self) -> Slice<u8, X> {
        Slice::from_utf16(self)
    }
}
impl<const N: usize, A: Allocator + Clone> Blob<u16, N, A> {
    #[inline]
    pub fn as_utf8_blob<const X: usize>(&self) -> Blob<u8, X, A> {
        let mut b = Blob::new_in(self.allocator_clone());
        let _ = utf16_to_vec(&mut b, self.as_slice());
        b
    }
}

impl Drop for LenGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        *self.ptr = self.new
    }
}

impl<T, const N: usize> Default for Blob<T, N> {
    #[inline]
    fn default() -> Blob<T, N> {
        Blob::new()
    }
}
impl<T, const N: usize, A: Allocator> Drop for Blob<T, N, A> {
    #[inline]
    fn drop(&mut self) {
        self.clear();
    }
}
impl<T, const N: usize, A: Allocator> Deref for Blob<T, N, A> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, const N: usize, A: Allocator> DerefMut for Blob<T, N, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Hash, const N: usize, A: Allocator> Hash for Blob<T, N, A> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        Hash::hash(&**self, h)
    }
}
impl<T, const N: usize, A: Allocator> AsMut<[T]> for Blob<T, N, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T, const N: usize, A: Allocator> AsRef<[T]> for Blob<T, N, A> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, const N: usize, A: Allocator> Borrow<[T]> for Blob<T, N, A> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Debug, const N: usize, A: Allocator> Debug for Blob<T, N, A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self.as_slice(), f)
    }
}
impl<T, const N: usize, A: Allocator> BorrowMut<[T]> for Blob<T, N, A> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Copy, const N: usize, A: Allocator> VecLike<T> for Blob<T, N, A> {
    #[inline]
    fn push(&mut self, v: T) {
        self.push(v)
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }
    #[inline]
    fn shrink_to_fit(&mut self) {
        self.shrink_to_fit();
    }

    #[inline]
    fn reserve(&mut self, len: usize) {
        self.reserve(len)
    }

    #[inline]
    fn truncate(&mut self, len: usize) {
        self.truncate(len)
    }
    #[inline]
    fn resize(&mut self, len: usize, v: T) {
        self.resize_with(len, v);
    }
    #[inline]
    fn extend_from_slice(&mut self, other: &[T]) {
        self.extend_from_slice(other)
    }
}
impl<T, const N: usize, A: Allocator> AsMut<Blob<T, N, A>> for Blob<T, N, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut Blob<T, N, A> {
        self
    }
}
impl<T, const N: usize, A: Allocator> AsRef<Blob<T, N, A>> for Blob<T, N, A> {
    #[inline]
    fn as_ref(&self) -> &Blob<T, N, A> {
        self
    }
}
impl<T: Clone, const N: usize, A: Allocator + Clone> Clone for Blob<T, N, A> {
    #[inline]
    fn clone(&self) -> Blob<T, N, A> {
        let mut b = Blob::with_capacity_in(self.len, self.heap.allocator().clone());
        b.extend_from_slice(&self);
        b
    }
}
impl<'a, T, const N: usize, A: Allocator> IntoIterator for &'a Blob<T, N, A> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.as_slice().into_iter()
    }
}

impl<T: Eq, const N: usize, A: Allocator> Eq for Blob<T, N, A> {}
impl<T: PartialEq, const N: usize, A: Allocator> PartialEq for Blob<T, N, A> {
    #[inline]
    fn eq(&self, other: &Blob<T, N, A>) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl<T: PartialEq<U>, U, const N: usize, A: Allocator> PartialEq<[U]> for Blob<T, N, A> {
    #[inline]
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice().eq(other)
    }
}
impl<T: PartialEq<U>, U, const N: usize, A: Allocator> PartialEq<&[U]> for Blob<T, N, A> {
    #[inline]
    fn eq(&self, other: &&[U]) -> bool {
        self.as_slice().eq(*other)
    }
}
impl<T: PartialEq<U>, U, const N: usize, A: Allocator> PartialEq<&mut [U]> for Blob<T, N, A> {
    #[inline]
    fn eq(&self, other: &&mut [U]) -> bool {
        self.as_slice().eq(*other)
    }
}
impl<T: PartialEq<U>, U, const N: usize, const X: usize, A: Allocator> PartialEq<[U; X]> for Blob<T, N, A> {
    #[inline]
    fn eq(&self, other: &[U; X]) -> bool {
        self.as_slice().eq(other)
    }
}

impl<T: Ord, const N: usize, A: Allocator> Ord for Blob<T, N, A> {
    #[inline]
    fn cmp(&self, other: &Blob<T, N, A>) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl<T: PartialOrd, const N: usize, A: Allocator> PartialOrd for Blob<T, N, A> {
    fn partial_cmp(&self, other: &Blob<T, N, A>) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<const N: usize, A: Allocator> Write for Blob<u8, N, A> {
    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        false
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }
    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

impl<T, const N: usize> FromIterator<T> for Blob<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(i: I) -> Blob<T, N> {
        let mut b = Blob::new();
        b._extend(i.into_iter());
        b
    }
}
impl<'a, T: Clone, const N: usize> FromIterator<&'a T> for Blob<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a T>>(i: I) -> Blob<T, N> {
        let mut b = Blob::new();
        b._extend(i.into_iter());
        b
    }
}

impl<T, const N: usize, A: Allocator, I: SliceIndex<[T]>> Index<I> for Blob<T, N, A> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}
impl<T, const N: usize, A: Allocator, I: SliceIndex<[T]>> IndexMut<I> for Blob<T, N, A> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl<T, const N: usize, A: Allocator> Extend<T> for Blob<T, N, A> {
    #[inline]
    fn extend_one(&mut self, v: T) {
        self.push(v);
    }
    #[inline]
    fn extend_reserve(&mut self, len: usize) {
        self.check(len);
    }
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, i: I) {
        self._extend(i.into_iter())
    }
}
impl<'a, T: 'a + Copy, const N: usize, A: Allocator> Extend<&'a T> for Blob<T, N, A> {
    #[inline]
    fn extend_one(&mut self, v: &'a T) {
        self.push(*v);
    }
    #[inline]
    fn extend_reserve(&mut self, len: usize) {
        self.check(len);
    }
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, i: I) {
        self._extend(i.into_iter())
    }
}

impl<T: Clone, const N: usize> From<&[T]> for Blob<T, N> {
    #[inline]
    fn from(v: &[T]) -> Blob<T, N> {
        let mut b = Blob::with_capacity(v.len());
        b._extend(v.iter());
        b
    }
}
impl<T: Clone, const N: usize> From<&Vec<T>> for Blob<T, N> {
    #[inline]
    fn from(v: &Vec<T>) -> Blob<T, N> {
        Blob::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize> From<&mut [T]> for Blob<T, N> {
    #[inline]
    fn from(v: &mut [T]) -> Blob<T, N> {
        let mut b = Blob::with_capacity(v.len());
        b.add(v);
        b
    }
}
impl<T: Clone, const N: usize> From<&Rc<[T]>> for Blob<T, N> {
    #[inline]
    fn from(v: &Rc<[T]>) -> Blob<T, N> {
        Blob::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&Arc<[T]>> for Blob<T, N> {
    #[inline]
    fn from(v: &Arc<[T]>) -> Blob<T, N> {
        Blob::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&Box<[T]>> for Blob<T, N> {
    #[inline]
    fn from(v: &Box<[T]>) -> Blob<T, N> {
        Blob::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&mut Vec<T>> for Blob<T, N> {
    #[inline]
    fn from(v: &mut Vec<T>) -> Blob<T, N> {
        Blob::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize> From<Cow<'_, [T]>> for Blob<T, N> {
    #[inline]
    fn from(v: Cow<'_, [T]>) -> Blob<T, N> {
        Blob::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize, const X: usize> From<[T; X]> for Blob<T, N> {
    #[inline]
    fn from(v: [T; X]) -> Blob<T, N> {
        Blob::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize, const X: usize> From<Slice<T, X>> for Blob<T, N> {
    #[inline]
    fn from(v: Slice<T, X>) -> Blob<T, N> {
        Blob::from(v.as_slice())
    }
}

impl<T, const N: usize, A: Allocator> From<Vec<T, A>> for Blob<T, N, A> {
    #[inline]
    fn from(v: Vec<T, A>) -> Blob<T, N, A> {
        let (p, s, c, a) = v.into_raw_parts_with_alloc();
        unsafe { Blob::from_raw_parts_in(p, s, c, a) }
    }
}
impl<T: Clone, const N: usize, A: Allocator> From<Rc<[T], A>> for Blob<T, N, A> {
    #[inline]
    fn from(v: Rc<[T], A>) -> Blob<T, N, A> {
        let n = v.len();
        let (p, a) = Rc::into_raw_with_allocator(v);
        unsafe { Blob::from_raw_parts_in(p as *mut T, n, n, a) }
    }
}
impl<T: Clone, const N: usize, A: Allocator> From<Arc<[T], A>> for Blob<T, N, A> {
    #[inline]
    fn from(v: Arc<[T], A>) -> Blob<T, N, A> {
        let n = v.len();
        let (p, a) = Arc::into_raw_with_allocator(v);
        unsafe { Blob::from_raw_parts_in(p as *mut T, n, n, a) }
    }
}
impl<T: Clone, const N: usize, A: Allocator> From<Box<[T], A>> for Blob<T, N, A> {
    #[inline]
    fn from(v: Box<[T], A>) -> Blob<T, N, A> {
        let n = v.len();
        let (p, a) = Box::into_raw_with_allocator(v);
        unsafe { Blob::from_raw_parts_in(p as *mut T, n, n, a) }
    }
}

impl<T: Clone, const N: usize, A: Allocator + Clone> From<&Vec<T, A>> for Blob<T, N, A> {
    #[inline]
    default fn from(v: &Vec<T, A>) -> Blob<T, N, A> {
        Blob::with_values_in(v.as_slice(), v.allocator().clone())
    }
}
impl<T: Clone, const N: usize, A: Allocator + Clone> From<&Rc<[T], A>> for Blob<T, N, A> {
    #[inline]
    default fn from(v: &Rc<[T], A>) -> Blob<T, N, A> {
        Blob::with_values_in(v.as_ref(), Rc::allocator(v).clone())
    }
}
impl<T: Clone, const N: usize, A: Allocator + Clone> From<&Arc<[T], A>> for Blob<T, N, A> {
    #[inline]
    default fn from(v: &Arc<[T], A>) -> Blob<T, N, A> {
        Blob::with_values_in(v.as_ref(), Arc::allocator(v).clone())
    }
}
impl<T: Clone, const N: usize, A: Allocator + Clone> From<&Box<[T], A>> for Blob<T, N, A> {
    #[inline]
    default fn from(v: &Box<[T], A>) -> Blob<T, N, A> {
        Blob::with_values_in(v.as_ref(), Box::allocator(v).clone())
    }
}
impl<T: Clone, const N: usize, A: Allocator + Clone> From<&mut Vec<T, A>> for Blob<T, N, A> {
    #[inline]
    default fn from(v: &mut Vec<T, A>) -> Blob<T, N, A> {
        Blob::with_values_in(v.as_slice(), v.allocator().clone())
    }
}

impl<const N: usize> From<&str> for Blob<u8, N> {
    #[inline]
    fn from(v: &str) -> Blob<u8, N> {
        Blob::from(v.as_bytes())
    }
}
impl<const N: usize> From<&CStr> for Blob<u8, N> {
    #[inline]
    fn from(v: &CStr) -> Blob<u8, N> {
        Blob::from(v.to_bytes())
    }
}
impl<const N: usize> From<String> for Blob<u8, N> {
    #[inline]
    fn from(v: String) -> Blob<u8, N> {
        Blob::from(v.into_bytes())
    }
}
impl<const N: usize> From<&String> for Blob<u8, N> {
    #[inline]
    fn from(v: &String) -> Blob<u8, N> {
        Blob::from(v.as_bytes())
    }
}
impl<const N: usize> From<CString> for Blob<u8, N> {
    #[inline]
    fn from(v: CString) -> Blob<u8, N> {
        Blob::from(v.into_bytes())
    }
}
impl<const N: usize> From<&CString> for Blob<u8, N> {
    #[inline]
    fn from(v: &CString) -> Blob<u8, N> {
        Blob::from(v.as_bytes())
    }
}
impl<const N: usize> From<&ByteStr> for Blob<u8, N> {
    #[inline]
    fn from(v: &ByteStr) -> Blob<u8, N> {
        Blob::from(&v.0)
    }
}
impl<const N: usize> From<ByteString> for Blob<u8, N> {
    #[inline]
    fn from(v: ByteString) -> Blob<u8, N> {
        Blob::from(v.0)
    }
}
impl<const N: usize> From<&ByteString> for Blob<u8, N> {
    #[inline]
    fn from(v: &ByteString) -> Blob<u8, N> {
        Blob::from(v.as_slice())
    }
}
impl<const N: usize> From<Option<&str>> for Blob<u8, N> {
    #[inline]
    fn from(v: Option<&str>) -> Blob<u8, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}
impl<const N: usize> From<Cow<'_, &str>> for Blob<u8, N> {
    #[inline]
    fn from(v: Cow<'_, &str>) -> Blob<u8, N> {
        Blob::from(v.as_bytes())
    }
}
impl<const N: usize> From<Option<String>> for Blob<u8, N> {
    #[inline]
    fn from(v: Option<String>) -> Blob<u8, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}
impl<const N: usize> From<Option<&String>> for Blob<u8, N> {
    #[inline]
    fn from(v: Option<&String>) -> Blob<u8, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}

impl<const N: usize> From<&str> for Blob<u16, N> {
    #[inline]
    fn from(v: &str) -> Blob<u16, N> {
        Blob::from_str_utf16(v)
    }
}
impl<const N: usize> From<String> for Blob<u16, N> {
    #[inline]
    fn from(v: String) -> Blob<u16, N> {
        Blob::from(v.as_str())
    }
}
impl<const N: usize> From<&String> for Blob<u16, N> {
    #[inline]
    fn from(v: &String) -> Blob<u16, N> {
        Blob::from(v.as_str())
    }
}
impl<const N: usize> From<Option<&str>> for Blob<u16, N> {
    #[inline]
    fn from(v: Option<&str>) -> Blob<u16, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}
impl<const N: usize> From<Cow<'_, &str>> for Blob<u16, N> {
    #[inline]
    fn from(v: Cow<'_, &str>) -> Blob<u16, N> {
        Blob::from(*v)
    }
}
impl<const N: usize> From<Option<String>> for Blob<u16, N> {
    #[inline]
    fn from(v: Option<String>) -> Blob<u16, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}
impl<const N: usize> From<Option<&String>> for Blob<u16, N> {
    #[inline]
    fn from(v: Option<&String>) -> Blob<u16, N> {
        v.map_or_else(Blob::new, Blob::from)
    }
}

impl<const N: usize, A: Allocator> Display for Blob<u8, N, A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_cstr())
    }
}
impl<const N: usize, A: Allocator> Display for Blob<u16, N, A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        utf16_display(self.as_slice(), f)
    }
}

impl<T: Copy, const N: usize, A: Allocator> BlobExtend<T, IntoIter<T>> for Blob<T, N, A> {
    /// Specialization to handle values that can be directly Copied from a
    /// [`Vec`] via it's [`Vec::into_iter`] which takes ownership of the Vec.
    ///
    /// This will use [`IntoIter::as_slice`] to directly add the slice using
    /// a Copy.
    #[inline]
    fn _extend(&mut self, i: IntoIter<T>) {
        self.add(i.as_slice());
    }
}
impl<T, const N: usize, A: Allocator, I: Iterator<Item = T>> BlobExtend<T, I> for Blob<T, N, A> {
    /// Specialization to handle values that can be directly taken from an
    /// [`Iterator`].
    ///
    /// This directly will take the owned values and add them.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self.add_iter(i);
    }
}
impl<'a, T: 'a + Copy, const N: usize, A: Allocator> BlobExtend<&'a T, Iter<'a, T>> for Blob<T, N, A> {
    /// Specialization to handle values that can be directly Copied from a
    /// slice.
    ///
    /// This will use [`Iter::as_slice`] to directly add the slice using a Copy.
    #[inline]
    fn _extend(&mut self, i: Iter<'a, T>) {
        self.add(i.as_slice());
    }
}
impl<T: Copy, const N: usize, A: Allocator, const X: usize> BlobExtend<T, array::IntoIter<T, X>> for Blob<T, N, A> {
    /// Specialization to handle values that can be directly Copied from an
    /// array.
    ///
    /// This will use [`array::IntoIter::as_slice`] to directly add the slice
    /// using a Copy.
    #[inline]
    fn _extend(&mut self, i: array::IntoIter<T, X>) {
        self.add(i.as_slice());
    }
}
impl<'a, T: 'a + Copy, const N: usize, A: Allocator, I: Iterator<Item = &'a T>> BlobExtend<&'a T, I> for Blob<T, N, A> {
    /// Specialization to handle values that can be Copied.
    ///
    /// This will use [`Iterator::copied`] on the [`Iterator`] to apply the
    /// values directly using the default extender.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self._extend(i.copied())
    }
}
impl<'a, T: 'a + Clone, const N: usize, A: Allocator, I: Iterator<Item = &'a T>> BlobExtend<&'a T, I> for Blob<T, N, A> {
    /// Specialization to handle values that can be Cloned.
    ///
    /// This will use [`Iterator::cloned`] on the [`Iterator`] to apply the
    /// values directly using the default extender.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self._extend(i.cloned());
    }
}

impl<T, const N: usize, A: Allocator, I: Iterator<Item = T> + ExactSizeIterator> BlobExtend<T, I> for Blob<T, N, A> {
    /// Specialization to handle values that can be directly taken from an
    /// [`Iterator`]. This specialization narrows the scope to handle
    /// [`Iterator`]s that have the [`ExactSizeIterator`] trait, allowing
    /// optimized reservation of space needed.
    ///
    /// This directly will take the owned values and add them.
    default fn _extend(&mut self, i: I) {
        self.add_exact(i.len(), i);
    }
}
impl<'a, T: 'a + Copy, const N: usize, A: Allocator, I: Iterator<Item = &'a T> + ExactSizeIterator> BlobExtend<&'a T, I> for Blob<T, N, A> {
    /// Specialization to handle values that can be Copied. This specialization
    /// narrows the scope to handle [`Iterator`]s that have the
    /// [`ExactSizeIterator`] trait, allowing
    ///
    /// This will use [`Iterator::copied`] on the [`Iterator`] to apply the
    /// values directly using the default extender.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self.add_exact(i.len(), i.copied());
    }
}
