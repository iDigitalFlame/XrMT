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
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;

use crate::sync::Once;
use crate::prelude::*;

pub struct LazyLock<T, F = fn() -> T> {
    cell: UnsafeCell<InitCell<T, F>>,
    once: Once,
}

union InitCell<T, F> {
    func: ManuallyDrop<F>,
    data: ManuallyDrop<T>,
}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    #[inline]
    pub const fn new(func: F) -> LazyLock<T, F> {
        LazyLock {
            once: Once::new(),
            cell: UnsafeCell::new(InitCell { func: ManuallyDrop::new(func) }),
        }
    }

    #[inline]
    pub fn force(this: &LazyLock<T, F>) -> &T {
        this.get()
    }
    #[inline]
    pub fn into_inner(this: LazyLock<T, F>) -> Result<T, F> {
        let mut t = ManuallyDrop::new(this);
        let v = unsafe { ptr::read(&t.cell) }.into_inner();
        let r = if t.once.is_completed() {
            Ok(ManuallyDrop::into_inner(unsafe { v.data }))
        } else {
            Err(ManuallyDrop::into_inner(unsafe { v.func }))
        };
        unsafe { ManuallyDrop::drop(&mut t) };
        r
    }

    #[inline]
    fn get(&self) -> &mut T {
        self.once.call_once(|| self.init());
        unsafe { &mut *(*self.cell.get()).data }
    }
    #[inline]
    fn init(&self) {
        let d = unsafe { &mut *self.cell.get() };
        let f = unsafe { ManuallyDrop::take(&mut d.func) };
        d.data = ManuallyDrop::new(f());
    }
}

impl<T, F> Drop for LazyLock<T, F> {
    #[inline]
    fn drop(&mut self) {
        let c = self.cell.get_mut();
        if self.once.is_completed() {
            unsafe { ManuallyDrop::drop(&mut c.data) }
        } else {
            unsafe { ManuallyDrop::drop(&mut c.func) }
        }
    }
}
impl<F: Default> Default for LazyLock<F> {
    #[inline]
    fn default() -> LazyLock<F> {
        LazyLock::new(F::default)
    }
}
impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.get()
    }
}

impl<T: UnwindSafe, F: UnwindSafe> UnwindSafe for LazyLock<T, F> {}
impl<T: RefUnwindSafe + UnwindSafe, F: UnwindSafe> RefUnwindSafe for LazyLock<T, F> {}

unsafe impl<T: Sync + Send, F: Send> Sync for LazyLock<T, F> {}
