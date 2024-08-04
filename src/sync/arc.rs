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

use alloc::sync::{Arc, Weak};
use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::cmp::Ordering;
use core::fmt::{self, Debug, Display, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::prelude::*;

pub struct ArcMut<T>(Arc<UnsafeCell<T>>);
pub struct WeakMut<T>(Weak<UnsafeCell<T>>);

impl<T> ArcMut<T> {
    #[inline]
    pub fn new(data: T) -> ArcMut<T> {
        ArcMut(Arc::new(UnsafeCell::new(data)))
    }

    #[inline]
    pub fn as_mut(this: &ArcMut<T>) -> &mut T {
        unsafe { &mut *this.0.get() }
    }
    #[inline]
    pub fn weak_count(this: &ArcMut<T>) -> usize {
        Arc::weak_count(&this.0)
    }
    #[inline]
    pub fn strong_count(this: &ArcMut<T>) -> usize {
        Arc::strong_count(&this.0)
    }
    #[inline]
    pub fn into_inner(this: ArcMut<T>) -> Option<T> {
        Arc::into_inner(this.0).map(|v| v.into_inner())
    }
    #[inline]
    pub fn downgrade(this: &ArcMut<T>) -> WeakMut<T> {
        WeakMut(Arc::downgrade(&this.0))
    }
    #[inline]
    pub fn try_unwrap(this: ArcMut<T>) -> Result<T, ArcMut<T>> {
        Arc::try_unwrap(this.0).map_or_else(|v| Err(ArcMut(v)), |v| Ok(v.into_inner()))
    }
    #[inline]
    pub fn ptr_eq(this: &ArcMut<T>, other: &ArcMut<T>) -> bool {
        Arc::ptr_eq(&this.0, &other.0)
    }
}
impl<T> WeakMut<T> {
    #[inline]
    pub fn weak_count(&self) -> usize {
        self.0.weak_count()
    }
    #[inline]
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
    #[inline]
    pub fn upgrade(&self) -> Option<ArcMut<T>> {
        self.0.upgrade().map(ArcMut)
    }
    #[inline]
    pub fn ptr_eq(&self, other: &WeakMut<T>) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

impl<T> Deref for ArcMut<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.0.get() }
    }
}
impl<T> Clone for ArcMut<T> {
    #[inline]
    fn clone(&self) -> ArcMut<T> {
        ArcMut(self.0.clone())
    }
}
impl<T> Unpin for ArcMut<T> {}
impl<T: Eq> Eq for ArcMut<T> {}
impl<T> AsRef<T> for ArcMut<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        unsafe { &*self.0.get() }
    }
}
impl<T> DerefMut for ArcMut<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}
impl<T: Ord> Ord for ArcMut<T> {
    #[inline]
    fn cmp(&self, other: &ArcMut<T>) -> Ordering {
        unsafe { (&*self.0.get()).cmp(&*other.0.get()) }
    }
}
impl<T> Borrow<T> for ArcMut<T> {
    #[inline]
    fn borrow(&self) -> &T {
        unsafe { &*self.0.get() }
    }
}
impl<T: Hash> Hash for ArcMut<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { &*self.0.get() }.hash(state);
    }
}
impl<T: Debug> Debug for ArcMut<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: Default> Default for ArcMut<T> {
    #[inline]
    fn default() -> ArcMut<T> {
        ArcMut::new(Default::default())
    }
}
impl<T: Display> Display for ArcMut<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: RefUnwindSafe> UnwindSafe for ArcMut<T> {}
impl<T: PartialEq> PartialEq<ArcMut<T>> for ArcMut<T> {
    #[inline]
    fn eq(&self, other: &ArcMut<T>) -> bool {
        unsafe { &*self.0.get() == &*other.0.get() }
    }
}
impl<T: PartialOrd> PartialOrd<ArcMut<T>> for ArcMut<T> {
    #[inline]
    fn partial_cmp(&self, other: &ArcMut<T>) -> Option<Ordering> {
        unsafe { (&*self.0.get()).partial_cmp(&*other.0.get()) }
    }
}

unsafe impl<T> Send for ArcMut<T> {}
unsafe impl<T> Sync for ArcMut<T> {}

unsafe impl<T> Send for WeakMut<T> {}
unsafe impl<T> Sync for WeakMut<T> {}
