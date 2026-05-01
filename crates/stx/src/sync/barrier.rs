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

use core::fmt::{Debug, Formatter};
use core::panic::RefUnwindSafe;
use core::sync::atomic::{fence, Ordering};

use crate::io::FmtResult;
use crate::sync::extra::EventConstant;
use crate::sync::Mutex;

/// A barrier enables multiple threads to synchronize the beginning
/// of some computation.
///
/// # Examples
///
/// ```
/// use xrmt_stx::sync::Barrier;
/// use xrmt_stx::thread;
///
/// let n = 10;
/// let barrier = Barrier::new(n);
/// thread::scope(|s| {
///     for _ in 0..n {
///         // The same messages will be printed together.
///         // You will NOT see any interleaving.
///         s.spawn(|| {
///             println!("before wait");
///             barrier.wait();
///             println!("after wait");
///         });
///     }
/// });
/// ```
#[cfg_attr(not(feature = "strip"), derive(Debug))]
pub struct Barrier {
    e:     EventConstant,
    lock:  Mutex<BarrierEntry>,
    limit: usize,
}
/// A `BarrierWaitResult` is returned by [`Barrier::wait()`] when all threads
/// in the [`Barrier`] have rendezvoused.
///
/// # Examples
///
/// ```
/// use xrmt_stx::sync::Barrier;
///
/// let barrier = Barrier::new(1);
/// let barrier_wait_result = barrier.wait();
/// ```
pub struct BarrierWaitResult(bool);

#[cfg_attr(not(feature = "strip"), derive(Debug))]
struct BarrierEntry {
    cur:  usize,
    wait: usize,
}

impl Barrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// A barrier will block `n`-1 threads which call [`wait()`] and then wake
    /// up all threads at once when the `n`th thread calls [`wait()`].
    ///
    /// [`wait()`]: Barrier::wait
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Barrier;
    ///
    /// let barrier = Barrier::new(10);
    /// ```
    #[inline]
    pub const fn new(n: usize) -> Barrier {
        Barrier {
            e:     EventConstant::new(),
            lock:  Mutex::new(BarrierEntry::new()),
            limit: n,
        }
    }

    /// Blocks the current thread until all threads have rendezvoused here.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once, and can
    /// be used continuously.
    ///
    /// A single (arbitrary) thread will receive a [`BarrierWaitResult`] that
    /// returns `true` from [`BarrierWaitResult::is_leader()`] when returning
    /// from this function, and all other threads will receive a result that
    /// will return `false` from [`BarrierWaitResult::is_leader()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Barrier;
    /// use xrmt_stx::thread;
    ///
    /// let n = 10;
    /// let barrier = Barrier::new(n);
    /// thread::scope(|s| {
    ///     for _ in 0..n {
    ///         // The same messages will be printed together.
    ///         // You will NOT see any interleaving.
    ///         s.spawn(|| {
    ///             println!("before wait");
    ///             barrier.wait();
    ///             println!("after wait");
    ///         });
    ///     }
    /// });
    /// ```
    #[inline]
    pub fn wait(&self) -> BarrierWaitResult {
        // SAFETY: This will never be Err
        let s = unsafe { self.lock.lock().unwrap_unchecked().wait(self.limit) };
        fence(Ordering::SeqCst);
        if s {
            self.e.set();
        } else {
            self.e.wait();
        }
        BarrierWaitResult(s)
    }
}
impl BarrierEntry {
    #[inline]
    const fn new() -> BarrierEntry {
        BarrierEntry { cur: 0usize, wait: 0usize }
    }

    #[inline]
    fn wait(&mut self, limit: usize) -> bool {
        self.cur += 1;
        if self.cur < limit {
            return false;
        }
        self.cur = 0;
        self.wait += 1;
        true
    }
}
impl BarrierWaitResult {
    /// Returns `true` if this thread is the "leader thread" for the call to
    /// [`Barrier::wait()`].
    ///
    /// Only one thread will have `true` returned from their result, all other
    /// threads will have `false` returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::Barrier;
    ///
    /// let barrier = Barrier::new(1);
    /// let barrier_wait_result = barrier.wait();
    /// println!("{:?}", barrier_wait_result.is_leader());
    /// ```
    #[inline]
    pub fn is_leader(&self) -> bool {
        self.0
    }
}

impl RefUnwindSafe for Barrier {}

impl Debug for BarrierWaitResult {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(&self.0, f)
    }
}
