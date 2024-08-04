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

//
// Module assistance with help from the Rust Team std/io code!
//

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::collections::TryReserveError;
use alloc::string::{String, ToString};
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From};
use core::default::Default;
use core::fmt::{self, Debug, Formatter};
use core::iter::{Extend, FromIterator, IntoIterator, Iterator};
use core::marker::Sized;
use core::mem::transmute;
use core::ops::{Deref, DerefMut};
use core::option::Option;
use core::result::Result;

use crate::ffi::{OsStr, OsString};

#[repr(transparent)]
pub struct Path {
    pub(super) inner: [u8],
}
#[repr(transparent)]
pub struct PathBuf {
    pub(super) inner: String,
}

impl Path {
    #[inline]
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        unsafe { transmute(self) }
    }
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.inner).ok()
    }
    #[inline(never)]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf {
            inner: self.to_string_lossy().to_string(),
        }
    }
    #[inline]
    pub fn as_mut_os_str(&mut self) -> &mut OsStr {
        unsafe { transmute(self) }
    }
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }
    #[inline]
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        let mut b = self.to_path_buf();
        b.push(path.as_ref());
        b
    }
}
impl PathBuf {
    #[inline]
    pub fn new() -> PathBuf {
        PathBuf { inner: String::new() }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> PathBuf {
        PathBuf {
            inner: String::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }
    #[inline]
    pub fn as_path(&self) -> &Path {
        self
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }
    #[inline]
    pub fn into_os_string(self) -> OsString {
        unsafe { transmute(self) }
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.inner)
    }
    #[inline]
    pub fn push(&mut self, path: impl AsRef<Path>) {
        let v = path.as_ref();
        if !self.inner.as_bytes().last().map_or(false, |v| *v == b'\\' || *v == b'/') && !v.inner.first().map_or(false, |v| *v == b'\\' || *v == b'/') {
            self.inner.push('\\');
        }
        unsafe { self.inner.as_mut_vec().extend_from_slice(&v.inner) }
    }
    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }
    #[inline]
    pub fn as_mut_os_string(&mut self) -> &mut OsString {
        unsafe { transmute(self) }
    }
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }
    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }
}

impl AsRef<Path> for str {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for Cow<'_, OsStr> {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl<'a> From<PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(v: PathBuf) -> Cow<'a, Path> {
        Cow::Owned(v)
    }
}
impl<'a> From<&'a Path> for Cow<'a, Path> {
    #[inline]
    fn from(v: &'a Path) -> Cow<'a, Path> {
        Cow::Borrowed(v)
    }
}
impl<'a> From<&'a PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(v: &'a PathBuf) -> Cow<'a, Path> {
        Cow::Borrowed(v.as_path())
    }
}

impl AsRef<Path> for String {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl Eq for Path {}
impl Debug for Path {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, formatter)
    }
}
impl ToOwned for Path {
    type Owned = PathBuf;
    #[inline]
    fn to_owned(&self) -> PathBuf {
        self.to_path_buf()
    }
    #[inline]
    fn clone_into(&self, target: &mut PathBuf) {
        unsafe { self.inner.clone_into(&mut target.inner.as_mut_vec()) }
    }
}
impl PartialEq for Path {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.inner == other.inner
    }
}
impl AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}
impl AsRef<OsStr> for Path {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        unsafe { transmute(self) }
    }
}

impl Eq for PathBuf {}
impl Debug for PathBuf {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&**self, formatter)
    }
}
impl Deref for PathBuf {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Path {
        Path::new(&self.inner)
    }
}
impl Clone for PathBuf {
    #[inline]
    fn clone(&self) -> PathBuf {
        PathBuf { inner: self.inner.clone() }
    }
    #[inline]
    fn clone_from(&mut self, source: &PathBuf) {
        self.inner.clone_from(&source.inner)
    }
}
impl Default for PathBuf {
    #[inline]
    fn default() -> PathBuf {
        PathBuf::new()
    }
}
impl DerefMut for PathBuf {
    #[inline]
    fn deref_mut(&mut self) -> &mut Path {
        unsafe { transmute(self.inner.as_bytes_mut()) }
    }
}
impl PartialEq for PathBuf {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.inner == other.inner
    }
}
impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}
impl From<String> for PathBuf {
    #[inline]
    fn from(v: String) -> PathBuf {
        PathBuf { inner: v }
    }
}
impl Borrow<Path> for PathBuf {
    #[inline]
    fn borrow(&self) -> &Path {
        self.deref()
    }
}
impl AsRef<OsStr> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        unsafe { transmute(self.inner.as_bytes()) }
    }
}
impl From<OsString> for PathBuf {
    #[inline]
    fn from(v: OsString) -> PathBuf {
        PathBuf { inner: v.inner }
    }
}
impl<'a> From<Cow<'a, Path>> for PathBuf {
    #[inline]
    fn from(v: Cow<'a, Path>) -> PathBuf {
        v.into_owned()
    }
}
impl<P: AsRef<Path>> Extend<P> for PathBuf {
    #[inline]
    fn extend<I: IntoIterator<Item = P>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |p| self.push(p.as_ref()));
    }
}
impl<P: AsRef<Path>> FromIterator<P> for PathBuf {
    #[inline]
    fn from_iter<I: IntoIterator<Item = P>>(iter: I) -> PathBuf {
        let mut b = PathBuf::new();
        b.extend(iter);
        b
    }
}
impl<T: ?Sized + AsRef<OsStr>> From<&T> for PathBuf {
    #[inline]
    fn from(v: &T) -> PathBuf {
        PathBuf::from(v.as_ref().to_os_string())
    }
}
