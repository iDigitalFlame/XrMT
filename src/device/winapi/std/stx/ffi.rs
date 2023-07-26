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
#![cfg(not(feature = "std"))]

extern crate alloc;
extern crate core;

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::collections::TryReserveError;
use alloc::string::{String, ToString};
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From};
use core::default::Default;
use core::fmt::{self, Write};
use core::hash::{Hash, Hasher};
use core::iter::{Extend, FromIterator, IntoIterator, Iterator};
use core::marker::Sized;
use core::mem;
use core::ops::{Deref, DerefMut, Index, IndexMut, RangeFull};
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Ok};

#[repr(transparent)]
pub struct Path {
    inner: [u8],
}
#[repr(transparent)]
pub struct OsStr {
    inner: [u8],
}
#[repr(transparent)]
pub struct PathBuf {
    inner: String,
}
#[repr(transparent)]
pub struct OsString {
    inner: String,
}

impl Path {
    #[inline]
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        unsafe { mem::transmute(self) }
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
        unsafe { mem::transmute(self) }
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
impl OsStr {
    #[inline]
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &OsStr {
        s.as_ref()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        core::str::from_utf8(&self.inner).ok()
    }
    #[inline]
    pub fn to_os_string(&self) -> OsString {
        OsString {
            inner: self.to_string_lossy().to_string(),
        }
    }
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
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
        unsafe { mem::transmute(self) }
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
        unsafe { mem::transmute(self) }
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
impl OsString {
    #[inline]
    pub fn new() -> OsString {
        OsString { inner: String::new() }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> OsString {
        OsString {
            inner: String::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        self
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
    pub fn push<T: AsRef<OsStr>>(&mut self, s: T) {
        unsafe { self.inner.as_mut_vec().extend_from_slice(&s.as_ref().inner) }
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
    pub fn into_string(self) -> Result<String, OsString> {
        Ok(self.inner)
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
impl AsRef<OsStr> for str {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        unsafe { mem::transmute(self.as_bytes()) }
    }
}
impl PartialEq<OsStr> for str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        other.inner == *self.as_bytes()
    }
}
impl PartialEq<OsString> for str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        other.inner.as_str() == self
    }
}
impl<'a> PartialEq<OsString> for &'a str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        other.inner.as_str() == *self
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
    fn from(s: PathBuf) -> Cow<'a, Path> {
        Cow::Owned(s)
    }
}
impl<'a> From<&'a Path> for Cow<'a, Path> {
    #[inline]
    fn from(s: &'a Path) -> Cow<'a, Path> {
        Cow::Borrowed(s)
    }
}
impl<'a> From<OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(s: OsString) -> Cow<'a, OsStr> {
        Cow::Owned(s)
    }
}
impl<'a> From<&'a OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn from(s: &'a OsStr) -> Cow<'a, OsStr> {
        Cow::Borrowed(s)
    }
}
impl<'a> From<&'a PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(p: &'a PathBuf) -> Cow<'a, Path> {
        Cow::Borrowed(p.as_path())
    }
}
impl<'a> From<&'a OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(s: &'a OsString) -> Cow<'a, OsStr> {
        Cow::Borrowed(s.as_os_str())
    }
}

impl AsRef<Path> for String {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<OsStr> for String {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        (&**self).as_ref()
    }
}

impl Eq for OsString {}
impl Ord for OsString {
    #[inline]
    fn cmp(&self, other: &OsString) -> Ordering {
        (&**self).cmp(&**other)
    }
}
impl Hash for OsString {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (&**self).hash(state)
    }
}
impl Deref for OsString {
    type Target = OsStr;

    #[inline]
    fn deref(&self) -> &OsStr {
        &self[..]
    }
}
impl Write for OsString {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push(s);
        Ok(())
    }
}
impl Clone for OsString {
    #[inline]
    fn clone(&self) -> OsString {
        OsString {
            inner: self.inner.clone(),
        }
    }
    #[inline]
    fn clone_from(&mut self, source: &OsString) {
        self.inner.clone_from(&source.inner)
    }
}
impl Default for OsString {
    #[inline]
    fn default() -> OsString {
        OsString::new()
    }
}
impl DerefMut for OsString {
    #[inline]
    fn deref_mut(&mut self) -> &mut OsStr {
        &mut self[..]
    }
}
impl PartialEq for OsString {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        &**self == &**other
    }
}
impl PartialOrd for OsString {
    #[inline]
    fn lt(&self, other: &OsString) -> bool {
        &**self < &**other
    }
    #[inline]
    fn le(&self, other: &OsString) -> bool {
        &**self <= &**other
    }
    #[inline]
    fn gt(&self, other: &OsString) -> bool {
        &**self > &**other
    }
    #[inline]
    fn ge(&self, other: &OsString) -> bool {
        &**self >= &**other
    }
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<Ordering> {
        (&**self).partial_cmp(&**other)
    }
}
impl AsRef<Path> for OsString {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<OsStr> for OsString {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}
impl From<String> for OsString {
    #[inline]
    fn from(v: String) -> OsString {
        OsString { inner: v }
    }
}
impl From<PathBuf> for OsString {
    #[inline]
    fn from(v: PathBuf) -> OsString {
        OsString { inner: v.inner }
    }
}
impl Borrow<OsStr> for OsString {
    #[inline]
    fn borrow(&self) -> &OsStr {
        unsafe { mem::transmute(self.inner.as_bytes()) }
    }
}
impl PartialEq<str> for OsString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.inner.as_str() == other
    }
}
impl PartialEq<&str> for OsString {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.inner.as_str() == *other
    }
}
impl PartialOrd<str> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.inner.as_str().partial_cmp(other)
    }
}
impl Extend<OsString> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = OsString>>(&mut self, iter: T) {
        for s in iter {
            self.push(&s);
        }
    }
}
impl Index<RangeFull> for OsString {
    type Output = OsStr;

    #[inline]
    fn index(&self, _index: RangeFull) -> &OsStr {
        unsafe { mem::transmute(self.inner.as_bytes()) }
    }
}
impl IndexMut<RangeFull> for OsString {
    #[inline]
    fn index_mut(&mut self, _index: RangeFull) -> &mut OsStr {
        unsafe { mem::transmute(self.inner.as_bytes_mut()) }
    }
}
impl<'a> Extend<&'a OsStr> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a OsStr>>(&mut self, iter: T) {
        for s in iter {
            self.push(s);
        }
    }
}
impl<'a> From<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn from(s: Cow<'a, OsStr>) -> OsString {
        s.into_owned()
    }
}
impl<T: ?Sized + AsRef<OsStr>> From<&T> for OsString {
    #[inline]
    fn from(s: &T) -> OsString {
        s.as_ref().to_os_string()
    }
}

impl Eq for OsStr {}
impl Ord for OsStr {
    #[inline]
    fn cmp(&self, other: &OsStr) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}
impl Hash for OsStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
impl Deref for OsStr {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.inner
    }
}
impl ToOwned for OsStr {
    type Owned = OsString;

    #[inline]
    fn to_owned(&self) -> OsString {
        self.to_os_string()
    }
    #[inline]
    fn clone_into(&self, target: &mut OsString) {
        unsafe { self.inner.clone_into(&mut target.inner.as_mut_vec()) }
    }
}
impl Default for &OsStr {
    #[inline]
    fn default() -> Self {
        OsStr::new("")
    }
}
impl PartialEq for OsStr {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.inner == other.inner
    }
}
impl PartialOrd for OsStr {
    #[inline]
    fn lt(&self, other: &OsStr) -> bool {
        self.inner.lt(&other.inner)
    }
    #[inline]
    fn le(&self, other: &OsStr) -> bool {
        self.inner.le(&other.inner)
    }
    #[inline]
    fn gt(&self, other: &OsStr) -> bool {
        self.inner.gt(&other.inner)
    }
    #[inline]
    fn ge(&self, other: &OsStr) -> bool {
        self.inner.ge(&other.inner)
    }
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}
impl AsRef<Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}
impl AsRef<OsStr> for OsStr {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}
impl PartialEq<str> for OsStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.inner == *other.as_bytes()
    }
}
impl PartialOrd<str> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.inner.partial_cmp(other.as_bytes())
    }
}
impl FromIterator<OsString> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = OsString>>(iter: I) -> OsString {
        let mut i = iter.into_iter();
        match i.next() {
            None => OsString::new(),
            Some(mut b) => {
                b.extend(i);
                b
            },
        }
    }
}
impl<'a> FromIterator<&'a OsStr> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a OsStr>>(iter: I) -> OsString {
        let mut b = OsString::new();
        for s in iter {
            b.push(s);
        }
        b
    }
}

impl Eq for Path {}
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
        unsafe { mem::transmute(self) }
    }
}

impl Eq for PathBuf {}
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
        PathBuf {
            inner: self.inner.clone(),
        }
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
        unsafe { mem::transmute(self.inner.as_bytes_mut()) }
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
        unsafe { mem::transmute(self.inner.as_bytes()) }
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
    fn from(p: Cow<'a, Path>) -> PathBuf {
        p.into_owned()
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
    fn from(s: &T) -> PathBuf {
        PathBuf::from(s.as_ref().to_os_string())
    }
}
