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
use core::fmt::{Debug, Formatter, Result};
use core::marker::{PhantomData, Send, Sync};
use core::ops::FnOnce;
use core::panic::{RefUnwindSafe, UnwindSafe};

use crate::sync::extra::LazyOnce;

/// A low-level synchronization primitive for one-time global execution.
///
/// Previously this was the only "execute once" synchronization in `std`.
/// Other libraries implemented novel synchronizing types with `Once`, like
/// [`OnceLock<T>`] or [`LazyLock<T, F>`], before those were added to `std`.
/// `OnceLock<T>` in particular supersedes `Once` in functionality and should
/// be preferred for the common case where the `Once` is associated with data.
///
/// This type can only be constructed with [`Once::new()`].
///
/// # Examples
///
/// ```
/// use xrmt_stx::sync::Once;
///
/// static START: Once = Once::new();
///
/// START.call_once(|| {
///     // run initialization here
/// });
/// ```
///
/// [`OnceLock<T>`]: crate::sync::OnceLock
/// [`LazyLock<T, F>`]: crate::sync::LazyLock
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Once(LazyOnce<()>);
pub struct OnceState(PhantomData<UnsafeCell<*mut ()>>);

impl Once {
    /// Creates a new `Once` value.
    #[inline]
    pub const fn new() -> Once {
        Once(LazyOnce::new())
    }

    /// Blocks the current thread until initialization has completed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xrmt_stx::sync::Once;
    /// use xrmt_stx::thread;
    ///
    /// static READY: Once = Once::new();
    ///
    /// let thread = thread::spawn(|| {
    ///     READY.wait();
    ///     println!("everything is ready");
    /// });
    ///
    /// READY.call_once(|| println!("performing setup"));
    /// ```
    ///
    /// # Panics
    ///
    /// If this [`Once`] has been poisoned because an initialization closure has
    /// panicked, this method will also panic. Use
    /// [`wait_force`](Self::wait_force) if this behavior is not desired.
    #[inline]
    pub fn wait(&self) {
        self.0.wait();
    }
    /// Blocks the current thread until initialization has completed, ignoring
    /// poisoning.
    #[inline]
    pub fn wait_force(&self) {
        self.wait();
    }
    /// Returns `true` if some [`call_once()`] call has completed
    /// successfully. Specifically, `is_completed` will return false in
    /// the following situations:
    ///   * [`call_once()`] was not called at all,
    ///   * [`call_once()`] was called, but has not yet completed,
    ///   * the [`Once`] instance is poisoned
    ///
    /// This function returning `false` does not mean that [`Once`] has not been
    /// executed. For example, it may have been executed in the time between
    /// when `is_completed` starts executing and when it returns, in which case
    /// the `false` return value would be stale (but still permissible).
    ///
    /// [`call_once()`]: Once::call_once
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// INIT.call_once(|| {
    ///     assert_eq!(INIT.is_completed(), false);
    /// });
    /// assert_eq!(INIT.is_completed(), true);
    /// ```
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    /// use xrmt_stx::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    /// assert_eq!(INIT.is_completed(), false);
    /// ```
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.0.is_ready()
    }
    /// Performs an initialization routine once and only once. The given closure
    /// will be executed if this is the first time `call_once` has been called,
    /// and otherwise the routine will *not* be invoked.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// When this function returns, it is guaranteed that some initialization
    /// has run and completed (it might not be the closure specified). It is
    /// also guaranteed that any memory writes performed by the executed
    /// closure can be reliably observed by other threads at this point
    /// (there is a happens-before relation between the closure and code
    /// executing after the return).
    ///
    /// If the given closure recursively invokes `call_once` on the same
    /// [`Once`] instance, the exact behavior is not specified: allowed
    /// outcomes are a panic or a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    ///
    /// static mut VAL: usize = 0;
    /// static INIT: Once = Once::new();
    ///
    /// // Accessing a `static mut` is unsafe much of the time, but if we do so
    /// // in a synchronized fashion (e.g., write once or read all) then we're
    /// // good to go!
    /// //
    /// // This function will only call `expensive_computation` once, and will
    /// // otherwise always return the value returned from the first invocation.
    /// fn get_cached_val() -> usize {
    ///     unsafe {
    ///         INIT.call_once(|| {
    ///             VAL = expensive_computation();
    ///         });
    ///         VAL
    ///     }
    /// }
    ///
    /// fn expensive_computation() -> usize {
    ///     // ...
    /// # 2
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// The closure `f` will only be executed once even if this is called
    /// concurrently amongst many threads. If that closure panics, however, then
    /// it will *poison* this [`Once`] instance, causing all future invocations
    /// of `call_once` to also panic.
    ///
    /// This is similar to [poisoning with mutexes][poison].
    ///
    /// [poison]: struct.Mutex.html#poisoning
    #[track_caller]
    #[inline]
    pub fn call_once(&self, f: impl FnOnce()) {
        self.call(f);
    }
    /// Performs the same function as [`call_once()`] except ignores poisoning.
    ///
    /// Unlike [`call_once()`], if this [`Once`] has been poisoned (i.e., a
    /// previous call to [`call_once()`] or [`call_once_force()`] caused a
    /// panic), calling [`call_once_force()`] will still invoke the closure
    /// `f` and will _not_ result in an immediate panic. If `f` panics, the
    /// [`Once`] will remain in a poison state. If `f` does _not_ panic, the
    /// [`Once`] will no longer be in a poison state and all future calls to
    /// [`call_once()`] or [`call_once_force()`] will be no-ops.
    ///
    /// The closure `f` is yielded a [`OnceState`] structure which can be used
    /// to query the poison status of the [`Once`].
    ///
    /// [`call_once()`]: Once::call_once
    /// [`call_once_force()`]: Once::call_once_force
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    /// use xrmt_stx::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// // poison the once
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // poisoning propagates
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| {});
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // call_once_force will still run and reset the poisoned state
    /// INIT.call_once_force(|state| {
    ///     assert!(state.is_poisoned());
    /// });
    ///
    /// // once any success happens, we stop propagating the poison
    /// INIT.call_once(|| {});
    /// ```
    #[track_caller]
    #[inline]
    pub fn call_once_force(&self, f: impl FnOnce(&OnceState)) {
        self.call(|| f(&OnceState(PhantomData)))
    }

    #[inline]
    fn call(&self, f: impl FnOnce()) {
        if self.0.is_ready() {
            return;
        }
        let _ = self.0.get(|| f());
    }
}
impl OnceState {
    /// Returns `true` if the associated [`Once`] was poisoned prior to the
    /// invocation of the closure passed to [`Once::call_once_force()`].
    ///
    /// # Examples
    ///
    /// A poisoned [`Once`]:
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    /// use xrmt_stx::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// // poison the once
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// INIT.call_once_force(|state| {
    ///     assert!(state.is_poisoned());
    /// });
    /// ```
    ///
    /// An unpoisoned [`Once`]:
    ///
    /// ```
    /// use xrmt_stx::sync::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// INIT.call_once_force(|state| {
    ///     assert!(!state.is_poisoned());
    /// });
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        false
    }
}

impl UnwindSafe for Once {}
impl RefUnwindSafe for Once {}

impl Debug for OnceState {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("OnceState")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> Result {
        core::result::Result::Ok(())
    }
}

unsafe impl Sync for Once {}
unsafe impl Send for OnceState {}
