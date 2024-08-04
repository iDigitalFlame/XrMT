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
#![cfg(target_family = "windows")]

use core::cell::UnsafeCell;
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter, Write};
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::prelude::*;
use crate::sync::Mutant;

pub enum TryLockError<T> {
    Poisoned(PoisonError<T>),
    WouldBlock,
}

pub struct PoisonError<T> {
    value: T,
}
pub struct Mutex<T: ?Sized> {
    inner: Mutant,
    data:  UnsafeCell<T>,
}
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    inner: &'a Mutex<T>,
}

pub type LockResult<T> = Result<T, PoisonError<T>>;
pub type TryLockResult<T> = Result<T, TryLockError<T>>;

impl<T> Mutex<T> {
    #[inline]
    pub const fn new(t: T) -> Mutex<T> {
        Mutex {
            data:  UnsafeCell::new(t),
            inner: Mutant::new(),
        }
    }
}
impl<T> PoisonError<T> {
    #[inline]
    pub fn new(guard: T) -> PoisonError<T> {
        PoisonError { value: guard }
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.value
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T: Sized> Mutex<T> {
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        Ok(self.data.into_inner())
    }
}
impl<T: ?Sized> Mutex<T> {
    #[inline]
    pub fn unlock(guard: MutexGuard<'_, T>) {
        drop(guard);
    }

    #[inline(always)]
    pub fn clear_poison(&self) {}
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        false
    }
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Ok(self.data.get_mut())
    }
    #[inline]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.inner.lock().map_or_else(
            |_| Err(PoisonError::new(MutexGuard { inner: self })),
            |_| Ok(MutexGuard { inner: self }),
        )
    }
    #[inline]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        self.inner.try_lock().map_or_else(
            |_| {
                Err(TryLockError::Poisoned(PoisonError::new(MutexGuard {
                    inner: self,
                })))
            },
            |r| {
                if r {
                    Ok(MutexGuard { inner: self })
                } else {
                    Err(TryLockError::WouldBlock)
                }
            },
        )
    }
}

impl<T> From<T> for Mutex<T> {
    #[inline]
    fn from(v: T) -> Mutex<T> {
        Mutex::new(v)
    }
}
impl<T: ?Sized + Debug> Debug for Mutex<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(unsafe { &*self.data.get() }, f)
    }
}
impl<T: ?Sized + Default> Default for Mutex<T> {
    #[inline]
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

impl<T> Error for PoisonError<T> {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T> Debug for PoisonError<T> {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}
impl<T> Display for PoisonError<T> {
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // NOTE(dij): We panic here as this shouldn't happen.
        unwrap_unlikely(self.inner.inner.unlock())
    }
}
impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.inner.data.get() }
    }
}
impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.inner.data.get() }
    }
}
impl<T: ?Sized + Debug> Debug for MutexGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(unsafe { &*self.inner.data.get() }, f)
    }
}
impl<T: ?Sized + Display> Display for MutexGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(unsafe { &*self.inner.data.get() }, f)
    }
}

impl<T> Error for TryLockError<T> {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T> Debug for TryLockError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl<T> Display for TryLockError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TryLockError::WouldBlock => f.write_char('1'),
            TryLockError::Poisoned(_) => f.write_char('0'),
        }
    }
}
impl<T> From<PoisonError<T>> for TryLockError<T> {
    #[inline]
    fn from(v: PoisonError<T>) -> TryLockError<T> {
        TryLockError::Poisoned(v)
    }
}

impl<T: ?Sized> UnwindSafe for Mutex<T> {}
impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for MutexGuard<'_, T> {}
