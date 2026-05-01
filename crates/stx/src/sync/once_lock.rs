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

use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::marker::{Send, Sync};
use core::ops::FnOnce;
use core::option::Option;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{self, Err, Ok};

use crate::io::FmtResult;
use crate::sync::extra::LazyReusable;

/// A synchronization primitive which can nominally be written to only once.
///
/// This type is a thread-safe [`OnceCell`], and can be used in statics.
/// In many simple cases, you can use [`LazyLock<T, F>`] instead to get the
/// benefits of this type with less effort: `LazyLock<T, F>` "looks like" `&T`
/// because it initializes with `F` on deref! Where OnceLock shines is when
/// LazyLock is too simple to support a given case, as LazyLock doesn't allow
/// additional inputs to its function after you call [`LazyLock::new(|| ...)`].
///
/// A `OnceLock` can be thought of as a safe abstraction over uninitialized data
/// that becomes initialized once written.
///
/// [`OnceCell`]: core::cell::OnceCell
/// [`LazyLock<T, F>`]: crate::sync::LazyLock
/// [`LazyLock::new(|| ...)`]: crate::sync::LazyLock::new
///
/// # Examples
///
/// Writing to a `OnceLock` from a separate thread:
///
/// ```
/// use xrmt_stx::sync::OnceLock;
///
/// static CELL: OnceLock<usize> = OnceLock::new();
///
/// // `OnceLock` has not been written to yet.
/// assert!(CELL.get().is_none());
///
/// // Spawn a thread and write to `OnceLock`.
/// xrmt_stx::thread::spawn(|| {
///     let value = CELL.get_or_init(|| 12345);
///     assert_eq!(value, &12345);
/// })
/// .join()
/// .unwrap();
///
/// // `OnceLock` now contains the value.
/// assert_eq!(
///     CELL.get(),
///     Some(&12345),
/// );
/// ```
///
/// You can use `OnceLock` to implement a type that requires "append-only"
/// logic:
///
/// ```
/// use xrmt_stx::sync::{OnceLock, atomic::{AtomicU32, Ordering}};
/// use xrmt_stx::thread;
///
/// struct OnceList<T> {
///     data: OnceLock<T>,
///     next: OnceLock<Box<OnceList<T>>>,
/// }
/// impl<T> OnceList<T> {
///     const fn new() -> OnceList<T> {
///         OnceList { data: OnceLock::new(), next: OnceLock::new() }
///     }
///     fn push(&self, value: T) {
///         // FIXME: this impl is concise, but is also slow for long lists or many threads.
///         // as an exercise, consider how you might improve on it while preserving the behavior
///         if let Err(value) = self.data.set(value) {
///             let next = self.next.get_or_init(|| Box::new(OnceList::new()));
///             next.push(value)
///         };
///     }
///     fn contains(&self, example: &T) -> bool
///     where
///         T: PartialEq,
///     {
///         self.data.get().map(|item| item == example).filter(|v| *v).unwrap_or_else(|| {
///             self.next.get().map(|next| next.contains(example)).unwrap_or(false)
///         })
///     }
/// }
///
/// // Let's exercise this new Sync append-only list by doing a little counting
/// static LIST: OnceList<u32> = OnceList::new();
/// static COUNTER: AtomicU32 = AtomicU32::new(0);
///
/// # const LEN: u32 = if cfg!(miri) { 50 } else { 1000 };
/// # /*
/// const LEN: u32 = 1000;
/// # */
/// thread::scope(|s| {
///     for _ in 0..thread::available_parallelism().unwrap().get() {
///         s.spawn(|| {
///             while let i @ 0..LEN = COUNTER.fetch_add(1, Ordering::Relaxed) {
///                 LIST.push(i);
///             }
///         });
///     }
/// });
///
/// for i in 0..LEN {
///     assert!(LIST.contains(&i));
/// }
/// ```
pub struct OnceLock<T>(LazyReusable<T>);

impl<T> OnceLock<T> {
    /// Creates a new uninitialized cell.
    #[inline]
    pub const fn new() -> OnceLock<T> {
        OnceLock(LazyReusable::new())
    }

    /// Blocks the current thread until the cell is initialized.
    ///
    /// # Example
    ///
    /// Waiting for a computation on another thread to finish:
    /// ```rust
    /// use xrmt_stx::thread;
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let value = OnceLock::new();
    ///
    /// thread::scope(|s| {
    ///     s.spawn(|| value.set(1 + 1));
    ///
    ///     let result = value.wait();
    ///     assert_eq!(result, &2);
    /// })
    /// ```
    #[inline]
    pub fn wait(&self) -> &T {
        self.0.wait();
        self.0.get_unchecked()
    }
    /// Gets the reference to the underlying value.
    ///
    /// Returns `None` if the cell is uninitialized, or being initialized.
    /// This method never blocks.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.0.get_no_init()
    }
    /// Takes the value out of this `OnceLock`, moving it back to an
    /// uninitialized state.
    ///
    /// Has no effect and returns `None` if the `OnceLock` was uninitialized.
    ///
    /// Safety is guaranteed by requiring a mutable reference.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let mut cell: OnceLock<String> = OnceLock::new();
    /// assert_eq!(cell.take(), None);
    ///
    /// let mut cell = OnceLock::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.take(), Some("hello".to_string()));
    /// assert_eq!(cell.get(), None);
    /// ```
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        self.0.reset()
    }
    /// Consumes the `OnceLock`, returning the wrapped value. Returns
    /// `None` if the cell was uninitialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let cell: OnceLock<String> = OnceLock::new();
    /// assert_eq!(cell.into_inner(), None);
    ///
    /// let cell = OnceLock::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.into_inner(), Some("hello".to_string()));
    /// ```
    #[inline]
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }
    /// Initializes the contents of the cell to `value`.
    ///
    /// May block if another thread is currently attempting to initialize the
    /// cell. The cell is guaranteed to contain a value when `set` returns,
    /// though not necessarily the one provided.
    ///
    /// Returns `Ok(())` if the cell was uninitialized and
    /// `Err(value)` if the cell was already initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// fn main() {
    ///     assert!(CELL.get().is_none());
    ///
    ///     xrmt_stx::thread::spawn(|| {
    ///         assert_eq!(CELL.set(92), Ok(()));
    ///     }).join().unwrap();
    ///
    ///     assert_eq!(CELL.set(62), Err(62));
    ///     assert_eq!(CELL.get(), Some(&92));
    /// }
    /// ```
    #[inline]
    pub fn set(&self, v: T) -> Result<(), T> {
        match self.try_insert(v) {
            Err((_, x)) => Err(x),
            Ok(_) => Ok(()),
        }
    }
    /// Gets the mutable reference to the underlying value.
    ///
    /// Returns `None` if the cell is uninitialized, or being initialized.
    /// This method never blocks.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.0.get_mut_no_init()
    }
    /// Initializes the contents of the cell to `value` if the cell was
    /// uninitialized, then returns a reference to it.
    ///
    /// May block if another thread is currently attempting to initialize the
    /// cell. The cell is guaranteed to contain a value when `try_insert`
    /// returns, though not necessarily the one provided.
    ///
    /// Returns `Ok(&value)` if the cell was uninitialized and
    /// `Err((&current_value, value))` if it was already initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(once_cell_try_insert)]
    ///
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// fn main() {
    ///     assert!(CELL.get().is_none());
    ///
    ///     xrmt_stx::thread::spawn(|| {
    ///         assert_eq!(CELL.try_insert(92), Ok(&92));
    ///     }).join().unwrap();
    ///
    ///     assert_eq!(CELL.try_insert(62), Err((&92, 62)));
    ///     assert_eq!(CELL.get(), Some(&92));
    /// }
    /// ```
    #[inline]
    pub fn try_insert(&self, v: T) -> Result<&T, (&T, T)> {
        // If `map_error` hits, the init will ALWAYS pass, so we can do unchecked.
        self.0.get(|| v).map_err(|r| (self.0.get_unchecked(), r))
    }
    /// Gets the contents of the cell, initializing it to `f()` if the cell
    /// was uninitialized.
    ///
    /// Many threads may call `get_or_init` concurrently with different
    /// initializing functions, but it is guaranteed that only one function
    /// will be executed.
    ///
    /// # Panics
    ///
    /// If `f()` panics, the panic is propagated to the caller, and the cell
    /// remains uninitialized.
    ///
    /// It is an error to reentrantly initialize the cell from `f`. The
    /// exact outcome is unspecified. Current implementation deadlocks, but
    /// this may be changed to a panic in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let cell = OnceLock::new();
    /// let value = cell.get_or_init(|| 92);
    /// assert_eq!(value, &92);
    /// let value = cell.get_or_init(|| unreachable!());
    /// assert_eq!(value, &92);
    /// ```
    #[inline]
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        match self.0.get(f) {
            Err(_) => self.0.get_unchecked(),
            Ok(v) => v,
        }
    }
    /// Gets the mutable reference of the contents of the cell, initializing
    /// it to `f()` if the cell was uninitialized.
    ///
    /// This method never blocks.
    ///
    /// # Panics
    ///
    /// If `f()` panics, the panic is propagated to the caller, and the cell
    /// remains uninitialized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let mut cell = OnceLock::new();
    /// let value = cell.get_mut_or_init(|| 92);
    /// assert_eq!(*value, 92);
    ///
    /// *value += 2;
    /// assert_eq!(*value, 94);
    ///
    /// let value = cell.get_mut_or_init(|| unreachable!());
    /// assert_eq!(*value, 94);
    /// ```
    #[inline]
    pub fn get_mut_or_init(&mut self, f: impl FnOnce() -> T) -> &mut T {
        match self.0.get_mut(f) {
            Err(_) => self.0.get_mut_unchecked(),
            Ok(v) => v,
        }
    }
    /// Gets the contents of the cell, initializing it to `f()` if
    /// the cell was uninitialized. If the cell was uninitialized
    /// and `f()` failed, an error is returned.
    ///
    /// # Panics
    ///
    /// If `f()` panics, the panic is propagated to the caller, and
    /// the cell remains uninitialized.
    ///
    /// It is an error to reentrantly initialize the cell from `f`.
    /// The exact outcome is unspecified. Current implementation
    /// deadlocks, but this may be changed to a panic in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(once_cell_try)]
    ///
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let cell = OnceLock::new();
    /// assert_eq!(cell.get_or_try_init(|| Err(())), Err(()));
    /// assert!(cell.get().is_none());
    /// let value = cell.get_or_try_init(|| -> Result<i32, ()> {
    ///     Ok(92)
    /// });
    /// assert_eq!(value, Ok(&92));
    /// assert_eq!(cell.get(), Some(&92))
    /// ```
    #[inline]
    pub fn get_or_try_init<E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<&T, E> {
        self.0.get_error(f)
    }
    /// Gets the mutable reference of the contents of the cell, initializing
    /// it to `f()` if the cell was uninitialized. If the cell was uninitialized
    /// and `f()` failed, an error is returned.
    ///
    /// This method never blocks.
    ///
    /// # Panics
    ///
    /// If `f()` panics, the panic is propagated to the caller, and
    /// the cell remains uninitialized.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use xrmt_stx::sync::OnceLock;
    ///
    /// let mut cell: OnceLock<u32> = OnceLock::new();
    ///
    /// // Failed attempts to initialize the cell do not change its contents
    /// assert!(cell.get_mut_or_try_init(|| "not a number!".parse()).is_err());
    /// assert!(cell.get().is_none());
    ///
    /// let value = cell.get_mut_or_try_init(|| "1234".parse());
    /// assert_eq!(value, Ok(&mut 1234));
    /// *value.unwrap() += 2;
    /// assert_eq!(cell.get(), Some(&1236))
    /// ```
    #[inline]
    pub fn get_mut_or_try_init<E>(&mut self, f: impl FnOnce() -> Result<T, E>) -> Result<&mut T, E> {
        self.0.get_error_mut(f)
    }
}

impl<T: Eq> Eq for OnceLock<T> {}
impl<T> Default for OnceLock<T> {
    #[inline]
    fn default() -> OnceLock<T> {
        OnceLock(LazyReusable::default())
    }
}
impl<T: Clone> Clone for OnceLock<T> {
    #[inline]
    fn clone(&self) -> OnceLock<T> {
        OnceLock(self.0.clone())
    }
}
impl<T: Debug> Debug for OnceLock<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(&self.0.get_no_init(), f)
    }
}
impl<T: PartialEq> PartialEq for OnceLock<T> {
    #[inline]
    fn eq(&self, other: &OnceLock<T>) -> bool {
        self.0.get_no_init().eq(&other.0.get_no_init())
    }
}
impl<T: UnwindSafe> UnwindSafe for OnceLock<T> {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceLock<T> {}

unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
