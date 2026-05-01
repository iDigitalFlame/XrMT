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

extern crate xrmt_data;

use alloc::alloc::Global;
use alloc::string::String;
use core::alloc::Allocator;
use core::clone::Clone;
use core::convert::{From, Into};
use core::iter::{FusedIterator, Iterator};
use core::marker::{Copy, PhantomData};
use core::ops::Deref;
use core::option::Option::{self, None, Some};
use core::ptr::{null, null_mut};
use core::slice::from_raw_parts;

use xrmt_data::text::{utf16_to_fiber_in, utf16_to_string};
use xrmt_data::{Blob, Fiber, Slice};

use crate::structs::{Chars, StringLike, StringLikeU16, WCharLike, WCharSlice, WChars};

pub struct EnvironmentIter<'a> {
    pos: usize,
    env: &'a EnvironmentBlock<'a>,
}
#[repr(transparent)]
pub struct EnvironmentBlock<'a> {
    ptr: *const u16,
    _p:  PhantomData<&'a [u16]>,
}
pub struct Variable<'a>(&'a [u16]);

impl<'a> Variable<'a> {
    #[inline]
    pub fn as_slice(&self) -> &[u16] {
        self.0
    }
    #[inline]
    pub fn contains_value(&self) -> bool {
        self.value().is_some()
    }
    #[inline]
    pub fn key_as_u8(&self) -> Option<Chars> {
        self.key_as_u8_in(Global)
    }
    #[inline]
    pub fn is_key(&self, key: &[u16]) -> bool {
        if self.0.is_empty() {
            return false;
        }
        match self.key_slice().iter().position(|v| *v == 0x3D) {
            Some(i) => i + 1 > 1 && i + 1 == key.len() && match_u16(unsafe { self.0.get_unchecked(0..i + 1) }, key),
            None => false,
        }
    }
    #[inline]
    pub fn key_as_u16(&self) -> Option<WChars> {
        self.key_as_u16_in(Global)
    }
    #[inline]
    pub fn value_as_u8(&self) -> Option<Chars> {
        self.value_as_u8_in(Global)
    }
    #[inline]
    pub fn key_as_fiber(&self) -> Option<Fiber> {
        self.key_as_fiber_in(Global)
    }
    #[inline]
    pub fn key(&self) -> Option<WCharSlice<'a>> {
        self.key_as_slice().map(WCharSlice::from)
    }
    #[inline]
    pub fn is_key_u8(&self, key: &[u8]) -> bool {
        self.key_as_slice().map_or(false, |v| match_u16_u8(v, key))
    }
    #[inline]
    pub fn value_as_u16(&self) -> Option<WChars> {
        self.value_as_u16_in(Global)
    }
    #[inline]
    pub fn value_as_fiber(&self) -> Option<Fiber> {
        self.value_as_fiber_in(Global)
    }
    #[inline]
    pub fn value(&self) -> Option<WCharSlice<'a>> {
        self.value_as_slice().map(WCharSlice::from)
    }
    #[inline]
    pub fn key_as_string(&self) -> Option<String> {
        Some(utf16_to_string(self.key()?.as_slice()))
    }
    #[inline]
    pub fn key_as_slice(&self) -> Option<&'a [u16]> {
        if self.0.is_empty() {
            return None;
        }
        Some(unsafe {
            self.0
                .get_unchecked(0..self.key_slice().iter().position(|v| *v == 0x3D)? + 1)
        })
    }
    #[inline]
    pub fn value_as_string(&self) -> Option<String> {
        Some(utf16_to_string(self.value()?.as_slice()))
    }
    #[inline]
    pub fn value_as_slice(&self) -> Option<&'a [u16]> {
        let n = self.key_slice().iter().position(|v| *v == 0x3D)?;
        if n + 2 >= self.0.len() {
            None
        } else {
            Some(unsafe { self.0.get_unchecked(n + 2..) })
        }
    }
    #[inline]
    pub fn key_as_u8_slice<const N: usize>(&self) -> Option<Slice<u8, N>> {
        self.key().map(|v| Slice::from_utf16(v.as_slice()))
    }
    #[inline]
    pub fn key_as_u8_in<A: Allocator>(&self, alloc: A) -> Option<Chars<A>> {
        self.key().map(|v| Blob::from_utf16_in(v.as_slice(), alloc))
    }
    #[inline]
    pub fn value_as_u8_slice<const N: usize>(&self) -> Option<Slice<u8, N>> {
        self.value().map(|v| Slice::from_utf16(v.as_slice()))
    }
    #[inline]
    pub fn key_as_u16_slice<const N: usize>(&self) -> Option<Slice<u16, N>> {
        self.key().map(|v| Slice::from(v.as_slice()))
    }
    #[inline]
    pub fn key_as_u16_in<A: Allocator>(&self, alloc: A) -> Option<WChars<A>> {
        self.key().map(|v| Blob::with_values_in(v.as_slice(), alloc))
    }
    #[inline]
    pub fn value_as_u8_in<A: Allocator>(&self, alloc: A) -> Option<Chars<A>> {
        self.value().map(|v| Blob::from_utf16_in(v.as_slice(), alloc))
    }
    #[inline]
    pub fn key_as_fiber_in<A: Allocator>(&self, alloc: A) -> Option<Fiber<A>> {
        self.key().map(|v| utf16_to_fiber_in(v.as_slice(), alloc))
    }
    #[inline]
    pub fn value_as_u16_slice<const N: usize>(&self) -> Option<Slice<u16, N>> {
        self.value().map(|v| Slice::from(v.as_slice()))
    }
    #[inline]
    pub fn value_as_u16_in<A: Allocator>(&self, alloc: A) -> Option<WChars<A>> {
        self.value().map(|v| Blob::with_values_in(v.as_slice(), alloc))
    }
    #[inline]
    pub fn value_as_fiber_in<A: Allocator>(&self, alloc: A) -> Option<Fiber<A>> {
        self.value().map(|v| utf16_to_fiber_in(v.as_slice(), alloc))
    }

    #[inline(always)]
    fn key_slice(&self) -> &[u16] {
        unsafe { self.0.get_unchecked(1..) }
    }
}
impl<'a> EnvironmentBlock<'a> {
    #[inline]
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
    #[inline]
    pub fn iter(&'a self) -> EnvironmentIter<'a> {
        EnvironmentIter { pos: 0, env: self }
    }
    #[inline]
    pub fn find_as_u8(&'a self, key: impl Into<WCharLike<'a>>) -> Option<Chars> {
        self.find_as_u8_in(key, Global)
    }
    #[inline]
    pub fn find(&'a self, key: impl Into<WCharLike<'a>>) -> Option<Variable<'a>> {
        let k = key.into();
        self.iter().find(|v| v.is_key(&k))
    }
    #[inline]
    pub fn find_as_str(&'a self, key: impl Into<WCharLike<'a>>) -> Option<String> {
        self.find(key)?.value_as_string()
    }
    #[inline]
    pub fn find_as_u16(&'a self, key: impl Into<WCharLike<'a>>) -> Option<WChars> {
        self.find_as_u16_in(key, Global)
    }
    #[inline]
    pub fn find_as_fiber(&'a self, key: impl Into<WCharLike<'a>>) -> Option<Fiber> {
        self.find_as_fiber_in(key, Global)
    }
    #[inline]
    pub fn find_as_u8_slice<const N: usize>(&'a self, key: impl Into<WCharLike<'a>>) -> Option<Slice<u8, N>> {
        self.find(key)?.value_as_u8_slice()
    }
    #[inline]
    pub fn find_as_u8_in<A: Allocator>(&'a self, key: impl Into<WCharLike<'a>>, alloc: A) -> Option<Chars<A>> {
        self.find(key)?.value_as_u8_in(alloc)
    }
    #[inline]
    pub fn find_as_u16_slice<const N: usize>(&'a self, key: impl Into<WCharLike<'a>>) -> Option<Slice<u16, N>> {
        self.find(key)?.value_as_u16_slice()
    }
    #[inline]
    pub fn find_as_u16_in<A: Allocator>(&'a self, key: impl Into<WCharLike<'a>>, alloc: A) -> Option<WChars<A>> {
        self.find(key)?.value_as_u16_in(alloc)
    }
    #[inline]
    pub fn find_as_fiber_in<A: Allocator>(&'a self, key: impl Into<WCharLike<'a>>, alloc: A) -> Option<Fiber<A>> {
        self.find(key)?.value_as_fiber_in(alloc)
    }

    #[cfg(target_family = "windows")]
    pub(crate) fn new(env: &'a [u16]) -> EnvironmentBlock<'a> {
        EnvironmentBlock {
            ptr: env.as_ptr(),
            _p:  PhantomData,
        }
    }

    fn next_entry(&self, pos: usize) -> Option<(&[u16], usize)> {
        let mut n = pos;
        loop {
            match unsafe { *self.ptr.add(n) } {
                0 => {
                    return if n.saturating_sub(pos) >= 1 {
                        Some((unsafe { from_raw_parts(self.ptr.add(pos), n - pos) }, n + 1))
                    } else {
                        None
                    }
                },
                _ => (),
            }
            n += 1
        }
    }
}

impl<'a> StringLikeU16 for Variable<'a> {}
impl<'a> StringLike<u16> for Variable<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        self.0
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        self.0.as_ptr()
    }
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u16 {
        self.0.as_ptr() as *mut u16
    }
}

impl<'a> StringLikeU16 for EnvironmentBlock<'a> {}
impl<'a> StringLike<u16> for EnvironmentBlock<'a> {
    fn len(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        let (mut n, mut c) = (0usize, 0u32);
        loop {
            if unsafe { *self.ptr.add(n) } == 0 {
                c += 1;
                if c == 2 {
                    break;
                }
            } else {
                c = 0;
            }
            n += 1
        }
        n
    }
    #[inline]
    fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
    #[inline]
    fn as_slice(&self) -> &[u16] {
        if self.ptr.is_null() {
            return &[];
        }
        let n = self.len();
        if n == 0 {
            &[]
        } else {
            unsafe { from_raw_parts(self.ptr, n) }
        }
    }
    #[inline]
    fn as_ptr(&self) -> *const u16 {
        if self.is_empty() {
            null()
        } else {
            self.ptr
        }
    }
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u16 {
        if self.is_empty() {
            null_mut()
        } else {
            self.ptr as *mut u16
        }
    }
}

impl Deref for Variable<'_> {
    type Target = [u16];

    #[inline]
    fn deref(&self) -> &[u16] {
        self.0
    }
}

impl Copy for EnvironmentBlock<'_> {}
impl Deref for EnvironmentBlock<'_> {
    type Target = [u16];

    #[inline]
    fn deref(&self) -> &[u16] {
        self.as_slice()
    }
}
impl<'a> Clone for EnvironmentBlock<'a> {
    #[inline]
    fn clone(&self) -> EnvironmentBlock<'a> {
        EnvironmentBlock { ptr: self.ptr, _p: PhantomData }
    }
}

impl<'a> Iterator for EnvironmentIter<'a> {
    type Item = Variable<'a>;

    #[inline]
    fn next(&mut self) -> Option<Variable<'a>> {
        let (r, n) = self.env.next_entry(self.pos)?;
        self.pos = n;
        Some(Variable(r))
    }
}
impl FusedIterator for EnvironmentIter<'_> {}

impl From<Variable<'_>> for Fiber {
    #[inline]
    fn from(v: Variable<'_>) -> Fiber {
        v.into_fiber()
    }
}
impl From<&Variable<'_>> for Fiber {
    #[inline]
    fn from(v: &Variable<'_>) -> Fiber {
        v.to_fiber()
    }
}

impl From<Variable<'_>> for String {
    #[inline]
    fn from(v: Variable<'_>) -> String {
        v.into_string()
    }
}
impl From<&Variable<'_>> for String {
    #[inline]
    fn from(v: &Variable<'_>) -> String {
        v.to_string()
    }
}

fn match_u16(src: &[u16], v: &[u16]) -> bool {
    if src.len() != v.len() {
        return false;
    }
    for i in 0..src.len() {
        // Lengths should be the same, matched above.
        let (x, y) = unsafe { (*src.get_unchecked(i), *v.get_unchecked(i)) };
        match (x, y) {
            (0x41..=0x5A, 0x61..=0x7A) if x + 0x20 == y => (),
            (0x61..=0x7A, 0x41..=0x5A) if x == y + 0x20 => (),
            _ if x == y => (),
            _ => return false,
        }
    }
    true
}
fn match_u16_u8(src: &[u16], v: &[u8]) -> bool {
    if src.len() != v.len() {
        return false;
    }
    for i in 0..src.len() {
        // Lengths should be the same, matched above.
        let (x, y) = unsafe { (*src.get_unchecked(i) as u8, *v.get_unchecked(i)) };
        match (x, y) {
            (0x41..=0x5A, 0x61..=0x7A) if x + 0x20 == y => (),
            (0x61..=0x7A, 0x41..=0x5A) if x == y + 0x20 => (),
            _ if x == y => (),
            _ => return false,
        }
    }
    true
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    extern crate xrmt_data;

    use core::fmt::{Debug, Display, Formatter, Result};
    use core::iter::Iterator;
    use core::result::Result::Ok;

    use xrmt_data::text::SPLITTER;

    use crate::structs::{EnvironmentBlock, StringLikeU16, Variable};

    impl Debug for Variable<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_debug(f)
        }
    }
    impl Display for Variable<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            self.fmt_display(f)
        }
    }

    impl Debug for EnvironmentBlock<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            for (i, v) in self.iter().enumerate() {
                if i > 0 {
                    f.write_str(SPLITTER)?;
                }
                Display::fmt(&v, f)?;
            }
            Ok(())
        }
    }
    impl Display for EnvironmentBlock<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(self, f)
        }
    }
}
