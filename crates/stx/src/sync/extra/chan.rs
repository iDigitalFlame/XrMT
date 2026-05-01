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

extern crate alloc;
extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_winapi;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::convert::AsMut;
use core::debug_assert;
use core::marker::Sync;
use core::mem::{drop, replace, MaybeUninit};
use core::option::Option::{self, None, Some};
use core::ptr::copy;
use core::result::Result::{self, Err, Ok};
use core::slice::from_raw_parts;
use core::time::Duration;

use xrmt_bugtrack::bugtrack;
use xrmt_winapi::functions::wait_on_multiple;

use crate::sync::extra::{KeySignal, Mutant, SignalAuto, SignalManual, ZERO};

const KEY_READY: usize = 0x800usize;
const KEY_LISTEN: usize = 0x400usize;

pub struct TimeoutError(());
pub struct FullError<T>(pub T);
pub struct Carrier<T>(Mailbox<T>);

enum Mailbox<T> {
    List(BoxList<T>),
    Array(BoxArray<T>),
    Single(BoxSingle<T>),
}

struct Array<T> {
    end:   usize,
    data:  Box<[MaybeUninit<T>]>,
    start: usize,
}
#[repr(C)]
struct BoxList<T> {
    lock:  Mutant,
    ready: SignalManual,
    data:  VecDeque<T>,
}
#[repr(C)]
struct BoxArray<T> {
    send: SignalManual,
    lock: Mutant,
    recv: SignalManual,
    data: Array<T>,
}
struct BoxSingle<T> {
    key:   KeySignal,
    ptr:   Box<MaybeUninit<T>>,
    lock:  Mutant,
    ready: SignalAuto,
}

impl<T> Array<T> {
    #[inline]
    fn new(len: usize) -> Array<T> {
        Array {
            end:   0usize,
            data:  Box::new_uninit_slice(len),
            start: 0usize,
        }
    }

    fn sort(&mut self) {
        bugtrack!(
            "(Array).sort(): Sort start: start={}, end={}, data.len()={}",
            self.start,
            self.end,
            self.data.len()
        );
        let n = self.end.saturating_sub(self.start);
        debug_assert!(n <= self.data.len());
        if n > 0 {
            // Don't waste a copy if nothing is in the buffer.
            unsafe {
                copy(
                    self.data.as_ptr().add(self.start),
                    self.data.as_mut_ptr(),
                    n,
                );
            }
        }
        (self.start, self.end) = (0, n);
        bugtrack!("(Array).sort(): Sort completed, start=0, end={n}");
    }
    #[inline]
    fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    #[inline]
    fn is_full(&self) -> bool {
        self.end == self.data.len() && self.start == 0
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.start == self.end
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.data.len()
    }
    #[inline]
    fn push(&mut self, v: T) -> Result<T, bool> {
        if self.end == self.data.len() && self.start == 0 {
            bugtrack!(
                "(Array).push(): Cannot push, Array is full, start={}, end={}, data.len()={}",
                self.start,
                self.end,
                self.data.len()
            );
            return Ok(v);
        }
        debug_assert!(self.end <= self.data.len() && self.start <= self.end);
        bugtrack!(
            "(Array).push(): Pre-sort? start={}, end={}, data.len()={}",
            self.start,
            self.end,
            self.data.len()
        );
        if self.end >= self.data.len() && self.start > 0 {
            self.sort();
            bugtrack!(
                "(Array).push(): Post-sort? start={}, end={}, data.len()={}",
                self.start,
                self.end,
                self.data.len()
            );
        }
        match self.data.get_mut(self.end) {
            Some(i) => {
                i.write(v);
            },
            _ => return Ok(v),
        }
        bugtrack!(
            "(Array).push(): Push successful! New item at {}, end={}",
            self.end,
            self.end + 1
        );
        self.end += 1;
        // This is Err instead of Ok, so we can use the "ok()" helper.
        Err(self.end < self.data.len() || self.start > 0)
    }
    #[inline]
    fn pop(&mut self) -> Option<(T, bool, bool)> {
        if self.start == self.end {
            bugtrack!(
                "(Array).push(): Cannot pop, Array is empty, start={}, end={}, data.len()={}",
                self.start,
                self.end,
                self.data.len()
            );
            return None;
        }
        debug_assert!(self.end <= self.data.len() && self.start <= self.end);
        let r = unsafe {
            MaybeUninit::assume_init(replace(
                self.data.get_unchecked_mut(self.start),
                MaybeUninit::uninit(),
            ))
        };
        bugtrack!(
            "(Array).push(): Pop successful! Retrived item at {}, start={}",
            self.start,
            self.start + 1
        );
        self.start += 1;
        bugtrack!(
            "(Array).pop(): Pre-sort? start={}, end={}, data.len()={}",
            self.start,
            self.end,
            self.data.len()
        );
        if self.start >= self.data.len() {
            self.sort();
            bugtrack!(
                "(Array).pop(): Post-sort? start={}, end={}, data.len()={}",
                self.start,
                self.end,
                self.data.len()
            );
        }
        Some((
            r,
            self.start < self.end,
            self.end < self.data.len() || self.start > 0,
        ))
    }
}
impl<T> Carrier<T> {
    #[inline]
    pub fn new_list() -> Carrier<T> {
        Carrier(Mailbox::List(BoxList::new()))
    }
    #[inline]
    pub fn new_single() -> Carrier<T> {
        Carrier(Mailbox::Single(BoxSingle::new()))
    }
    #[inline]
    pub fn new_array(len: usize) -> Carrier<T> {
        // NOTE(dij): I want to put a pin in here for a thought I had when making
        //            this.
        //            The current implementation here works /exactly/ like the
        //            stdlib implementartion would work.
        //              ie: [`channel`]         = BoxList
        //                  [`sync_channel(0)`] = BoxSingle
        //                  [`sync_channel(N)`] = BoxArray<N>
        //                          if N > 256? = BoxList
        //            While this makes sense, is it common to have a [`sync_channel`]
        //            with a large size? Wouldn't this allocate alot of memory??
        //            I checked their code, it's basically the same as I use
        //            which is a `Box<[T]>` or an array on the heap backed by a
        //            [`Box`]. Technically larger numbers would benifit from using
        //            a BoxList on the backend if N > (some_large_number) to prevent
        //            directly allocating all that memory. Or maybe we can work
        //            on a way to do it easier?
        // TODO(dij): Maybe don't pre-allocate the array and only allocate it in
        //            chunks? or something to grow as we add items into the list?
        Carrier(Mailbox::Array(BoxArray::new(len)))
    }

    #[inline]
    pub fn len(&self) -> usize {
        match &self.0 {
            Mailbox::List(c) => c.data.len(),
            Mailbox::Array(c) => c.data.len(),
            Mailbox::Single(_) => 0,
        }
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        match &self.0 {
            Mailbox::List(_) => false,
            Mailbox::Array(c) => c.data.is_full(),
            Mailbox::Single(_) => true,
        }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Mailbox::List(c) => c.data.is_empty(),
            Mailbox::Array(c) => c.data.is_empty(),
            Mailbox::Single(_) => true,
        }
    }
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        match &self.0 {
            Mailbox::List(_) => None,
            Mailbox::Array(c) => Some(c.data.capacity()),
            Mailbox::Single(_) => Some(0),
        }
    }
    #[inline]
    pub fn send_try(&mut self, v: T) -> Result<(), FullError<T>> {
        match &mut self.0 {
            Mailbox::List(c) => c.write(v, ZERO),
            Mailbox::Array(c) => c.write(v, ZERO),
            Mailbox::Single(c) => c.write(v, ZERO),
        }
    }
    #[inline]
    pub fn recv(&mut self, dur: Option<Duration>) -> Result<T, TimeoutError> {
        match &mut self.0 {
            Mailbox::List(c) => c.read(dur),
            Mailbox::Array(c) => c.read(dur),
            Mailbox::Single(c) => c.read(dur),
        }
    }
    #[inline]
    pub fn send(&mut self, v: T, dur: Option<Duration>) -> Result<(), FullError<T>> {
        match &mut self.0 {
            Mailbox::List(c) => c.write(v, dur),
            Mailbox::Array(c) => c.write(v, dur),
            Mailbox::Single(c) => c.write(v, dur),
        }
    }
}
impl<T> BoxList<T> {
    #[inline]
    fn new() -> BoxList<T> {
        BoxList {
            lock:  Mutant::new(false),
            data:  VecDeque::new(),
            ready: SignalManual::new(false),
        }
    }

    #[cold]
    fn read_cold(&mut self) -> T {
        bugtrack!("(BoxList).read_cold(): Moved to cold read to wait for a signal..");
        // No timeout here, loop until we can successfully pop an item.
        loop {
            if wait_on_multiple(self.signals(), true, None) > -1 {
                // pop will unlock the queue for us so we don't need too.
                if let Some(v) = self.pop() {
                    break v;
                }
            }
        }
    }
    #[inline]
    fn signals(&self) -> &[usize] {
        // Simple method to create a slice. Since BoxList is #[repr(C)] and
        // the Signal and Lock structs are just Handles, we can make this object
        // a slice instead of making one each time :D
        unsafe { from_raw_parts(self as *const BoxList<T> as *const usize, 2) }
    }
    #[inline]
    fn pop(&mut self) -> Option<T> {
        let v = self.data.pop_front();
        if self.data.is_empty() {
            bugtrack!("(BoxList).pop(): Clearing the recv signal as the queue is empty!");
            self.ready.clear(); // Clear recv signal if the queue is empty.
        }
        self.lock.unlock(); // Drop the lock
        v
    }
    /// Read for BoxList is also very simple. Mainly it's just a signal with
    /// a lock to protect against data contention. If we can't get the lock or
    /// the signal is not set, we'll block (if no timeout) in a loop, otherwise
    /// we'll fail with a [`RecvTimeoutError`]
    #[inline]
    fn read(&mut self, dur: Option<Duration>) -> Result<T, TimeoutError> {
        bugtrack!("(BoxList).read(): Waiting on lock and recv signal..");
        // 1. Wait on lock and recv signal.
        if wait_on_multiple(self.signals(), true, dur) == -1 {
            // 1a. We're not supposed to block, return a timeout.
            if dur.is_some() {
                return Err(TimeoutError(()));
            }
            // 1a. Fallback if we need to block.
            return Ok(self.read_cold()); // Block
        }
        // Technically here, there's no way that the queue should be empty. But!
        // if it does, we'll timeout or block.
        bugtrack!("(BoxList).read(): Lock and recv signal received!");
        // 2. Pop the data value.
        match self.pop() {
            // ^ This call unlocks the queue.
            Some(v) => Ok(v),
            None if dur.is_some() => Err(TimeoutError(())), // Timeout
            None => Ok(self.read_cold()),                   // Block
        }
    }
    /// Write for BoxList is pretty simple. This function will only
    /// fail with [`TrySendError`] if obtaining the lock in the specified
    /// timeframe fails, which would be rare and likely a bug.
    #[inline]
    fn write(&mut self, v: T, dur: Option<Duration>) -> Result<(), FullError<T>> {
        bugtrack!("(BoxList).write(): Waiting for queue lock..");
        // 1. Obtain the queue lock.
        let g = match unsafe { self.lock.lock_handle(dur) } {
            None => return Err(FullError(v)),
            Some(h) => h,
        };
        bugtrack!("(BoxList).write(): Received queue lock!");
        // 2. Push the value into the back of the queue.
        self.data.push_back(v);
        bugtrack!("(BoxList).write(): Signaling all Receivers.");
        // 3. Signal to all Receivers that there is data avaliable.
        self.ready.set();
        drop(g); // Explicitly drop the guard here.
        Ok(())
    }
}
impl<T> BoxArray<T> {
    #[inline]
    fn new(len: usize) -> BoxArray<T> {
        BoxArray {
            data: Array::new(len),
            lock: Mutant::new(false),
            recv: SignalManual::new(false),
            send: SignalManual::new(true),
        }
    }

    #[inline]
    fn pop(&mut self) -> Option<T> {
        let r = self.data.pop();
        match r.as_ref() {
            Some((_, d, s)) => {
                bugtrack!("(BoxArray).pop(): Pop succeeded! (free={s}, avaliable={d})");
                if !*d {
                    self.recv.clear(); // Clear recv signal if the queue is
                                       // empty.
                }
                if *s {
                    self.send.set(); // Indicate that there is space avaliable
                }
            },
            // Clear recv signal if the queue is empty.
            None => self.recv.clear(),
        }
        self.lock.unlock(); // Clear any locks
        r.map(|v| v.0) // Remove everything but the value
    }
    #[inline]
    fn signals_recv(&self) -> &[usize] {
        // See BoxList.signals()
        // Advance 1 usize to get "lock/recv"
        unsafe { from_raw_parts((self as *const BoxArray<T> as *const usize).add(1), 2) }
    }
    #[inline]
    fn signals_send(&self) -> &[usize] {
        // See BoxList.signals()
        unsafe { from_raw_parts(self as *const BoxArray<T> as *const usize, 2) }
    }
    #[inline]
    fn push(&mut self, v: T) -> Option<T> {
        let r = self.data.push(v);
        match r.as_ref() {
            Err(x) if *x => {
                bugtrack!("(BoxArray).push(): Array has free space, setting send signal..");
                self.send.set(); // Indicate that free space is avaliable
            },
            _ => {
                bugtrack!("(BoxArray).push(): Array is full, clearing send signal..");
                self.send.clear(); // Indicate that no free space is avaliable
            },
        }
        // The result is Err when it passes and Ok when failed.
        if !r.is_ok() {
            bugtrack!("(BoxArray).push(): Push successful, setting recv signal!");
            self.recv.set(); // Let Receivers know data is avaliable
        }
        self.lock.unlock(); // Clear any locks
        r.ok() // Returns None if the push was successful.
    }
    #[inline]
    fn read(&mut self, dur: Option<Duration>) -> Result<T, TimeoutError> {
        bugtrack!("(BoxArray).read(): Waiting on lock and recv signal..");
        // 1. Wait on lock and recv signal.
        if wait_on_multiple(self.signals_recv(), true, dur) == -1 {
            // No need to verify if we need to block as a None dur will block
            // until the Lock is ready.
            return Err(TimeoutError(()));
        }
        bugtrack!("(BoxArray).read(): Lock and recv signal received!");
        match self.pop() {
            Some(v) => {
                bugtrack!("(BoxArray).read(): Pop succeeded!");
                Ok(v)
            },
            None => {
                bugtrack!("(BoxArray).write(): Moved to cold read to wait for data..");
                self.read_cold(dur)
            },
        }
    }
    #[cold]
    fn read_cold(&mut self, dur: Option<Duration>) -> Result<T, TimeoutError> {
        // 1. Try once as we might have a duration, then loop if so. If wait returns
        //    'false' and dur is Some, than return RecvTimeoutError.
        // 2. Wait till the recv signal is ready and we have the lock
        if wait_on_multiple(self.signals_recv(), true, dur) == -1 {
            if dur.is_some() {
                return Err(TimeoutError(())); // Bail
            }
            return self.read_cold(None); // Block
        }
        // 3. Attempt to pop an entry if we can.
        match self.pop() {
            Some(v) => Ok(v),
            None if dur.is_some() => Err(TimeoutError(())), // Bail
            None => self.read_cold(None),                   // Block
        }
    }
    #[inline]
    fn write(&mut self, v: T, dur: Option<Duration>) -> Result<(), FullError<T>> {
        bugtrack!("(BoxArray).write(): Waiting on lock and send signal..");
        // 1. Wait on lock and send signal.
        if wait_on_multiple(self.signals_send(), true, dur) == -1 {
            // No need to verify if we need to block as a None dur will block
            // until the Lock is ready.
            return Err(FullError(v));
        }
        bugtrack!("(BoxArray).write(): Lock and send signal received!");
        // 2. Attempt to push an item into the array, re-sorting it if needed.
        match self.push(v) {
            // ^ This call free's the Lock.
            Some(x) => {
                bugtrack!("(BoxArray).write(): Moved to cold write to wait for a free space..");
                self.write_cold(x, dur)
            },
            None => {
                bugtrack!("(BoxArray).write(): Push succeeded!");
                Ok(())
            },
        }
    }
    #[cold]
    fn write_cold(&mut self, v: T, dur: Option<Duration>) -> Result<(), FullError<T>> {
        // 1. Try once as we might have a duration, then loop if so. If wait returns
        //    'false' and dur is Some, than return TrySendError.
        // 2. Wait till the send signal is ready and we have the lock
        if wait_on_multiple(self.signals_send(), true, dur) == -1 {
            if dur.is_some() {
                return Err(FullError(v)); // Bail
            }
            return self.write_cold(v, None); // Block
        }
        bugtrack!("(BoxArray).write_cold(): Lock and send signal received!");
        // 3. Attempt to push an entry if we can.
        match self.push(v) {
            Some(x) if dur.is_some() => Err(FullError(x)), // Bail
            Some(x) => self.write_cold(x, None),           // Block
            None => Ok(()),
        }
    }
}
impl<T> BoxSingle<T> {
    #[inline]
    fn new() -> BoxSingle<T> {
        BoxSingle {
            key:   KeySignal::new(),
            ptr:   Box::new_uninit(),
            lock:  Mutant::new(false),
            ready: SignalAuto::new(false),
        }
    }

    #[inline]
    fn read(&mut self, dur: Option<Duration>) -> Result<T, TimeoutError> {
        // 1. Set sync flag
        self.ready.set();
        // 2. Signal and wait to indicate the Receiver is ready.
        bugtrack!("(BoxSingle).read(): Waiting for listen event..");
        if !self.key.set(KEY_LISTEN, dur) {
            return Err(TimeoutError(()));
        }
        bugtrack!("(BoxSingle).read(): Listen event received! Waiting for data ready event!");
        // 3. Wait for the Sender to place data in the buffer.
        if !self.key.wait(KEY_READY, None) {
            return Err(TimeoutError(()));
        }
        bugtrack!("(BoxSingle).read(): Data ready event received, reading data!");
        // 4. Transfer data using pointer packet, replacing with uninit
        Ok(unsafe { replace(self.ptr.as_mut(), MaybeUninit::uninit()).assume_init() })
    }
    fn write(&mut self, v: T, dur: Option<Duration>) -> Result<(), FullError<T>> {
        // 1. Set sync flag
        self.ready.set();
        // 2. Lock sender to make sure only one sends at a time.
        bugtrack!("(BoxSingle).write(): Waiting on lock..");
        let g = match unsafe { self.lock.lock_handle(dur) } {
            None => return Err(FullError(v)),
            Some(v) => v,
        };
        // 3. Check to see if a Receiver is waiting.
        bugtrack!("(BoxSingle).write(): Lock received, waiting for listen event..");
        if !self.key.wait(KEY_LISTEN, dur) {
            return Err(FullError(v));
        }
        bugtrack!("(BoxSingle).write(): Listen event received, writing data to buffer..");
        // 4. Write the data to the shared buffer.
        self.ptr.write(v);
        // 5. Signal that data is ready!
        bugtrack!("(BoxSingle).write(): Setting data ready event..");
        let _ = self.key.set(KEY_READY, None);
        bugtrack!("(BoxSingle).write(): Send complete!");
        // 6. Done!
        drop(g); // Explicitly drop the guard here.
        Ok(())
    }
}

unsafe impl<T> Sync for BoxList<T> {}
unsafe impl<T> Sync for BoxArray<T> {}
unsafe impl<T> Sync for BoxSingle<T> {}
