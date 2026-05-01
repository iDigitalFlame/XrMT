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
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

use core::cell::UnsafeCell;
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::marker::{Send, Sync};
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut, Drop, FnOnce};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr::read;
use core::result::Result::{self, Err, Ok};

use crate::io::FmtResult;
use crate::sync::Once;

/// A value which is initialized on the first access.
///
/// This type is a thread-safe [`LazyCell`], and can be used in statics.
/// Since initialization may be called from multiple threads, any
/// dereferencing call will block the calling thread if another
/// initialization routine is currently running.
///
/// [`LazyCell`]: core::cell::LazyCell
///
/// # Examples
///
/// Initialize static variables with `LazyLock`.
/// ```
/// use xrmt_stx::sync::LazyLock;
///
/// // Note: static items do not call [`Drop`] on program termination, so this won't be deallocated.
/// // this is fine, as the OS can deallocate the terminated program faster than we can free memory
/// // but tools like valgrind might report "memory leaks" as it isn't obvious this is intentional.
/// static DEEP_THOUGHT: LazyLock<String> = LazyLock::new(|| {
/// # mod another_crate {
/// #     pub fn great_question() -> String { "42".to_string() }
/// # }
///     // M3 Ultra takes about 16 million years in --release config
///     another_crate::great_question()
/// });
///
/// // The `String` is built, stored in the `LazyLock`, and returned as `&String`.
/// let _ = &*DEEP_THOUGHT;
/// ```
///
/// Initialize fields with `LazyLock`.
/// ```
/// use xrmt_stx::sync::LazyLock;
///
/// #[derive(Debug)]
/// struct UseCellLock {
///     number: LazyLock<u32>,
/// }
/// fn main() {
///     let lock: LazyLock<u32> = LazyLock::new(|| 0u32);
///
///     let data = UseCellLock { number: lock };
///     println!("{}", *data.number);
/// }
/// ```
pub struct LazyLock<T, F = fn() -> T> {
    v: Once,
    d: UnsafeCell<Data<T, F>>,
}

union Data<T, F> {
    v: ManuallyDrop<T>,
    f: ManuallyDrop<F>,
}

impl<T, F> LazyLock<T, F> {
    /// Returns a reference to the value if initialized, or `None` if not.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get(&lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// assert_eq!(LazyLock::get(&lazy), Some(&92));
    /// ```
    #[inline]
    pub fn get(v: &LazyLock<T, F>) -> Option<&T> {
        if v.v.is_completed() {
            Some(unsafe { &(&*v.d.get()).v })
        } else {
            None
        }
    }
    /// Returns a mutable reference to the value if initialized, or `None` if
    /// not.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get_mut(&mut lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// *LazyLock::get_mut(&mut lazy).unwrap() = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    pub fn get_mut(v: &mut LazyLock<T, F>) -> Option<&mut T> {
        if v.v.is_completed() {
            Some(unsafe { &mut (&mut *v.d.get()).v })
        } else {
            None
        }
    }
}
impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    /// Creates a new lazy value with the given initializing function.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyLock::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// ```
    #[inline]
    pub const fn new(f: F) -> LazyLock<T, F> {
        LazyLock {
            v: Once::new(),
            d: UnsafeCell::new(Data { f: ManuallyDrop::new(f) }),
        }
    }

    /// Forces the evaluation of this lazy value and returns a reference to
    /// result. This is equivalent to the `Deref` impl, but is explicit.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    #[inline]
    pub fn force(v: &LazyLock<T, F>) -> &T {
        v.v.call_once(|| {
            let d = unsafe { &mut *v.d.get() };
            d.v = ManuallyDrop::new(unsafe { ManuallyDrop::take(&mut d.f) }());
        });
        unsafe { &*(*v.d.get()).v }
    }
    /// Forces the evaluation of this lazy value and returns a mutable reference
    /// to the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// let p = LazyLock::force_mut(&mut lazy);
    /// assert_eq!(*p, 92);
    /// *p = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    pub fn force_mut(v: &mut LazyLock<T, F>) -> &mut T {
        if !v.v.is_completed() {
            let _ = LazyLock::force(v);
        }
        unsafe { &mut *(*v.d.get()).v }
    }
    /// Consumes this `LazyLock` returning the stored value.
    ///
    /// Returns `Ok(value)` if `Lazy` is initialized and `Err(f)` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(lazy_cell_into_inner)]
    ///
    /// use xrmt_stx::sync::LazyLock;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyLock::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// assert_eq!(LazyLock::into_inner(lazy).ok(), Some("HELLO, WORLD!".to_string()));
    /// ```
    #[inline]
    pub fn into_inner(v: LazyLock<T, F>) -> Result<T, F> {
        let d = unsafe { read(&v.d).into_inner() };
        if v.v.is_completed() {
            Ok(ManuallyDrop::into_inner(unsafe { d.v }))
        } else {
            Err(ManuallyDrop::into_inner(unsafe { d.f }))
        }
    }
}

impl<T, F> Drop for LazyLock<T, F> {
    #[inline]
    fn drop(&mut self) {
        if self.v.is_completed() {
            unsafe { ManuallyDrop::drop(&mut self.d.get_mut().v) }
        } else {
            unsafe { ManuallyDrop::drop(&mut self.d.get_mut().f) }
        }
    }
}
impl<T: Default> Default for LazyLock<T> {
    #[inline]
    fn default() -> LazyLock<T> {
        LazyLock::new(T::default)
    }
}
impl<T: Debug, F> Debug for LazyLock<T, F> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(&LazyLock::get(self), f)
    }
}
impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        LazyLock::force(self)
    }
}
impl<T, F: FnOnce() -> T> DerefMut for LazyLock<T, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        LazyLock::force_mut(self)
    }
}
impl<T: UnwindSafe, F: UnwindSafe> UnwindSafe for LazyLock<T, F> {}
impl<T: RefUnwindSafe + UnwindSafe, F: UnwindSafe> RefUnwindSafe for LazyLock<T, F> {}

unsafe impl<T: Sync + Send, F: Send> Sync for LazyLock<T, F> {}
