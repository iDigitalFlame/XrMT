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

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::bstr::ByteString;
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
use core::fmt::{Debug, Display, Formatter, Result};
use core::hash::{Hash, Hasher};
use core::intrinsics::transmute_unchecked;
use core::iter::{repeat_n, Extend, FromIterator, IntoIterator, Iterator};
use core::marker::Copy;
use core::mem::{drop, replace, transmute, MaybeUninit, SizedTypeProperties};
use core::ops::{Deref, DerefMut, Drop, FnMut, FnOnce, Index, IndexMut};
use core::option::Option::{self, None, Some};
use core::ptr::{copy, copy_nonoverlapping, drop_in_place, read, swap_nonoverlapping, write, write_bytes};
use core::result::Result::Ok;
use core::slice::{from_raw_parts, from_raw_parts_mut, Iter, SliceIndex};
use core::str::from_utf8;

use xrmt_io::{IoResult, Write};

use crate::blob::failure_oob;
use crate::text::{str_to_utf16, utf16_display, utf16_to_buf, utf16_to_string, utf8_to_lossy};
use crate::Blob;

pub struct Slice<T, const N: usize> {
    len:  usize,
    data: [MaybeUninit<T>; N],
}

trait SliceExtend<T, I> {
    fn _extend(&mut self, i: I);
}

impl<T, const N: usize> Slice<T, N> {
    #[inline]
    pub const fn empty() -> Slice<T, N> {
        Slice {
            len:  0usize,
            data: [const { MaybeUninit::uninit() }; N],
        }
    }
    #[inline]
    pub const fn new(data: [T; N]) -> Slice<T, N> {
        Slice {
            len:  N,
            data: unsafe { transmute_unchecked(data) },
        }
    }
    #[inline]
    pub const fn empty_with_size(size: usize) -> Slice<T, N> {
        Slice {
            len:  if size > N { N } else { size },
            data: [const { MaybeUninit::uninit() }; N],
        }
    }
    #[inline]
    pub const fn with(size: usize, data: [T; N]) -> Slice<T, N> {
        Slice {
            len:  if size > N { N } else { size },
            data: unsafe { transmute_unchecked(data) },
        }
    }

    #[inline]
    pub fn with_iter(i: impl Iterator<Item = T>) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(i);
        s
    }
    #[inline]
    pub fn with_func(len: usize, mut f: impl FnMut() -> T) -> Slice<T, N> {
        let mut s = Slice::empty();
        for _ in 0..len {
            s.push(f()); // We use this since we don't have the 'resize_func'
                         // function.
        }
        s
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        // BCE in min
        unsafe { from_raw_parts(self.as_ptr(), if self.len < N { self.len } else { N }) }
    }
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }
    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.data.as_ptr() as *const T
    }
    #[inline]
    pub const fn as_array(&self) -> &[T; N] {
        unsafe { transmute_unchecked(&self.data) }
    }
    #[inline]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.data.as_mut_ptr() as *mut T
    }
    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // BCE in min
        unsafe { from_raw_parts_mut(self.as_mut_ptr(), if self.len < N { self.len } else { N }) }
    }

    #[inline]
    pub fn clear(&mut self) {
        unsafe { drop_in_place(self.as_mut_slice()) }; // Drop values if needed.
        self.len = 0;
    }
    #[inline]
    pub fn into_array(self) -> [T; N] {
        let (_, v) = self.into_inner();
        v
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
    /// "Drains" the [`Slice`], which will discard all data before `pos` and any
    /// remaining data after `pos` will be at the start of the Blob.
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
    }
    #[inline]
    pub fn push(&mut self, v: T) -> bool {
        if self.len + 1 > N {
            return false;
        }
        // Guarded by above
        unsafe { self.data.get_unchecked_mut(self.len).write(v) };
        self.len += 1;
        true
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
    pub fn into_inner(self) -> (usize, [T; N]) {
        let mut v = self;
        let a = unsafe {
            let mut t = [const { MaybeUninit::uninit() }; N];
            swap_nonoverlapping(v.data.as_mut_ptr(), t.as_mut_ptr(), N);
            transmute_unchecked(t)
        };
        let n = replace(&mut v.len, 0);
        drop(v);
        (n, a)
    }
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        match self.try_remove(index) {
            Some(v) => v,
            None => failure_oob(index, self.len),
        }
    }
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
    pub fn insert(&mut self, index: usize, v: T) -> bool {
        if index > self.len {
            failure_oob(index, self.len);
        }
        if self.len + 1 > N {
            return false;
        }
        unsafe {
            let p = self.as_mut_ptr().add(index);
            if index < self.len {
                copy(p, p.add(1), self.len - index);
            }
            write(p, v);
        }
        self.len += 1;
        true
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
    pub fn pop_if(&mut self, f: impl FnOnce(&mut T) -> bool) -> Option<T> {
        let v = self.last_mut()?;
        if f(v) {
            self.pop()
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn as_slice_of<U>(&self) -> &[U] {
        unsafe { from_raw_parts(self.as_ptr() as *const U, (self.len * T::SIZE) / (U::SIZE)) }
    }
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        self.len = len.min(N)
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
    pub unsafe fn as_array_mut(&mut self) -> &mut [T; N] {
        unsafe { transmute(&mut self.data) }
    }
    #[inline]
    pub unsafe fn set_len_as_bytes(&mut self, len: usize) {
        unsafe { self.set_len(len / T::SIZE) }
    }

    #[inline]
    fn add(&mut self, v: &[T]) -> usize {
        let n = N.saturating_sub(self.len).min(v.len());
        unsafe { copy_nonoverlapping(v.as_ptr(), self.as_mut_ptr().add(self.len), n) };
        self.len += n;
        n
    }
    #[inline]
    fn add_iter(&mut self, i: impl Iterator<Item = T>) {
        for v in i {
            if self.len + 1 > N {
                break;
            }
            unsafe { write(self.as_mut_ptr().add(self.len), v) };
            self.len += 1;
        }
    }
}
impl<T: Copy, const N: usize> Slice<T, N> {
    /// Copies elements from 'v' until none are left or the Slice is full.
    ///
    /// Returns the amount of elements copied.
    #[inline]
    pub fn extend(&mut self, v: &[T]) -> usize {
        self.add(v)
    }
}
impl<T: Clone, const N: usize> Slice<T, N> {
    #[inline]
    pub fn filled_with(v: T) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(repeat_n(v, N));
        s
    }
    #[inline]
    pub fn with_values(v: &[T]) -> Slice<T, N> {
        Slice::from(v)
    }

    #[inline]
    pub fn extend_from_slice(&mut self, v: &[T]) {
        self._extend(v.iter())
    }
}
impl<T: Copy + Default, const N: usize> Slice<T, N> {
    #[inline]
    pub fn with_size(size: usize) -> Slice<T, N> {
        Slice {
            len:  if size > N { N } else { size },
            data: [MaybeUninit::new(T::default()); N],
        }
    }
}
impl<T: Clone + Default, const N: usize> Slice<T, N> {
    #[inline]
    pub fn filled() -> Slice<T, N> {
        Slice::filled_with(Default::default())
    }
}

impl<const N: usize> Slice<u8, N> {
    #[inline]
    pub fn from_cstr(v: &[u8]) -> Slice<u8, N> {
        let mut s = Slice::from(v);
        if let Some(i) = s.iter().position(|i| *i == 0) {
            s.truncate(i);
        }
        s
    }
    #[inline]
    pub fn from_utf16(v: &[u16]) -> Slice<u8, N> {
        let mut s = Slice::empty();
        let n = utf16_to_buf(&mut s, v);
        s.truncate(n);
        s
    }
    #[inline]
    pub fn from_str(v: impl AsRef<str>) -> Slice<u8, N> {
        Slice::from(v.as_ref().as_bytes())
    }

    /// Convert UTF16 to UTF8 by truncating the values to UTF8 limits. No
    /// checking is made to ensure the values are correct.
    ///
    /// Recommended only for UTF16 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf16_unchecked(v: &[u16]) -> Slice<u8, N> {
        let n = match v.iter().position(|i| *i == 0) {
            Some(i) => i,
            None => v.len(),
        };
        let mut s = Slice::empty_with_size(n.min(N));
        for i in 0..s.len {
            // BCE already done above
            let _ = unsafe { *s.data.get_unchecked_mut(i).write(*v.get_unchecked(i) as u8) };
        }
        s
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
impl<const N: usize> Slice<u16, N> {
    #[inline]
    pub fn from_str_utf16(v: impl AsRef<str>) -> Slice<u16, N> {
        let mut s = Slice::empty_with_size(N);
        let n = str_to_utf16(&mut s, v.as_ref());
        s.truncate(n);
        s
    }

    /// Convert UTF8 to UTF16 by extending the values to UTF16 widths while
    /// keeping the higher bytes zero. No checking is made to ensure the
    /// values are correct.
    ///
    /// Recommended only for UTF8 that is only ASCII characters.
    #[inline]
    pub unsafe fn from_utf8_unchecked(v: &[u8]) -> Slice<u16, N> {
        let n = match v.iter().position(|i| *i == 0) {
            Some(i) => i,
            None => v.len(),
        };
        let mut s = Slice::empty_with_size(n.min(N));
        for i in 0..s.len {
            // BCE already done above
            let _ = unsafe { *s.data.get_unchecked_mut(i).write(*v.get_unchecked(i) as u16) };
        }
        s
    }

    #[inline]
    pub fn as_string(&self) -> String {
        utf16_to_string(self.as_slice())
    }
    #[inline]
    pub fn as_utf8<const X: usize>(&self) -> Slice<u8, X> {
        Slice::from_utf16(self)
    }
}

impl<T, const N: usize> Drop for Slice<T, N> {
    #[inline]
    fn drop(&mut self) {
        self.clear();
    }
}
impl<T, const N: usize> Deref for Slice<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, const N: usize> Default for Slice<T, N> {
    #[inline]
    fn default() -> Slice<T, N> {
        Slice::empty()
    }
}
impl<T, const N: usize> DerefMut for Slice<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Hash, const N: usize> Hash for Slice<T, N> {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        Hash::hash(&**self, h)
    }
}
impl<T, const N: usize> AsMut<[T]> for Slice<T, N> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T, const N: usize> AsRef<[T]> for Slice<T, N> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T, const N: usize> Borrow<[T]> for Slice<T, N> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Debug, const N: usize> Debug for Slice<T, N> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self.as_slice(), f)
    }
}
impl<T: Clone, const N: usize> Clone for Slice<T, N> {
    #[inline]
    fn clone(&self) -> Slice<T, N> {
        Slice::with(self.len, self.as_array().clone())
    }
}
impl<T, const N: usize> BorrowMut<[T]> for Slice<T, N> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T, const N: usize> AsMut<Slice<T, N>> for Slice<T, N> {
    #[inline]
    fn as_mut(&mut self) -> &mut Slice<T, N> {
        self
    }
}
impl<T, const N: usize> AsRef<Slice<T, N>> for Slice<T, N> {
    #[inline]
    fn as_ref(&self) -> &Slice<T, N> {
        self
    }
}
impl<'a, T, const N: usize> IntoIterator for &'a Slice<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.as_slice().into_iter()
    }
}

impl<T: Eq, const N: usize> Eq for Slice<T, N> {}
impl<T: PartialEq, const N: usize> PartialEq for Slice<T, N> {
    #[inline]
    fn eq(&self, other: &Slice<T, N>) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl<T: PartialEq<U>, U, const N: usize> PartialEq<[U]> for Slice<T, N> {
    #[inline]
    fn eq(&self, other: &[U]) -> bool {
        self.as_slice().eq(other)
    }
}
impl<T: PartialEq<U>, U, const N: usize> PartialEq<&[U]> for Slice<T, N> {
    #[inline]
    fn eq(&self, other: &&[U]) -> bool {
        self.as_slice().eq(*other)
    }
}
impl<T: PartialEq<U>, U, const N: usize> PartialEq<&mut [U]> for Slice<T, N> {
    #[inline]
    fn eq(&self, other: &&mut [U]) -> bool {
        self.as_slice().eq(*other)
    }
}
impl<T: PartialEq<U>, U, const N: usize, const X: usize> PartialEq<[U; X]> for Slice<T, N> {
    #[inline]
    fn eq(&self, other: &[U; X]) -> bool {
        self.as_slice().eq(other)
    }
}

impl<T: Ord, const N: usize> Ord for Slice<T, N> {
    #[inline]
    fn cmp(&self, other: &Slice<T, N>) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl<T: PartialOrd, const N: usize> PartialOrd for Slice<T, N> {
    fn partial_cmp(&self, other: &Slice<T, N>) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<const N: usize> Write for Slice<u8, N> {
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

impl<T, const N: usize> FromIterator<T> for Slice<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(i: I) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(i.into_iter());
        s
    }
}
impl<'a, T: Clone, const N: usize> FromIterator<&'a T> for Slice<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a T>>(i: I) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(i.into_iter());
        s
    }
}

impl<T, const N: usize, I: SliceIndex<[T]>> Index<I> for Slice<T, N> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}
impl<T, const N: usize, I: SliceIndex<[T]>> IndexMut<I> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}

impl<T, const N: usize> Extend<T> for Slice<T, N> {
    #[inline]
    fn extend_one(&mut self, v: T) {
        self.push(v);
    }
    #[inline]
    fn extend_reserve(&mut self, _len: usize) {}
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, i: I) {
        self._extend(i.into_iter())
    }
}
impl<'a, T: 'a + Clone, const N: usize> Extend<&'a T> for Slice<T, N> {
    #[inline]
    fn extend_one(&mut self, v: &'a T) {
        self.push(v.clone());
    }
    #[inline]
    fn extend_reserve(&mut self, _len: usize) {}
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, i: I) {
        self._extend(i.into_iter())
    }
}

impl<T: Clone, const N: usize> From<&[T]> for Slice<T, N> {
    #[inline]
    fn from(v: &[T]) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(v.iter());
        s
    }
}
impl<T: Clone, const N: usize> From<Vec<T>> for Slice<T, N> {
    #[inline]
    fn from(v: Vec<T>) -> Slice<T, N> {
        let mut s = Slice::empty();
        s._extend(v.into_iter());
        s
    }
}
impl<T: Clone, const N: usize> From<&Vec<T>> for Slice<T, N> {
    #[inline]
    fn from(v: &Vec<T>) -> Slice<T, N> {
        Slice::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize> From<Rc<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: Rc<[T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&mut [T]> for Slice<T, N> {
    #[inline]
    fn from(v: &mut [T]) -> Slice<T, N> {
        let mut s = Slice::empty();
        let _ = s.add(v);
        s
    }
}
impl<T: Clone, const N: usize> From<&Rc<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: &Rc<[T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<Arc<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: Arc<[T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<Box<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: Box<[T]>) -> Slice<T, N> {
        Slice::from(v.into_vec())
    }
}
impl<T: Clone, const N: usize> From<&Arc<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: &Arc<[T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&Box<[T]>> for Slice<T, N> {
    #[inline]
    fn from(v: &Box<[T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize> From<&mut Vec<T>> for Slice<T, N> {
    #[inline]
    fn from(v: &mut Vec<T>) -> Slice<T, N> {
        Slice::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize> From<Cow<'_, [T]>> for Slice<T, N> {
    #[inline]
    fn from(v: Cow<'_, [T]>) -> Slice<T, N> {
        Slice::from(v.as_ref())
    }
}
impl<T: Clone, const N: usize, const X: usize> From<[T; X]> for Slice<T, N> {
    #[inline]
    fn from(v: [T; X]) -> Slice<T, N> {
        Slice::from(v.as_slice())
    }
}
impl<T: Clone, const N: usize, const X: usize, A: Allocator> From<Blob<T, X, A>> for Slice<T, N> {
    #[inline]
    fn from(v: Blob<T, X, A>) -> Slice<T, N> {
        Slice::from(v.as_slice())
    }
}

impl<const N: usize> From<&str> for Slice<u8, N> {
    #[inline]
    fn from(v: &str) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<&CStr> for Slice<u8, N> {
    #[inline]
    fn from(v: &CStr) -> Slice<u8, N> {
        Slice::from(v.to_bytes())
    }
}
impl<const N: usize> From<String> for Slice<u8, N> {
    #[inline]
    fn from(v: String) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<&String> for Slice<u8, N> {
    #[inline]
    fn from(v: &String) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<CString> for Slice<u8, N> {
    #[inline]
    fn from(v: CString) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<&CString> for Slice<u8, N> {
    #[inline]
    fn from(v: &CString) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<&ByteStr> for Slice<u8, N> {
    #[inline]
    fn from(v: &ByteStr) -> Slice<u8, N> {
        Slice::from(&v.0)
    }
}
impl<const N: usize> From<ByteString> for Slice<u8, N> {
    #[inline]
    fn from(v: ByteString) -> Slice<u8, N> {
        Slice::from(v.as_slice())
    }
}
impl<const N: usize> From<&ByteString> for Slice<u8, N> {
    #[inline]
    fn from(v: &ByteString) -> Slice<u8, N> {
        Slice::from(v.as_slice())
    }
}
impl<const N: usize> From<Option<&str>> for Slice<u8, N> {
    #[inline]
    fn from(v: Option<&str>) -> Slice<u8, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}
impl<const N: usize> From<Cow<'_, &str>> for Slice<u8, N> {
    #[inline]
    fn from(v: Cow<'_, &str>) -> Slice<u8, N> {
        Slice::from(v.as_bytes())
    }
}
impl<const N: usize> From<Option<String>> for Slice<u8, N> {
    #[inline]
    fn from(v: Option<String>) -> Slice<u8, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}
impl<const N: usize> From<Option<&String>> for Slice<u8, N> {
    #[inline]
    fn from(v: Option<&String>) -> Slice<u8, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}

impl<const N: usize> From<&str> for Slice<u16, N> {
    #[inline]
    fn from(v: &str) -> Slice<u16, N> {
        Slice::from_str_utf16(v)
    }
}
impl<const N: usize> From<String> for Slice<u16, N> {
    #[inline]
    fn from(v: String) -> Slice<u16, N> {
        Slice::from(v.as_str())
    }
}
impl<const N: usize> From<&String> for Slice<u16, N> {
    #[inline]
    fn from(v: &String) -> Slice<u16, N> {
        Slice::from(v.as_str())
    }
}
impl<const N: usize> From<Option<&str>> for Slice<u16, N> {
    #[inline]
    fn from(v: Option<&str>) -> Slice<u16, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}
impl<const N: usize> From<Cow<'_, &str>> for Slice<u16, N> {
    #[inline]
    fn from(v: Cow<'_, &str>) -> Slice<u16, N> {
        Slice::from(*v)
    }
}
impl<const N: usize> From<Option<String>> for Slice<u16, N> {
    #[inline]
    fn from(v: Option<String>) -> Slice<u16, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}
impl<const N: usize> From<Option<&String>> for Slice<u16, N> {
    #[inline]
    fn from(v: Option<&String>) -> Slice<u16, N> {
        v.map_or_else(Slice::empty, Slice::from)
    }
}

impl<const N: usize> Display for Slice<u8, N> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_cstr())
    }
}
impl<const N: usize> Display for Slice<u16, N> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        utf16_display(self.as_slice(), f)
    }
}

impl<T: Copy, const N: usize> SliceExtend<T, IntoIter<T>> for Slice<T, N> {
    /// Specialization to handle values that can be directly Copied from a
    /// [`Vec`] via it's [`Vec::into_iter`] which takes ownership of the Vec.
    ///
    /// This will use [`IntoIter::as_slice`] to directly add the slice using
    /// a Copy.
    #[inline]
    fn _extend(&mut self, i: IntoIter<T>) {
        let _ = self.add(i.as_slice());
    }
}
impl<T, const N: usize, I: Iterator<Item = T>> SliceExtend<T, I> for Slice<T, N> {
    /// Specialization to handle values that can be directly taken from an
    /// [`Iterator`].
    ///
    /// This directly will take the owned values and add them.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self.add_iter(i);
    }
}
impl<'a, T: 'a + Copy, const N: usize> SliceExtend<&'a T, Iter<'a, T>> for Slice<T, N> {
    /// Specialization to handle values that can be directly Copied from a
    /// slice.
    ///
    /// This will use [`Iter::as_slice`] to directly add the slice using a Copy.
    #[inline]
    fn _extend(&mut self, i: Iter<'a, T>) {
        let _ = self.add(i.as_slice());
    }
}
impl<T: Copy, const N: usize, const X: usize> SliceExtend<T, array::IntoIter<T, X>> for Slice<T, N> {
    /// Specialization to handle values that can be directly Copied from an
    /// array.
    ///
    /// This will use [`array::IntoIter::as_slice`] to directly add the slice
    /// using a Copy.
    #[inline]
    fn _extend(&mut self, i: array::IntoIter<T, X>) {
        let _ = self.add(i.as_slice());
    }
}
impl<'a, T: 'a + Copy, const N: usize, I: Iterator<Item = &'a T>> SliceExtend<&'a T, I> for Slice<T, N> {
    /// Specialization to handle values that can be Copied.
    ///
    /// This will use [`Iterator::copied`] on the [`Iterator`] to apply the
    /// values directly using the default extender.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self.add_iter(i.copied());
    }
}
impl<'a, T: 'a + Clone, const N: usize, I: Iterator<Item = &'a T>> SliceExtend<&'a T, I> for Slice<T, N> {
    /// Specialization to handle values that can be Cloned.
    ///
    /// This will use [`Iterator::cloned`] on the [`Iterator`] to apply the
    /// values directly using the default extender.
    #[inline]
    default fn _extend(&mut self, i: I) {
        self._extend(i.cloned());
    }
}
