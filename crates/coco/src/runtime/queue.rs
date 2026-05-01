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

extern crate alloc;
extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_stx;

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use core::convert::From;
use core::future::Future;
use core::hint::unlikely;
use core::iter::Iterator;
use core::mem::{drop, forget};
use core::num::NonZeroU32;
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use xrmt_bugtrack::bugtrack;
use xrmt_stx::abort_unlikely;
use xrmt_stx::io::{ErrorKind, IoError, IoResult};
use xrmt_stx::sync::extra::Event;
use xrmt_stx::sync::Mutex;
use xrmt_stx::time::extra::Time;

use crate::debug;
use crate::runtime::{Driver, Entry, EntryMap, EntryPointer, EntryReference, PollResult, QueueResult, Thread, ThreadPool};

pub struct Queue<'a> {
    v: Mutex<QueueInner<'a>>,
    c: AtomicU8,
}

struct QueueInner<'a> {
    map:     EntryMap<'a>,
    pool:    ThreadPool,
    driver:  Driver,
    backlog: VecDeque<EntryPointer<'a>>,
    entries: Vec<EntryPointer<'a>>,
}

impl<'a> Queue<'a> {
    const MAX: usize = 0xFFusize;

    #[inline]
    pub fn new() -> IoResult<(Event, Queue<'a>)> {
        if unlikely(Driver::SIZE > Queue::MAX) {
            return Err(IoError::from(ErrorKind::QuotaExceeded));
        }
        let e = Event::new()?;
        let d = Driver::new(&e)?;
        Ok((e, Queue {
            c: AtomicU8::new(0u8),
            v: Mutex::new(QueueInner {
                map:     EntryMap::new(),
                pool:    ThreadPool::new(),
                driver:  d,
                backlog: VecDeque::new(),
                entries: Vec::new(),
            }),
        }))
    }

    #[inline]
    pub fn close(&mut self) {
        let mut i = abort_unlikely!(self.v.lock());
        let _ = i.driver.signals();
        // TODO(dij): ^
        i.pool.close();
        i.backlog.clear();
        i.entries.clear();
        forget(i); // Keep the lock.
    }
    #[inline]
    pub fn set_theads_max(&mut self, v: u8) {
        abort_unlikely!(self.v.get_mut()).pool.set_max(v);
    }
    #[inline]
    pub fn set_theads_initial(&mut self, v: u8) {
        abort_unlikely!(self.v.get_mut()).pool.set_initial(v);
    }
    #[inline]
    pub fn threads(&mut self) -> Option<&[Thread]> {
        abort_unlikely!(self.v.get_mut()).pool.threads()
    }
    #[inline]
    pub fn pop(&self, sig: &Event, dirty: &AtomicBool, active: &AtomicBool) -> QueueResult<'a> {
        // Check if we're still active before doing anything.
        if !active.load(Ordering::Acquire) {
            return QueueResult::Shutdown;
        }
        // Clear the signal flag, since we're here to pull an Entry.
        bugtrack!("(Queue).pop(): Waiting on lock..");
        //
        debug!("(Queue).pop(): Waiting on lock..");
        //
        // Increment our "lock tracker" to see how many are waiting.
        self.c.fetch_add(1, Ordering::Release);
        // Lock the queue and backlog, since we're editing the contents.
        let mut i = abort_unlikely!(self.v.lock());
        // Now decrement the "lock tracker" since we are working..
        self.c.fetch_sub(1, Ordering::Release);
        bugtrack!("(Queue).pop(): Received lock!");
        // Clear any pending signals.
        sig.reset();
        i.pop(dirty, self.c.load(Ordering::Acquire))
    }
    #[inline]
    pub fn add<F: Future<Output = ()> + 'a>(&self, sig: &Event, dirty: &AtomicBool, f: F) -> EntryReference<'a> {
        bugtrack!("(Queue).add(): Waiting on lock..");
        // Clear the signal flag, since we're here to pull it out.
        sig.reset();
        sig.set();
        // Lock the queue and backlog, since we're editing the contents.
        let mut i = abort_unlikely!(self.v.lock());
        bugtrack!("(Queue).add(): Received lock!");
        //
        debug!("(Queue).add(): Got lock!");
        //
        let p = i.add(dirty, f);
        bugtrack!("(Queue).add(): Making driver aware of new Event!");
        // Signal the new Event is added.
        sig.set();
        p
    }
}
impl<'a> QueueInner<'a> {
    #[inline]
    fn poll(&mut self) -> PollResult {
        // Get the smallest time we need to wait before a wake occurs.
        let d = self.timeout();
        bugtrack!("(Controller).poll(): Polling Driver with timeout {d:?}..");
        // Actually poll the Driver.
        self.driver.poll(&mut self.map, &mut self.entries, d)
    }
    fn sort(&mut self, dirty: &AtomicBool) {
        // Check if there's work to do.
        if !dirty.load(Ordering::Acquire) {
            return;
        }
        bugtrack!("(QueueInner).sort(): Starting prune and sort..");
        // Remove any completed Events.
        for v in self.entries.extract_if(0.., |v| v.is_done()) {
            bugtrack!("(QueueInner).sort(): Removing completed {v:?}.");
            drop(v);
        }
        // Add any new Events to the backlog.
        if self.backlog.len() > 0 {
            // TODO(dij): We could allow more than the Driver size, since not
            //            every Entry gets a Driver slot anyway. (ie: sleep/dur
            //            entries don't have a Driver entry). And would allow us
            //            to ease backlog pressure.
            bugtrack!("(QueueInner).sort(): Backlog is not empty, attempting to add more..");
            while self.entries.len() < Driver::SIZE {
                match self.backlog.pop_front() {
                    Some(v) => {
                        bugtrack!("(QueueInner).sort(): Adding new {v:?}!");
                        self.entries.push(v)
                    },
                    None => break,
                }
            }
        }
        // Now resort the Driver event queue
        bugtrack!("(QueueInner).sort(): Queue is dirty, resorting..");
        // Sort by wake time, so the first in queue will have the sortest time.
        self.entries.sort_unstable();
        // Update the driver Entry list.
        let n = self.driver.update(&mut self.map, &mut self.entries);
        // Set the Driver Map len. (How many Entries are backed by the Driver).
        self.map.set_len(n);
        // Clear the dirty flag.
        dirty.store(false, Ordering::Release);
    }
    #[inline]
    fn timeout(&self) -> Option<NonZeroU32> {
        // Update the internal trigger time.
        let n = Time::now();
        match self.entries.len() {
            // No entries
            0 => None,
            // Fastpath, only have one entry, set time from that. This
            // might return None if it's zero.
            1 => unsafe { self.entries.get_unchecked(0).timeout(&n) },
            // Fastpath, since we sort via wake time, the first non-zero is the smallest.
            // This would technically be the default since we'll always sort the
            // Entry list.
            _ => self
                .entries
                .iter()
                .find(|v| !v.is_zero())
                .map(|v| v.timeout(&n))
                .unwrap_or(None),
        }
    }
    #[inline]
    fn check(&mut self) -> Option<EntryReference<'a>> {
        let n = Time::now();
        // Compare all Entries to the current time and mark any as ready, but
        // return the first Entry that is Ready (if any).
        self.entries
            .iter_mut()
            .position(|v| v.is_ready_timeout(&n))
            .map(|v| unsafe { self.entries.get_unchecked_mut(v).as_ptr() })
        // ^ We have to take it like this as the borrow checker will be confused
        // about the lifetime of the Entry. It also allows us to avoid bounds
        // checks. Since the Entry is in a Box and the Vec just contains
        // pointers, the Entry will still be valid, even if it moves.
    }
    fn pop(&mut self, dirty: &AtomicBool, w: u8) -> QueueResult<'a> {
        // Try to pull the next Entry in the list that's active, break if not.
        // Special return the 'QueueResult::Empty' value if the queue is empty
        // and no entries are also in the backlog.
        let e = match self.next(dirty, w) {
            Some(v) => unsafe { &mut *v.as_ptr() },
            None if self.entries.is_empty() => return QueueResult::Empty,
            None => return QueueResult::None,
        };
        bugtrack!("(QueueInner).pop(): next() returned {e:?}.");
        let _ = e.set_ready(); // For Windows as the Entry won't be manually set, unless it's IOCP.
                               //
        debug!("next(): pop() Entry {i}={e:?}");
        // Check if this Entry is even valid for us to use.
        if unlikely(!e.is_avaliable()) {
            bugtrack!(r"(QueueInner).pop(): {e:?} was returned but is not avaliable!.");
            core::panic!(); // WUT
        }
        // While locked, prepare the Entry, mark it as running and NULL the
        // Entry from the Driver list. The State object will hold a lock and pointer to
        // the Entry for us to use without annoying the borrow checker.
        match e.lock(&mut self.driver) {
            Some(v) => QueueResult::Entry(v),
            None => QueueResult::None,
        }
    }
    #[inline]
    fn next(&mut self, dirty: &AtomicBool, w: u8) -> Option<EntryReference<'a>> {
        // Try to sort if the dirty flag is true.
        self.sort(dirty);
        // Quick path, is there even anything in the queue?
        if self.entries.is_empty() {
            bugtrack!("(QueueInner).next(): Queue is empty!");
            return None;
        }
        // Check if we need additional Threads to handle Entries.
        // Pass the Thread waiting count also to lift wight if there's none waiting.
        self.pool.check(self.entries.len(), w);
        // Did anyone timeout while we waited?
        if let Some(v) = self.check() {
            return Some(v);
        }
        // Poll the driver to see if anything was updated?
        match self.poll() {
            // If we get a Signal or Wake event, treat it as a high priority issue
            // and don't check for other events.
            //
            // This just translates to a break in the calling functions and will
            // cause the queue to be re-evaluated.
            #[cfg(any(
                target_os = "netbsd",
                target_os = "freebsd",
                target_os = "openbsd",
                target_os = "dragonfly",
                target_vendor = "apple",
                target_family = "windows"
            ))] // KQueue/IOCP only
            PollResult::Pointer(v) => Some(unsafe { core::mem::transmute(v) }),
            // Use the Driver-backed list to get the Entry instead.
            PollResult::Entry(v) => self.map.get(v),
            PollResult::Signal | PollResult::Wake => None, // Event was signaled.
            // If the driver returned nothing, check again for anyone that might
            // have a timeout during the poll check.
            PollResult::None => self.check(),
        }
    }
    #[inline]
    fn add<F: Future<Output = ()> + 'a>(&mut self, dirty: &AtomicBool, f: F) -> EntryReference<'a> {
        let mut e = Box::new(Entry::new(f));
        let p = e.as_ptr();
        bugtrack!("(QueueInner).add(): Adding Entry {e:?} to backlog..");
        self.backlog.push_back(e);
        bugtrack!("(QueueInner).add(): Marking queue dirty..");
        dirty.store(true, Ordering::Release);
        bugtrack!("(QueueInner).add(): Add complete!");
        p
    }
}
