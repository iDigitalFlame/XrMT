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
use alloc::vec::Vec;
use core::clone::Clone;
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::{AsRef, From};
use core::default::Default;
use core::fmt::{self, Write};
use core::hash::{Hash, Hasher};
use core::iter::{Extend, FromIterator, IntoIterator, Iterator};
use core::marker::Sized;
use core::mem::transmute;
use core::ops::{Deref, DerefMut, Index, IndexMut, RangeFull};
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Ok};

#[repr(transparent)]
pub struct OsStr {
    pub(crate) inner: [u8],
}
#[repr(transparent)]
pub struct OsString {
    pub(crate) inner: String,
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
    pub fn as_encoded_bytes(&self) -> &[u8] {
        &self
    }
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
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
    pub fn into_encoded_bytes(self) -> Vec<u8> {
        self.to_vec()
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

impl AsRef<OsStr> for str {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        unsafe { transmute(self.as_bytes()) }
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

impl<'a> From<OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: OsString) -> Cow<'a, OsStr> {
        Cow::Owned(v)
    }
}
impl<'a> From<&'a OsStr> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: &'a OsStr) -> Cow<'a, OsStr> {
        Cow::Borrowed(v)
    }
}
impl<'a> From<&'a OsString> for Cow<'a, OsStr> {
    #[inline]
    fn from(v: &'a OsString) -> Cow<'a, OsStr> {
        Cow::Borrowed(v.as_os_str())
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
        OsString { inner: self.inner.clone() }
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
impl Borrow<OsStr> for OsString {
    #[inline]
    fn borrow(&self) -> &OsStr {
        unsafe { transmute(self.inner.as_bytes()) }
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
        unsafe { transmute(self.inner.as_bytes()) }
    }
}
impl IndexMut<RangeFull> for OsString {
    #[inline]
    fn index_mut(&mut self, _index: RangeFull) -> &mut OsStr {
        unsafe { transmute(self.inner.as_bytes_mut()) }
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
    fn from(v: Cow<'a, OsStr>) -> OsString {
        v.into_owned()
    }
}
impl<T: ?Sized + AsRef<OsStr>> From<&T> for OsString {
    #[inline]
    fn from(v: &T) -> OsString {
        v.as_ref().to_os_string()
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
    fn default<'a>() -> Self {
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
