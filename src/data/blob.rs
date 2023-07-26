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

use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::{self, Debug, Formatter};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};
use core::slice::Iter;
use core::{cmp, mem, slice};

use crate::util::stx::prelude::*;

pub struct Slice<T: Sized, const N: usize> {
    pub data: [T; N],
    pub len:  usize,
}
pub struct Blob<T: Copy, const N: usize = 256> {
    pos:   usize,
    swap:  bool,
    heap:  MaybeUninit<Vec<T>>,
    stack: [MaybeUninit<T>; N],
}

impl<T: Copy, const N: usize> Blob<T, N> {
    #[inline]
    pub const fn new() -> Blob<T, N> {
        Blob {
            pos:   0,
            swap:  false,
            heap:  MaybeUninit::uninit(),
            stack: [MaybeUninit::uninit(); N],
        }
    }

    #[inline]
    pub fn with_capacity(size: usize) -> Blob<T, N> {
        if size > N {
            Blob {
                pos:   0,
                swap:  true,
                heap:  MaybeUninit::new(Vec::with_capacity(size)),
                stack: [MaybeUninit::uninit(); N],
            }
        } else {
            Blob {
                pos:   0,
                swap:  false,
                heap:  MaybeUninit::uninit(),
                stack: [MaybeUninit::uninit(); N],
            }
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0)
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.pos
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pos == 0
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[0..self.pos]
        } else {
            unsafe { mem::transmute(&self.stack[0..self.pos]) }
        }
    }
    #[inline]
    pub fn push(&mut self, val: T) {
        self.size_check(1);
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.push(val);
        } else {
            self.stack[self.pos].write(val);
        }
        self.pos += 1;
    }
    #[inline]
    pub fn is_on_heap(&self) -> bool {
        self.swap
    }
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        if self.swap {
            unsafe { &*self.heap.as_ptr() }.as_ptr() as *const T
        } else {
            self.stack[0].as_ptr() as *const T
        }
    }
    pub fn cut(&mut self, pos: usize) {
        if pos == self.pos {
            return;
        }
        if pos > self.len() {
            self.truncate(0);
            return;
        }
        let n = self.pos - pos;
        if self.swap {
            unsafe {
                (*self.heap.as_mut_ptr()).copy_within(pos..self.pos, 0);
                (*self.heap.as_mut_ptr()).truncate(n);
            }
        } else {
            for i in pos..self.pos {
                unsafe { *(self.stack[i - pos].assume_init_mut()) = *(self.stack[i].as_ptr()) }
            }
            for i in n..self.pos {
                unsafe { self.stack[i].assume_init_drop() }
            }
        }
        self.pos = n;
    }
    #[inline]
    pub fn len_as_bytes(&self) -> usize {
        self.pos * mem::size_of::<T>()
    }
    #[inline]
    pub fn as_ptr_of<U>(&self) -> *const U {
        self.as_ptr() as *const U
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.as_mut_ptr() as *mut T
        } else {
            self.stack.as_mut_ptr() as *mut T
        }
    }
    #[inline]
    pub fn as_ref_of<'a, U>(&self) -> &'a U {
        unsafe { &*(self.as_ptr() as *const U) }
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[0..self.pos]
        } else {
            unsafe { mem::transmute(&mut self.stack[0..self.pos]) }
        }
    }
    pub fn truncate(&mut self, new_size: usize) {
        if new_size >= self.pos {
            return;
        }
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.truncate(new_size);
        } else {
            for i in new_size..self.pos {
                unsafe { self.stack[i].assume_init_drop() }
            }
        }
        self.pos = new_size;
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.reserve(additional);
        }
    }
    #[inline]
    pub fn extend_from_slice(&mut self, other: &[T]) {
        self.write_data(other.len(), other)
    }
    pub fn resize_with(&mut self, new_size: usize, val: T) {
        if self.pos > new_size {
            return;
        }
        self.size_check(new_size);
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.resize(new_size, val);
        } else {
            for i in self.pos..new_size {
                self.stack[i].write(val);
            }
        }
        self.pos += new_size;
    }
    #[inline]
    pub fn as_array<const X: usize>(&self) -> Option<[T; X]> {
        if self.len() < X {
            return None;
        }
        Some(unsafe { *&*(self.as_slice()[0..X].as_ptr() as *const [T; X]) })
    }

    #[inline]
    pub unsafe fn as_slice_of<U: Sized>(&self) -> &[U] {
        let (u, t) = (mem::size_of::<U>(), mem::size_of::<T>());
        slice::from_raw_parts(
            self.as_ptr() as *const U,
            if t > u { self.pos * t } else { self.pos / u },
        )
    }
    #[inline]
    pub unsafe fn write_item<U: Sized>(&mut self, v: U) {
        let n = cmp::max(mem::size_of::<U>() / mem::size_of::<T>(), 1);
        self.write_data(n, slice::from_raw_parts((&v as *const U) as *const T, n))
    }
    #[inline]
    pub unsafe fn write_item_ptr<U>(&mut self, size: usize, v: *const U) {
        let n = cmp::max(size / mem::size_of::<T>(), 1);
        self.write_data(n, slice::from_raw_parts(v as *const T, n))
    }
    #[inline]
    pub unsafe fn read_item<U: Sized>(&self, pos: usize, v: &mut U) -> usize {
        self.read_item_ptr(pos, mem::size_of::<T>(), v as *mut U)
    }
    pub unsafe fn read_item_ptr<U>(&self, pos: usize, len: usize, v: *mut U) -> usize {
        if pos > self.pos {
            return 0;
        }
        let i = len / mem::size_of::<T>();
        let n = if pos + i > self.pos { self.pos - pos } else { i };
        slice::from_raw_parts_mut(v as *mut T, n).copy_from_slice(if self.swap {
            &(*self.heap.as_ptr())[pos..pos + n]
        } else {
            mem::transmute(&self.stack[pos..pos + n])
        });
        n
    }

    fn size_check(&mut self, new_size: usize) {
        if self.pos + new_size < N {
            return;
        }
        if !self.swap {
            self.heap
                .write(Vec::with_capacity(N + new_size))
                .extend_from_slice(unsafe { mem::transmute(&self.stack[0..self.pos]) });
            for i in 0..self.pos {
                unsafe { self.stack[i].assume_init_drop() };
            }
            self.swap = true;
        }
        unsafe { &mut *self.heap.as_mut_ptr() }.reserve(new_size);
    }
    fn write_data(&mut self, size: usize, buf: &[T]) {
        self.size_check(size);
        if self.swap {
            unsafe { &mut *self.heap.as_mut_ptr() }.extend_from_slice(&buf[0..size]);
        } else {
            for i in 0..size {
                self.stack[self.pos + i].write(buf[i]);
            }
        }
        self.pos += size;
    }
}
impl<T: Copy + Default, const N: usize> Blob<T, N> {
    #[inline]
    pub fn resize(&mut self, new_size: usize) {
        self.resize_with(new_size, Default::default())
    }
    #[inline]
    pub fn resize_as_bytes(&mut self, new_size: usize) {
        self.resize_with(new_size * mem::size_of::<T>(), Default::default())
    }
}

impl<T: Sized, const N: usize> Slice<T, N> {
    #[inline]
    pub const fn full(data: [T; N]) -> Slice<T, N> {
        Slice { data, len: N }
    }
    #[inline]
    pub const fn new(len: usize, data: [T; N]) -> Slice<T, N> {
        Slice { data, len }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data[0..self.len]
    }
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.data.as_ptr()
    }
    #[inline]
    pub fn into_inner(self) -> [T; N] {
        self.data
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.data.as_mut_ptr()
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data[0..self.len]
    }
}
impl<T: Sized + Default + Copy, const N: usize> Slice<T, N> {
    #[inline]
    pub fn empty() -> Slice<T, N> {
        Slice {
            data: [Default::default(); N],
            len:  0,
        }
    }
    #[inline]
    pub fn with_len(len: usize) -> Slice<T, N> {
        Slice {
            data: [Default::default(); N],
            len,
        }
    }
}

impl<T: Copy, const N: usize> Drop for Blob<T, N> {
    #[inline]
    fn drop(&mut self) {
        if self.swap {
            unsafe { self.heap.assume_init_drop() }
        } else {
            for i in 0..self.pos {
                unsafe { self.stack[i].assume_init_drop() }
            }
        }
    }
}
impl<T: Copy, const N: usize> Clone for Blob<T, N> {
    fn clone(&self) -> Blob<T, N> {
        let mut v = Blob::with_capacity(self.pos);
        v.extend_from_slice(self.as_slice());
        v
    }
}
impl<T: Copy, const N: usize> Deref for Blob<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Copy, const N: usize> Default for Blob<T, N> {
    #[inline]
    fn default() -> Blob<T, N> {
        Blob::new()
    }
}
impl<T: Copy, const N: usize> DerefMut for Blob<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Copy, const N: usize> Extend<T> for Blob<T, N> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let x = iter.into_iter();
        if let Some(n) = x.size_hint().1 {
            self.reserve(n);
        }
        for i in x {
            self.push(i)
        }
    }
}
impl<T: Copy, const N: usize> From<&[T]> for Blob<T, N> {
    #[inline]
    fn from(v: &[T]) -> Blob<T, N> {
        let mut b = Blob::with_capacity(v.len());
        b.write_data(v.len(), v);
        b
    }
}
impl<T: Copy, const N: usize> AsRef<[T]> for Blob<T, N> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Copy, const N: usize> Borrow<[T]> for Blob<T, N> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Copy, const N: usize> Index<usize> for Blob<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &T {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[index]
        } else {
            unsafe { self.stack[index].assume_init_ref() }
        }
    }
}
impl<T: Copy + Debug, const N: usize> Debug for Blob<T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}
impl<T: Copy, const N: usize> BorrowMut<[T]> for Blob<T, N> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Copy, const N: usize> From<&mut [T]> for Blob<T, N> {
    #[inline]
    fn from(v: &mut [T]) -> Blob<T, N> {
        let mut b = Blob::with_capacity(v.len());
        b.write_data(v.len(), v);
        b
    }
}
impl<T: Copy, const N: usize> IndexMut<usize> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut T {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[index]
        } else {
            unsafe { self.stack[index].assume_init_mut() }
        }
    }
}
impl<T: Copy, const N: usize> FromIterator<T> for Blob<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Blob<T, N> {
        let mut b = Blob::new();
        let x = iter.into_iter();
        if let Some(n) = x.size_hint().1 {
            b.reserve(n);
        }
        for i in x {
            b.push(i)
        }
        b
    }
}
impl<T: Copy, const N: usize> Index<RangeFull> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, _index: RangeFull) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[0..self.pos]
        } else {
            unsafe { mem::transmute(&self.stack[0..self.pos]) }
        }
    }
}
impl<T: Copy, const N: usize> AsMut<Blob<T, N>> for Blob<T, N> {
    #[inline]
    fn as_mut(&mut self) -> &mut Blob<T, N> {
        self
    }
}
impl<'a, T: Copy, const N: usize> Extend<&'a T> for Blob<T, N> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for i in iter {
            self.write_data(1, slice::from_ref(i))
        }
    }
}
impl<T: Copy, const N: usize> Index<Range<usize>> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: Range<usize>) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[index]
        } else {
            unsafe { mem::transmute(&self.stack[index]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<RangeFull> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, _index: RangeFull) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[0..self.pos]
        } else {
            unsafe { mem::transmute(&mut self.stack[0..self.pos]) }
        }
    }
}
impl<'a, T: Copy, const N: usize> IntoIterator for &'a Blob<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.as_slice().into_iter()
    }
}
impl<T: Copy, const N: usize> Index<RangeTo<usize>> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeTo<usize>) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[0..index.end]
        } else {
            unsafe { mem::transmute(&self.stack[0..index.end]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<Range<usize>> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: Range<usize>) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[index]
        } else {
            unsafe { mem::transmute(&mut self.stack[index]) }
        }
    }
}
impl<'a, T: Copy, const N: usize> FromIterator<&'a T> for Blob<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a T>>(iter: I) -> Blob<T, N> {
        let mut b = Blob::new();
        let x = iter.into_iter();
        if let Some(n) = x.size_hint().1 {
            b.reserve(n);
        }
        for i in x {
            b.push(*i)
        }
        b
    }
}
impl<T: Copy, const N: usize> Index<RangeFrom<usize>> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeFrom<usize>) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[index.start..self.pos]
        } else {
            unsafe { mem::transmute(&self.stack[index.start..self.pos]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<RangeTo<usize>> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[0..index.end]
        } else {
            unsafe { mem::transmute(&mut self.stack[0..index.end]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<RangeFrom<usize>> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[index.start..self.pos]
        } else {
            unsafe { mem::transmute(&mut self.stack[index.start..self.pos]) }
        }
    }
}
impl<T: Copy, const N: usize, const X: usize> From<[T; X]> for Blob<T, N> {
    #[inline]
    fn from(v: [T; X]) -> Blob<T, N> {
        let mut b = Blob::with_capacity(v.len());
        b.write_data(v.len(), &v);
        b
    }
}
impl<T: Copy, const N: usize> Index<RangeInclusive<usize>> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeInclusive<usize>) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[index]
        } else {
            unsafe { mem::transmute(&self.stack[index]) }
        }
    }
}
impl<T: Copy, const N: usize> Index<RangeToInclusive<usize>> for Blob<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeToInclusive<usize>) -> &[T] {
        if self.swap {
            &(unsafe { &*self.heap.as_ptr() })[0..=index.end]
        } else {
            unsafe { mem::transmute(&self.stack[0..=index.end]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<RangeInclusive<usize>> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeInclusive<usize>) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[index]
        } else {
            unsafe { mem::transmute(&mut self.stack[index]) }
        }
    }
}
impl<T: Copy, const N: usize> IndexMut<RangeToInclusive<usize>> for Blob<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeToInclusive<usize>) -> &mut [T] {
        if self.swap {
            &mut (unsafe { &mut *self.heap.as_mut_ptr() })[0..=index.end]
        } else {
            unsafe { mem::transmute(&mut self.stack[0..=index.end]) }
        }
    }
}
impl<T: Sized + Copy, const N: usize, const X: usize> From<Slice<T, X>> for Blob<T, N> {
    #[inline]
    fn from(v: Slice<T, X>) -> Blob<T, N> {
        let mut b = Blob::new();
        b.extend_from_slice(v.as_slice());
        b
    }
}

impl<const N: usize> Blob<u8, N> {
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(self.as_slice()) }
    }
}
impl<const N: usize> ToString for Blob<u8, N> {
    #[inline]
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl<T: Sized, const N: usize> Deref for Slice<T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Sized, const N: usize> DerefMut for Slice<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Sized, const N: usize> AsRef<[T]> for Slice<T, N> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Sized, const N: usize> Borrow<[T]> for Slice<T, N> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: Sized, const N: usize> Index<usize> for Slice<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &T {
        &self.data[index]
    }
}
impl<T: Sized + Debug, const N: usize> Debug for Slice<T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}
impl<T: Sized, const N: usize> BorrowMut<[T]> for Slice<T, N> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}
impl<T: Sized, const N: usize> IndexMut<usize> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.data[index]
    }
}
impl<T: Sized, const N: usize> Index<RangeFull> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, _index: RangeFull) -> &[T] {
        &self.data[0..self.len]
    }
}
impl<T: Sized, const N: usize> AsMut<Slice<T, N>> for Slice<T, N> {
    #[inline]
    fn as_mut(&mut self) -> &mut Slice<T, N> {
        self
    }
}
impl<T: Sized, const N: usize> Index<Range<usize>> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: Range<usize>) -> &[T] {
        &self.data[index]
    }
}
impl<T: Sized, const N: usize> IndexMut<RangeFull> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, _index: RangeFull) -> &mut [T] {
        &mut self.data[0..self.len]
    }
}
impl<'a, T: Sized, const N: usize> IntoIterator for &'a Slice<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.data[0..self.len].into_iter()
    }
}
impl<T: Sized, const N: usize> Index<RangeTo<usize>> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeTo<usize>) -> &[T] {
        &self.data[0..index.end]
    }
}
impl<T: Sized, const N: usize> IndexMut<Range<usize>> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: Range<usize>) -> &mut [T] {
        &mut self.data[index]
    }
}
impl<T: Sized, const N: usize> Index<RangeFrom<usize>> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeFrom<usize>) -> &[T] {
        &self.data[index.start..self.len]
    }
}
impl<T: Sized, const N: usize> IndexMut<RangeTo<usize>> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut [T] {
        &mut self.data[0..index.end]
    }
}
impl<T: Sized + Default + Copy, const N: usize> Default for Slice<T, N> {
    #[inline]
    fn default() -> Slice<T, N> {
        Slice::empty()
    }
}
impl<T: Sized, const N: usize> IndexMut<RangeFrom<usize>> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut [T] {
        &mut self.data[index.start..self.len]
    }
}
impl<T: Sized + Default + Copy, const N: usize> From<&[T]> for Slice<T, N> {
    #[inline]
    fn from(v: &[T]) -> Slice<T, N> {
        let mut s = Slice::with_len(cmp::min(v.len(), N));
        s.data[0..s.len].copy_from_slice(&v[0..s.len]);
        s
    }
}
impl<T: Sized, const N: usize> Index<RangeInclusive<usize>> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeInclusive<usize>) -> &[T] {
        &self.data[index]
    }
}
impl<T: Sized, const N: usize> Index<RangeToInclusive<usize>> for Slice<T, N> {
    type Output = [T];

    #[inline]
    fn index(&self, index: RangeToInclusive<usize>) -> &[T] {
        &self.data[0..=index.end]
    }
}
impl<T: Sized, const N: usize> IndexMut<RangeInclusive<usize>> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeInclusive<usize>) -> &mut [T] {
        &mut self.data[index]
    }
}
impl<T: Sized + Default + Copy, const N: usize> From<&mut [T]> for Slice<T, N> {
    #[inline]
    fn from(v: &mut [T]) -> Slice<T, N> {
        let mut s = Slice::with_len(cmp::min(v.len(), N));
        s.data[0..s.len].copy_from_slice(&v[0..s.len]);
        s
    }
}
impl<T: Sized + Default + Copy, const N: usize> FromIterator<T> for Slice<T, N> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Slice<T, N> {
        let mut s = Slice::empty();
        for i in iter {
            if s.len >= N {
                break;
            }
            s.data[s.len] = i;
            s.len += 1;
        }
        s
    }
}
impl<T: Sized, const N: usize> IndexMut<RangeToInclusive<usize>> for Slice<T, N> {
    #[inline]
    fn index_mut(&mut self, index: RangeToInclusive<usize>) -> &mut [T] {
        &mut self.data[0..=index.end]
    }
}
impl<T: Sized + Copy, const N: usize, const X: usize> From<Blob<T, X>> for Slice<T, N> {
    #[inline]
    fn from(v: Blob<T, X>) -> Slice<T, N> {
        let mut s = Slice {
            len:  cmp::min(v.len(), N),
            data: unsafe { mem::zeroed() },
        };
        match v.len().cmp(&N) {
            Ordering::Equal => s.data.copy_from_slice(&v),
            Ordering::Less => s.data[0..v.len()].copy_from_slice(&v),
            Ordering::Greater => s.data.copy_from_slice(&v[0..N]),
        }
        s
    }
}
impl<T: Sized + Default + Copy, const N: usize, const X: usize> From<[T; X]> for Slice<T, N> {
    #[inline]
    fn from(v: [T; X]) -> Slice<T, N> {
        let mut s = Slice::with_len(cmp::min(v.len(), N));
        s.data[0..s.len].copy_from_slice(&v[0..s.len]);
        s
    }
}

impl<const N: usize> Slice<u8, N> {
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(self.as_slice()) }
    }
}
impl<const N: usize> ToString for Slice<u8, N> {
    #[inline]
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}
