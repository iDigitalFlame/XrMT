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

extern crate core;

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::convert::AsRef;
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::hint::spin_loop;
use core::marker::{PhantomData, Send, Sized, Sync};
use core::mem::{drop, replace, MaybeUninit};
use core::ops::{Deref, Drop, FnOnce};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr::{read, read_volatile};
use core::result::Result::{self, Err, Ok};
use core::sync::atomic::{fence, AtomicIsize, AtomicU8, Ordering};
use core::{debug_assert, unreachable};

use crate::io::FmtResult;

const STATE_NEW: u8 = 0u8;
const STATE_INIT: u8 = 1u8;
const STATE_READY: u8 = 2u8;

pub struct Lazy<T> {
    d: UnsafeCell<MaybeUninit<T>>,
    v: AtomicU8,
}
#[repr(transparent)]
pub struct LazyHandle<T> {
    v:  AtomicIsize,
    _p: PhantomData<T>,
}

pub trait LazyValue: Sized {
    fn lazy_new() -> isize;
}

pub(super) enum LazyResult<T, E> {
    Ok,
    Error(E),
    Filled(T),
}

impl<T> Lazy<T> {
    #[inline]
    pub const fn new() -> Lazy<T> {
        Lazy {
            v: AtomicU8::new(STATE_NEW),
            d: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    #[inline]
    pub fn new_ready(v: T) -> Lazy<T> {
        Lazy {
            v: AtomicU8::new(STATE_READY),
            d: UnsafeCell::new(MaybeUninit::new(v)),
        }
    }

    #[inline]
    pub fn load(&self) -> u8 {
        self.v.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.v.load(Ordering::Acquire) == STATE_READY
    }
    #[inline]
    pub fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_ready());
        unsafe { (&*self.d.get()).assume_init_ref() }
    }
    #[inline]
    pub fn get_no_init(&self) -> Option<&T> {
        if self.is_ready() {
            Some(unsafe { (&*self.d.get()).assume_init_ref() })
        } else {
            None
        }
    }
    #[inline]
    pub fn get_mut_unchecked(&self) -> &mut T {
        debug_assert!(self.is_ready());
        unsafe { (&mut *self.d.get()).assume_init_mut() }
    }
    #[inline]
    pub fn get_mut_no_init(&self) -> Option<&mut T> {
        if self.is_ready() {
            Some(unsafe { (&mut *self.d.get()).assume_init_mut() })
        } else {
            None
        }
    }
    #[inline]
    pub fn get(&self, f: impl FnOnce() -> T) -> &T {
        let _ = self.lock::<()>(|| Ok(f()), || spin(&self.v));
        unsafe { (&*self.d.get()).assume_init_ref() }
    }
    #[inline]
    pub fn get_mut(&self, f: impl FnOnce() -> T) -> &mut T {
        let _ = self.lock::<()>(|| Ok(f()), || spin(&self.v));
        unsafe { (&mut *self.d.get()).assume_init_mut() }
    }

    #[inline]
    pub unsafe fn take(&self) -> Option<T> {
        unsafe {
            self.v
                .compare_exchange(STATE_READY, STATE_NEW, Ordering::AcqRel, Ordering::Relaxed)
                .ok()
                .map(|_| {
                    // Replace and drop the previous value.
                    replace(&mut *self.d.get(), MaybeUninit::uninit()).assume_init()
                })
        }
    }

    #[inline(never)]
    pub(super) fn lock<E>(&self, f: impl FnOnce() -> Result<T, E>, w: impl FnOnce()) -> LazyResult<T, E> {
        fence(Ordering::SeqCst);
        match self
            .v
            .compare_exchange(STATE_NEW, STATE_INIT, Ordering::AcqRel, Ordering::Relaxed)
        {
            Ok(_) => {
                let r = match f() {
                    Ok(v) => {
                        unsafe { &mut *self.d.get() }.write(v);
                        LazyResult::Ok
                    },
                    Err(e) => LazyResult::Error(e),
                };
                self.v.store(STATE_READY, Ordering::Release);
                r
            },
            Err(STATE_NEW) => unreachable!(),
            Err(STATE_INIT) => {
                w();
                f().map_or_else(LazyResult::Error, LazyResult::Filled)
            },
            Err(_) => LazyResult::Ok,
        }
    }
}
impl<T> LazyHandle<T> {
    #[inline]
    pub fn new_with_value(v: T) -> LazyHandle<T> {
        let r: LazyHandle<T> = LazyHandle {
            v:  AtomicIsize::new(-1isize),
            _p: PhantomData,
        };
        let _ = unsafe { r.replace(v) };
        r
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        match self.v.load(Ordering::Acquire) {
            -1 | 0 => false,
            _ => true,
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.v.as_ptr() as *const T
    }
    #[inline]
    pub fn get_unchecked(&self) -> &T {
        debug_assert!(self.is_ready());
        unsafe { &*(self.v.as_ptr() as *const T) }
    }
    #[inline]
    pub fn get_no_init(&self) -> Option<&T> {
        if self.is_ready() {
            Some(self.get_unchecked())
        } else {
            None
        }
    }
    #[inline]
    pub fn get_mut_unchecked(&self) -> &mut T {
        debug_assert!(self.is_ready());
        unsafe { &mut *(self.v.as_ptr() as *mut T) }
    }
    #[inline]
    pub fn get_mut_no_init(&self) -> Option<&mut T> {
        if self.is_ready() {
            Some(self.get_mut_unchecked())
        } else {
            None
        }
    }

    #[inline]
    pub unsafe fn take(&self) -> T {
        let v = unsafe { read(self.v.as_ptr() as *mut T) };
        self.v.store(-1, Ordering::Release);
        v
    }
    #[inline]
    pub unsafe fn replace(&self, v: T) -> T {
        replace(unsafe { &mut *(self.v.as_ptr() as *mut T) }, v)
    }
    #[inline]
    pub unsafe fn as_inner(&self) -> &AtomicIsize {
        &self.v
    }
}
impl<T: LazyValue> LazyHandle<T> {
    #[inline]
    pub const fn new() -> LazyHandle<T> {
        LazyHandle {
            v:  AtomicIsize::new(-1isize),
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn get(&self) -> &T {
        self.lock();
        self.get_unchecked()
    }
    #[inline]
    pub fn get_mut(&self) -> &mut T {
        self.lock();
        self.get_mut_unchecked()
    }

    #[inline(never)]
    fn lock(&self) {
        fence(Ordering::SeqCst);
        match self.v.compare_exchange(-1, 0, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => self.v.store(T::lazy_new(), Ordering::Release),
            Err(-1) => unreachable!(),
            Err(0) => {
                // Spin while we wait..
                while self.v.load(Ordering::Relaxed) == 0 {
                    let _ = unsafe { read_volatile(&self.v) }; // Prevent optimization of loop.
                    spin_loop()
                }
            },
            Err(_) => (),
        }
    }
}

impl<T> Drop for Lazy<T> {
    #[inline]
    fn drop(&mut self) {
        if self.is_ready() {
            unsafe { (&mut *self.d.get()).assume_init_drop() };
        }
    }
}
impl<T: Clone> Clone for Lazy<T> {
    #[inline]
    fn clone(&self) -> Lazy<T> {
        self.get_no_init().map_or_else(Lazy::new, |v| Lazy::new_ready(v.clone()))
    }
}
impl<T: Debug> Debug for Lazy<T> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Lazy(")?;
        match self.v.load(Ordering::Relaxed) {
            STATE_NEW => f.write_str("Uninitialized"),
            STATE_INIT => f.write_str("Spinning"),
            _ => Debug::fmt(self.get_unchecked(), f),
        }?;
        f.write_str(")")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}

impl<T> Drop for LazyHandle<T> {
    #[inline]
    fn drop(&mut self) {
        if self.is_ready() {
            drop(unsafe { self.take() });
        }
    }
}
impl<T> AsRef<usize> for LazyHandle<T> {
    #[inline]
    fn as_ref(&self) -> &usize {
        unsafe { &*(self.v.as_ptr() as *const usize) }
    }
}
impl<T: Debug> Debug for LazyHandle<T> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("Lazy(")?;
        match self.v.load(Ordering::Relaxed) {
            -1 => f.write_str("Uninitialized"),
            0 => f.write_str("Spinning"),
            _ => Debug::fmt(self.get_unchecked(), f),
        }?;
        f.write_str(")")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T: LazyValue> Deref for LazyHandle<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.get()
    }
}
impl<T: LazyValue> Default for LazyHandle<T> {
    #[inline]
    fn default() -> LazyHandle<T> {
        LazyHandle::new()
    }
}
impl<T: LazyValue + Clone> Clone for LazyHandle<T> {
    #[inline]
    fn clone(&self) -> LazyHandle<T> {
        self.get_no_init().map_or_else(
            || LazyHandle::new(),
            |v| LazyHandle::new_with_value(v.clone()),
        )
    }
}

impl<T> UnwindSafe for Lazy<T> {}
impl<T> RefUnwindSafe for Lazy<T> {}

impl<T> UnwindSafe for LazyHandle<T> {}
impl<T> RefUnwindSafe for LazyHandle<T> {}

unsafe impl<T> Send for Lazy<T> {}
unsafe impl<T> Sync for Lazy<T> {}

unsafe impl<T> Send for LazyHandle<T> {}
unsafe impl<T> Sync for LazyHandle<T> {}

#[inline(never)]
fn spin(v: &AtomicU8) {
    while v.load(Ordering::Relaxed) == STATE_INIT {
        let _ = unsafe { read_volatile(v) };
        spin_loop()
    }
}
