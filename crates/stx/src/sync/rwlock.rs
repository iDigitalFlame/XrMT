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
use core::clone::Clone;
use core::convert::From;
use core::default::Default;
use core::fmt::{Debug, Display, Formatter};
use core::marker::{PhantomData, Send, Sized, Sync};
use core::mem::{drop, forget, needs_drop, replace};
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::None;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{self, Err, Ok};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::io::FmtResult;
use crate::sync::extra::MutantConstant;
use crate::sync::{LockResult, PoisonError, TryLockError, TryLockResult};

/// A reader-writer lock
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// In comparison, a [`Mutex`] does not distinguish between readers or writers
/// that acquire the lock, therefore blocking any threads waiting for the lock
/// to become available. An `RwLock` will allow any number of readers to acquire
/// the lock as long as a writer is not holding the lock.
///
/// The priority policy of the lock is dependent on the underlying operating
/// system's implementation, and this type does not guarantee that any
/// particular policy will be used. In particular, a writer which is waiting to
/// acquire the lock in `write` might or might not block concurrent calls to
/// `read`, e.g.:
///
/// <details><summary>Potential deadlock example</summary>
///
/// ```text
/// // Thread 1              |  // Thread 2
/// let _rg1 = lock.read();  |
///                          |  // will block
///                          |  let _wg = lock.write();
/// // may deadlock          |
/// let _rg2 = lock.read();  |
/// ```
///
/// </details>
///
/// The type parameter `T` represents the data that this lock protects. It is
/// required that `T` satisfies [`Send`] to be shared across threads and
/// [`Sync`] to allow concurrent access through readers. The RAII guards
/// returned from the locking methods implement [`Deref`] (and [`DerefMut`]
/// for the `write` methods) to allow access to the content of the lock.
///
/// # Poisoning
///
/// An `RwLock`, like [`Mutex`], will become poisoned on a panic. Note, however,
/// that an `RwLock` may only be poisoned if a panic occurs while it is locked
/// exclusively (write mode). If a panic occurs in any reader, then the lock
/// will not be poisoned.
///
/// # Examples
///
/// ```
/// use xrmt_stx::sync::RwLock;
///
/// let lock = RwLock::new(5);
///
/// // many reader locks can be held at once
/// {
///     let r1 = lock.read().unwrap();
///     let r2 = lock.read().unwrap();
///     assert_eq!(*r1, 5);
///     assert_eq!(*r2, 5);
/// } // read locks are dropped at this point
///
/// // only one write lock may be held, however
/// {
///     let mut w = lock.write().unwrap();
///     *w += 1;
///     assert_eq!(*w, 6);
/// } // write lock is dropped here
/// ```
///
/// [`Mutex`]: super::Mutex
pub struct RwLock<T: ?Sized> {
    w: MutantConstant,
    r: MutantConstant,
    c: AtomicUsize,
    d: UnsafeCell<T>,
}
/// RAII structure used to release the shared read access of a lock when
/// dropped.
///
/// This structure is created by the [`read`] and [`try_read`] methods on
/// [`RwLock`].
///
/// [`read`]: RwLock::read
/// [`try_read`]: RwLock::try_read
pub struct RwLockReadGuard<'a, T: ?Sized + 'a> {
    v:  &'a RwLock<T>,
    _p: PhantomData<*const ()>,
}
/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by the [`write`] and [`try_write`] methods
/// on [`RwLock`].
///
/// [`write`]: RwLock::write
/// [`try_write`]: RwLock::try_write
pub struct RwLockWriteGuard<'a, T: ?Sized + 'a> {
    v:  &'a RwLock<T>,
    _p: PhantomData<*const ()>,
}

impl<T> RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[inline]
    pub const fn new(v: T) -> RwLock<T> {
        RwLock {
            w: MutantConstant::new(),
            r: MutantConstant::new(),
            c: AtomicUsize::new(0),
            d: UnsafeCell::new(v),
        }
    }

    /// Replaces the contained value with `value`, and returns the old contained
    /// value.
    ///
    /// # Errors
    ///
    /// This function will return an error containing the provided `value` if
    /// the `RwLock` is poisoned. An `RwLock` is poisoned whenever a writer
    /// panics while holding an exclusive lock.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.replace(11).unwrap(), 7);
    /// assert_eq!(lock.get_cloned().unwrap(), 11);
    /// ```
    #[inline]
    pub fn replace(&self, value: T) -> LockResult<T> {
        Ok(replace(
            &mut *unsafe { self.write().unwrap_unchecked() },
            value,
        ))
    }
    /// Sets the contained value.
    ///
    /// # Errors
    ///
    /// This function will return an error containing the provided `value` if
    /// the `RwLock` is poisoned. An `RwLock` is poisoned whenever a writer
    /// panics while holding an exclusive lock.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned().unwrap(), 7);
    /// lock.set(11).unwrap();
    /// assert_eq!(lock.get_cloned().unwrap(), 11);
    /// ```
    #[inline]
    pub fn set(&self, value: T) -> Result<(), PoisonError<T>> {
        if needs_drop::<T>() {
            return self.replace(value).map(drop);
        }
        *unsafe { self.write().unwrap_unchecked() } = value;
        Ok(())
    }
}
impl<T: Clone> RwLock<T> {
    /// Returns the contained value by cloning it.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `RwLock` is poisoned. An
    /// `RwLock` is poisoned whenever a writer panics while holding an exclusive
    /// lock.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned().unwrap(), 7);
    /// ```
    #[inline]
    pub fn get_cloned(&self) -> Result<T, PoisonError<()>> {
        Ok(unsafe { (*self.read().unwrap_unchecked()).clone() })
    }
}
impl<T: Sized> RwLock<T> {
    /// Consumes this `RwLock`, returning the underlying data.
    ///
    /// # Errors
    ///
    /// This function will return an error containing the underlying data if
    /// the `RwLock` is poisoned. An `RwLock` is poisoned whenever a writer
    /// panics while holding an exclusive lock. An error will only be returned
    /// if the lock would have otherwise been acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let lock = RwLock::new(String::new());
    /// {
    ///     let mut s = lock.write().unwrap();
    ///     *s = "modified".to_owned();
    /// }
    /// assert_eq!(lock.into_inner().unwrap(), "modified");
    /// ```
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        Ok(self.d.into_inner())
    }
}
impl<T: ?Sized> RwLock<T> {
    /// Clear the poisoned state from a lock.
    ///
    /// If the lock is poisoned, it will remain poisoned until this function is
    /// called. This allows recovering from a poisoned state and marking
    /// that it has recovered. For example, if the value is overwritten by a
    /// known-good value, then the lock can be marked as un-poisoned. Or
    /// possibly, the value could be inspected to determine if it is in a
    /// consistent state, and if so the poison is removed.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::{Arc, RwLock};
    /// use xrmt_stx::thread;
    ///
    /// let lock = Arc::new(RwLock::new(0));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_lock.write().unwrap();
    ///     panic!(); // the lock gets poisoned
    /// }).join();
    ///
    /// assert_eq!(lock.is_poisoned(), true);
    /// let guard = lock.write().unwrap_or_else(|mut e| {
    ///     **e.get_mut() = 1;
    ///     lock.clear_poison();
    ///     e.into_inner()
    /// });
    /// assert_eq!(lock.is_poisoned(), false);
    /// assert_eq!(*guard, 1);
    /// ```
    #[inline]
    pub fn clear_poison(&self) {}
    /// Returns a raw pointer to the underlying data.
    ///
    /// The returned pointer is always non-null and properly aligned, but it is
    /// the user's responsibility to ensure that any reads and writes through it
    /// are properly synchronized to avoid data races, and that it is not read
    /// or written through after the lock is dropped.
    #[inline]
    pub fn data_ptr(&self) -> *mut T {
        self.d.get()
    }
    /// Determines whether the lock is poisoned.
    ///
    /// If another thread is active, the lock can still become poisoned at any
    /// time. You should not trust a `false` value for program correctness
    /// without additional synchronization.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::{Arc, RwLock};
    /// use xrmt_stx::thread;
    ///
    /// let lock = Arc::new(RwLock::new(0));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_lock.write().unwrap();
    ///     panic!(); // the lock gets poisoned
    /// }).join();
    /// assert_eq!(lock.is_poisoned(), true);
    /// ```
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        false
    }
    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the `RwLock` mutably, no actual locking needs to
    /// take place -- the mutable borrow statically guarantees no locks exist.
    ///
    /// # Errors
    ///
    /// This function will return an error containing a mutable reference to
    /// the underlying data if the `RwLock` is poisoned. An `RwLock` is
    /// poisoned whenever a writer panics while holding an exclusive lock.
    /// An error will only be returned if the lock would have otherwise been
    /// acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(0);
    /// *lock.get_mut().unwrap() = 10;
    /// assert_eq!(*lock.read().unwrap(), 10);
    /// ```
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        Ok(self.d.get_mut())
    }
    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns. This method does not provide any guarantees with
    /// respect to the ordering of whether contentious readers or writers will
    /// acquire the lock first.
    ///
    /// Returns an RAII guard which will release this thread's shared access
    /// once it is dropped.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `RwLock` is poisoned. An
    /// `RwLock` is poisoned whenever a writer panics while holding an exclusive
    /// lock. The failure will occur immediately after the lock has been
    /// acquired. The acquired lock guard will be contained in the returned
    /// error.
    ///
    /// # Panics
    ///
    /// This function might panic when called if the lock is already held by the
    /// current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::{Arc, RwLock};
    /// use xrmt_stx::thread;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// thread::spawn(move || {
    ///     let r = c_lock.read();
    ///     assert!(r.is_ok());
    /// }).join().unwrap();
    /// ```
    #[inline]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        // Lock the read lock, this will block updates to 'c' from going through
        // until the write lock is free.
        let _ = self.r.lock(None);
        // Add a reader, return the previous count.
        let n = self.c.fetch_add(1, Ordering::AcqRel);
        // If a read lock is requested, only the first reader needs to request
        // the lock to prevent any writers.
        if n == 0 {
            // Lock the write lock. This will block until the write lock is free.
            let _ = self.w.lock(None);
            // The read lock is also locked while this blocks, so no new
            // readers can be added also.
        }
        // Unlock the read lock to allow more readers.
        // The write lock stays locked to prevent new writers.
        self.r.unlock();
        Ok(RwLockReadGuard { v: self, _p: PhantomData })
    }
    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `RwLock` is poisoned. An
    /// `RwLock` is poisoned whenever a writer panics while holding an exclusive
    /// lock. An error will be returned when the lock is acquired. The acquired
    /// lock guard will be contained in the returned error.
    ///
    /// # Panics
    ///
    /// This function might panic when called if the lock is already held by the
    /// current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut n = lock.write().unwrap();
    /// *n = 2;
    ///
    /// assert!(lock.try_read().is_err());
    /// ```
    #[inline]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        // Lock the read lock, this will block updates to 'c' from going through
        // until the write lock is free.
        let _ = self.r.lock(None);
        // Try to lock the write lock now. this is what we need. This will block
        // until 'c' is zero as write will get held until then.
        let _ = self.w.lock(None);
        Ok(RwLockWriteGuard { v: self, _p: PhantomData })
    }
    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the shared
    /// access when it is dropped.
    ///
    /// This function does not block.
    ///
    /// This function does not provide any guarantees with respect to the
    /// ordering of whether contentious readers or writers will acquire the
    /// lock first.
    ///
    /// # Errors
    ///
    /// This function will return the [`Poisoned`] error if the `RwLock` is
    /// poisoned. An `RwLock` is poisoned whenever a writer panics while holding
    /// an exclusive lock. `Poisoned` will only be returned if the lock would
    /// have otherwise been acquired. An acquired lock guard will be contained
    /// in the returned error.
    ///
    /// This function will return the [`WouldBlock`] error if the `RwLock` could
    /// not be acquired because it was already locked exclusively.
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// match lock.try_read() {
    ///     Ok(n) => assert_eq!(*n, 1),
    ///     Err(_) => unreachable!(),
    /// };
    /// ```
    #[inline]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        let h = unsafe { self.r.lock_handle(None) }.ok_or(TryLockError::WouldBlock)?;
        if self.c.load(Ordering::Acquire) > 0 {
            // 'h' would be dropped here.
            return Ok(RwLockReadGuard { v: self, _p: PhantomData });
        }
        let r = if self.w.lock(None) {
            let _ = self.c.fetch_add(1, Ordering::AcqRel);
            Ok(RwLockReadGuard { v: self, _p: PhantomData })
        } else {
            Err(TryLockError::WouldBlock)
        };
        // Free the read lock
        drop(h);
        r
    }
    /// Attempts to lock this `RwLock` with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then `Err` is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when
    /// it is dropped.
    ///
    /// This function does not block.
    ///
    /// This function does not provide any guarantees with respect to the
    /// ordering of whether contentious readers or writers will acquire the
    /// lock first.
    ///
    /// # Errors
    ///
    /// This function will return the [`Poisoned`] error if the `RwLock` is
    /// poisoned. An `RwLock` is poisoned whenever a writer panics while holding
    /// an exclusive lock. `Poisoned` will only be returned if the lock would
    /// have otherwise been acquired. An acquired lock guard will be contained
    /// in the returned error.
    ///
    /// This function will return the [`WouldBlock`] error if the `RwLock` could
    /// not be acquired because it was already locked exclusively.
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// assert!(lock.try_write().is_err());
    /// ```
    #[inline]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        if !self.r.lock(None) {
            return Err(TryLockError::WouldBlock);
        }
        if self.w.lock(None) {
            return Ok(RwLockWriteGuard { v: self, _p: PhantomData });
        }
        self.r.unlock();
        Err(TryLockError::WouldBlock)
    }
}
impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    /// Downgrades a write-locked `RwLockWriteGuard` into a read-locked
    /// [`RwLockReadGuard`].
    ///
    /// This method will atomically change the state of the [`RwLock`] from
    /// exclusive mode into shared mode. This means that it is impossible
    /// for a writing thread to get in between a thread calling `downgrade`
    /// and the same thread reading whatever it wrote while it had the
    /// [`RwLock`] in write mode.
    ///
    /// Note that since we have the `RwLockWriteGuard`, we know that the
    /// [`RwLock`] is already locked for writing, so this method cannot
    /// fail.
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(rwlock_downgrade)]
    /// use xrmt_stx::sync::{Arc, RwLock, RwLockWriteGuard};
    ///
    /// // The inner value starts as 0.
    /// let rw = Arc::new(RwLock::new(0));
    ///
    /// // Put the lock in write mode.
    /// let mut main_write_guard = rw.write().unwrap();
    ///
    /// let evil = rw.clone();
    /// let handle = xrmt_stx::thread::spawn(move || {
    ///     // This will not return until the main thread drops the `main_read_guard`.
    ///     let mut evil_guard = evil.write().unwrap();
    ///
    ///     assert_eq!(*evil_guard, 1);
    ///     *evil_guard = 2;
    /// });
    ///
    /// // After spawning the writer thread, set the inner value to 1.
    /// *main_write_guard = 1;
    ///
    /// // Atomically downgrade the write guard into a read guard.
    /// let main_read_guard = RwLockWriteGuard::downgrade(main_write_guard);
    ///
    /// // Since `downgrade` is atomic, the writer thread cannot have set the inner value to 2.
    /// assert_eq!(*main_read_guard, 1, "`downgrade` was not atomic");
    ///
    /// // Clean up everything now
    /// drop(main_read_guard);
    /// handle.join().unwrap();
    ///
    /// let final_check = rw.read().unwrap();
    /// assert_eq!(*final_check, 2);
    /// ```
    #[inline]
    pub fn downgrade(s: RwLockWriteGuard<'a, T>) -> RwLockReadGuard<'a, T> {
        let v = s.v;
        // Prevent unlocking the write lock.
        forget(s);
        // Add one to the reader count.
        let _ = v.c.fetch_add(1, Ordering::AcqRel);
        // Unlock the read lock, the write lock stays locked to prevent new writers.
        v.r.unlock();
        RwLockReadGuard { v, _p: PhantomData }
    }
}

impl<T> From<T> for RwLock<T> {
    #[inline]
    fn from(v: T) -> RwLock<T> {
        RwLock::new(v)
    }
}
impl<T: Default> Default for RwLock<T> {
    /// Creates a new `RwLock<T>`, with the `Default` value for T.
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}
impl<T: ?Sized> UnwindSafe for RwLock<T> {}
impl<T: ?Sized + Debug> Debug for RwLock<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(unsafe { &*self.d.get() }, f)
    }
}
impl<T: ?Sized> RefUnwindSafe for RwLock<T> {}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        let _ = self.v.r.lock(None);
        if self.v.c.fetch_sub(1, Ordering::AcqRel) <= 1 {
            // Free the write lock if no more readers are present.
            self.v.w.unlock();
        }
        self.v.r.unlock();
    }
}
impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.v.d.get() }
    }
}
impl<T: ?Sized + Debug> Debug for RwLockReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(unsafe { &*self.v.d.get() }, f)
    }
}
impl<'a, T: ?Sized> UnwindSafe for RwLockWriteGuard<'a, T> {}
impl<T: ?Sized + Display> Display for RwLockReadGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(unsafe { &*self.v.d.get() }, f)
    }
}
impl<'a, T: ?Sized + RefUnwindSafe> RefUnwindSafe for RwLockReadGuard<'a, T> {}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.v.r.unlock();
        self.v.w.unlock();
    }
}
impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.v.d.get() }
    }
}
impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.v.d.get() }
    }
}
impl<T: ?Sized + Debug> Debug for RwLockWriteGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(unsafe { &*self.v.d.get() }, f)
    }
}
impl<T: ?Sized + Display> Display for RwLockWriteGuard<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(unsafe { &*self.v.d.get() }, f)
    }
}
impl<'a, T: ?Sized> RefUnwindSafe for RwLockWriteGuard<'a, T> {}
impl<'a, T: ?Sized + RefUnwindSafe> UnwindSafe for RwLockReadGuard<'a, T> {}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}
