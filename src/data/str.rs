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

use alloc::alloc::Global;
use alloc::collections::TryReserveError;
use core::alloc::Allocator;
use core::cmp::Ordering;
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter, Write};
use core::ops::{Add, AddAssign, Deref, DerefMut, Index, IndexMut};
use core::ptr;
use core::slice::SliceIndex;
use core::str::{from_utf8, from_utf8_unchecked, from_utf8_unchecked_mut, Utf8Error};

use crate::ffi::{OsStr, OsString};
use crate::path::{Path, PathBuf};
use crate::prelude::*;

pub struct Fiber<A: Allocator = Global> {
    vec: Vec<u8, A>,
}
pub struct FromUtf8Error<A: Allocator = Global> {
    bytes: Vec<u8, A>,
    error: Utf8Error,
}

pub trait MaybeString {
    fn into_string(&self) -> Option<&str>;
}
pub trait ToFiber<A: Allocator = Global> {
    fn to_fiber(&self) -> Fiber<A>;
}

impl Fiber {
    #[inline]
    pub const fn new() -> Fiber {
        Fiber { vec: Vec::new() }
    }

    #[inline]
    pub fn from_str(v: &str) -> Fiber {
        Fiber { vec: v.as_bytes().to_vec() }
    }
    #[inline]
    pub fn from_utf8_lossy(v: &[u8]) -> Fiber {
        Fiber { vec: v.to_vec() }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> Fiber {
        Fiber {
            vec: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn convert_vec(vec: Vec<impl AsRef<str>>) -> Vec<Fiber> {
        if vec.is_empty() {
            return Vec::new();
        }
        let mut r = Vec::with_capacity(vec.len());
        for i in vec {
            r.push(Fiber::from_str(i.as_ref()));
        }
        r
    }
}
impl<A: Allocator> Fiber<A> {
    #[inline]
    pub const fn new_in(alloc: A) -> Fiber<A> {
        Fiber { vec: Vec::new_in(alloc) }
    }

    #[inline]
    pub fn from_str_in(v: &str, alloc: A) -> Fiber<A> {
        Fiber {
            vec: v.as_bytes().to_vec_in(alloc),
        }
    }
    #[inline]
    pub fn from_utf8_lossy_in(v: &[u8], alloc: A) -> Fiber<A> {
        Fiber { vec: v.to_vec_in(alloc) }
    }
    #[inline]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Fiber<A> {
        Fiber {
            vec: Vec::with_capacity_in(capacity, alloc),
        }
    }
    #[inline]
    pub fn from_utf8(vec: Vec<u8, A>) -> Result<Fiber<A>, FromUtf8Error<A>> {
        match from_utf8(&vec) {
            Ok(..) => Ok(Fiber { vec }),
            Err(e) => Err(FromUtf8Error { bytes: vec, error: e }),
        }
    }

    #[inline]
    pub unsafe fn swap<B: Allocator>(dst: &mut Fiber<A>, src: &mut Fiber<B>) {
        let (x, y) = (src.vec.len(), dst.vec.len());
        ptr::swap(src.vec.as_mut_ptr(), dst.vec.as_mut_ptr());
        src.vec.set_len(y);
        dst.vec.set_len(x);
    }
    #[inline]
    pub unsafe fn from_utf8_unchecked(bytes: Vec<u8, A>) -> Fiber<A> {
        Fiber { vec: bytes }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        self
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.vec
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.vec.shrink_to_fit()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.vec.capacity()
    }
    #[inline]
    pub fn push(&mut self, ch: char) {
        match ch.len_utf8() {
            1 => self.vec.push(ch as u8),
            _ => self.vec.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    #[inline]
    pub fn into_bytes(self) -> Vec<u8, A> {
        self.vec
    }
    #[inline]
    pub fn pop(&mut self) -> Option<char> {
        let c = self.chars().rev().next()?;
        let n = self.len() - c.len_utf8();
        unsafe { self.vec.set_len(n) };
        Some(c)
    }
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        self.vec.extend_from_slice(string.as_bytes())
    }
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        self
    }
    #[inline]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len <= self.len() {
            self.vec.truncate(new_len)
        }
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.vec.reserve(additional)
    }
    #[inline]
    pub fn insert(&mut self, idx: usize, ch: char) {
        let mut b = [0; 4];
        let b = ch.encode_utf8(&mut b).as_bytes();
        unsafe { self.insert_bytes(idx, b) }
    }
    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.vec.shrink_to(min_capacity)
    }
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.vec.reserve_exact(additional)
    }

    #[inline]
    pub fn insert_str(&mut self, idx: usize, string: &str) {
        unsafe { self.insert_bytes(idx, string.as_bytes()) }
    }
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.vec.try_reserve(additional)
    }
    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.vec.try_reserve_exact(additional)
    }

    #[inline]
    pub unsafe fn as_mut_vec(&mut self) -> &mut Vec<u8, A> {
        &mut self.vec
    }

    unsafe fn insert_bytes(&mut self, idx: usize, bytes: &[u8]) {
        let n = self.len();
        let c = bytes.len();
        self.vec.reserve(c);
        unsafe {
            ptr::copy(
                self.vec.as_ptr().add(idx),
                self.vec.as_mut_ptr().add(idx + c),
                n - idx,
            );
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.vec.as_mut_ptr().add(idx), c);
            self.vec.set_len(n + c);
        }
    }
}
impl<A: Allocator + Clone> Fiber<A> {
    #[inline]
    pub fn allocator(&self) -> A {
        self.vec.allocator().clone()
    }
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Fiber<A> {
        unsafe { Fiber::from_utf8_unchecked(self.vec.split_off(at)) }
    }

    #[inline]
    pub fn convert_vec_in<B: Allocator>(vec: Vec<impl AsRef<str>, B>, alloc: A) -> Vec<Fiber<A>, A> {
        if vec.is_empty() {
            return Vec::new_in(alloc);
        }
        let mut r = Vec::with_capacity_in(vec.len(), alloc.clone());
        for i in vec {
            r.push(i.as_ref().into_alloc(alloc.clone()))
        }
        r
    }
}
impl<A: Allocator> FromUtf8Error<A> {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
    #[inline]
    pub fn into_bytes(self) -> Vec<u8, A> {
        self.bytes
    }
    #[inline]
    pub fn utf8_error(&self) -> Utf8Error {
        self.error
    }
}

impl Default for Fiber {
    #[inline]
    fn default() -> Fiber {
        Fiber::new()
    }
}
impl From<&str> for Fiber {
    #[inline]
    fn from(v: &str) -> Fiber {
        Fiber::from_utf8_lossy(v.as_bytes())
    }
}
impl From<char> for Fiber {
    #[inline]
    fn from(v: char) -> Fiber {
        Fiber::from_utf8_lossy(v.encode_utf8(&mut [0; 4]).as_bytes())
    }
}

impl From<String> for Fiber {
    #[inline]
    fn from(v: String) -> Fiber {
        Fiber::from_utf8_lossy(v.as_bytes())
    }
}
impl From<&String> for Fiber {
    #[inline]
    fn from(v: &String) -> Fiber {
        Fiber::from_utf8_lossy(v.as_bytes())
    }
}
impl From<PathBuf> for Fiber {
    #[inline]
    fn from(v: PathBuf) -> Fiber {
        Fiber {
            vec: v.to_string_lossy().as_bytes().to_vec(),
        }
    }
}
impl From<OsString> for Fiber {
    #[inline]
    fn from(v: OsString) -> Fiber {
        Fiber {
            vec: v.to_string_lossy().as_bytes().to_vec(),
        }
    }
}
impl From<&mut str> for Fiber {
    #[inline]
    fn from(v: &mut str) -> Fiber {
        Fiber::from_utf8_lossy(v.as_bytes())
    }
}

impl MaybeString for &str {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for String {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}
impl MaybeString for Option<&str> {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        *self
    }
}
impl MaybeString for alloc::borrow::Cow<'_, str> {
    #[inline]
    fn into_string(&self) -> Option<&str> {
        Some(self)
    }
}

impl<T: AsRef<[u8]>> ToFiber for T {
    #[inline]
    fn to_fiber(&self) -> Fiber {
        Fiber::from_utf8_lossy(self.as_ref())
    }
}
impl<A: Allocator> Eq for Fiber<A> {}
impl<A: Allocator> Ord for Fiber<A> {
    #[inline]
    fn cmp(&self, other: &Fiber<A>) -> Ordering {
        self.vec.cmp(&other.vec)
    }
}
impl<A: Allocator> Deref for Fiber<A> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        unsafe { from_utf8_unchecked(&self.vec) }
    }
}
impl<A: Allocator> Write for Fiber<A> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}
impl<A: Allocator> Debug for Fiber<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl<A: Allocator> Display for Fiber<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}
impl<A: Allocator> DerefMut for Fiber<A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut str {
        unsafe { from_utf8_unchecked_mut(&mut *self.vec) }
    }
}
impl<A: Allocator> Add<&str> for Fiber<A> {
    type Output = Fiber<A>;

    #[inline]
    fn add(mut self, other: &str) -> Fiber<A> {
        self.push_str(other);
        self
    }
}
impl<A: Allocator> PartialEq for Fiber<A> {
    #[inline]
    fn eq(&self, other: &Fiber<A>) -> bool {
        self.vec.eq(&other.vec)
    }
}
impl<A: Allocator> PartialOrd for Fiber<A> {
    #[inline]
    fn partial_cmp(&self, other: &Fiber<A>) -> Option<Ordering> {
        self.vec.partial_cmp(&other.vec)
    }
}
impl<A: Allocator> AsRef<str> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &str {
        self
    }
}
impl<A: Allocator> AsMut<str> for Fiber<A> {
    #[inline]
    fn as_mut(&mut self) -> &mut str {
        self
    }
}
impl<A: Allocator> Extend<u8> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = u8>>(&mut self, iter: I) {
        let x = iter.into_iter();
        if let Some(n) = x.size_hint().1 {
            self.reserve(n);
        }
        for i in x {
            self.vec.push(i)
        }
    }
}
impl<A: Allocator> AsRef<[u8]> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl<A: Allocator> AsRef<OsStr> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        (&**self).as_ref()
    }
}
impl<A: Allocator> AsRef<Path> for Fiber<A> {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl<A: Allocator> Extend<char> for Fiber<A> {
    #[inline]
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let x = iter.into_iter();
        if let Some(n) = x.size_hint().1 {
            self.reserve(n);
        }
        for i in x {
            self.push(i)
        }
    }
}
impl<A: Allocator> Into<String> for Fiber<A> {
    #[inline]
    fn into(self) -> String {
        // Zero allocation, just transfer the vec.
        let (a, b, c) = self.vec.into_raw_parts();
        unsafe { String::from_raw_parts(a, b, c) }
    }
}
impl<A: Allocator + Clone> Clone for Fiber<A> {
    #[inline]
    fn clone(&self) -> Fiber<A> {
        Fiber { vec: self.vec.clone() }
    }
    #[inline]
    fn clone_from(&mut self, source: &Fiber<A>) {
        self.vec.clone_from(&source.vec);
    }
}
impl<A: Allocator> AddAssign<&str> for Fiber<A> {
    #[inline]
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}
impl<A: Allocator> From<Fiber<A>> for Vec<u8, A> {
    #[inline]
    fn from(v: Fiber<A>) -> Vec<u8, A> {
        v.into_bytes()
    }
}
impl<A: Allocator> From<Vec<u8, A>> for Fiber<A> {
    #[inline]
    fn from(v: Vec<u8, A>) -> Fiber<A> {
        Fiber { vec: v }
    }
}
impl<A: Allocator + Clone> From<&Fiber<A>> for Fiber<A> {
    #[inline]
    fn from(v: &Fiber<A>) -> Fiber<A> {
        v.clone()
    }
}
impl<A: Allocator, I: SliceIndex<str>> Index<I> for Fiber<A> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        index.index(self.as_str())
    }
}
impl<A: Allocator, I: SliceIndex<str>> IndexMut<I> for Fiber<A> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self.as_mut_str())
    }
}

impl<A: Allocator> Debug for FromUtf8Error<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, f)
    }
}
impl<A: Allocator> Error for FromUtf8Error<A> {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<A: Allocator> Display for FromUtf8Error<A> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, f)
    }
}
impl<A: Allocator + Clone> Clone for FromUtf8Error<A> {
    #[inline]
    fn clone(&self) -> FromUtf8Error<A> {
        FromUtf8Error {
            bytes: self.bytes.clone(),
            error: self.error,
        }
    }
}

impl<A: Allocator> AllocFrom<u8, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: u8, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(v);
        r
    }
}
impl<A: Allocator> AllocFrom<&str, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &str, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(v, alloc)
    }
}
impl<A: Allocator> AllocFrom<char, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: char, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(v.len_utf8(), alloc);
        r.push(v);
        r
    }
}
impl<A: Allocator> AllocFrom<&[u8], A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &[u8], alloc: A) -> Fiber<A> {
        Fiber::from_utf8_lossy_in(v, alloc)
    }
}
impl<A: Allocator> AllocFrom<String, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: String, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&v, alloc)
    }
}
impl<A: Allocator> AllocFrom<&String, A> for Fiber<A> {
    #[inline]
    fn from_alloc(v: &String, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&v, alloc)
    }
}

impl<A: Allocator> AllocInto<Fiber<A>, A> for u8 {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(1, alloc);
        r.vec.push(self);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &str {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for char {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        let mut r = Fiber::with_capacity_in(self.len_utf8(), alloc);
        r.push(self);
        r
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &[u8] {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_utf8_lossy_in(self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for String {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(&self, alloc)
    }
}
impl<A: Allocator> AllocInto<Fiber<A>, A> for &String {
    #[inline]
    fn into_alloc(self, alloc: A) -> Fiber<A> {
        Fiber::from_str_in(self, alloc)
    }
}

impl<A: Allocator> Eq for FromUtf8Error<A> {}
impl<A: Allocator> PartialEq for FromUtf8Error<A> {
    #[inline]
    fn eq(&self, other: &FromUtf8Error<A>) -> bool {
        self.bytes.eq(&other.bytes) && self.error.eq(&other.error)
    }
}
