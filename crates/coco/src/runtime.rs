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
use core::future::Future;
use core::marker::{PhantomData, Sync};
use core::option::Option::{self, None, Some};
use core::result::Result::Ok;
use core::sync::atomic::{AtomicBool, Ordering};

use xrmt_bugtrack::bugtrack;
use xrmt_stx::abort_unlikely;
use xrmt_stx::io::IoResult;
use xrmt_stx::sync::extra::{Event, Lazy};
use xrmt_stx::thread::yield_now;

use crate::debug;
use crate::runtime::link::{Link, Ticket};
use crate::runtime::Queue;

mod container;
mod data;
mod entry;
mod link;
mod pool;
mod queue;
mod state;

pub use self::pool::Thread;
pub use self::state::*;

#[cfg(target_family = "windows")]
#[path = "./runtime/driver/windows.rs"]
mod driver;
#[cfg(not(target_family = "windows"))]
#[path = "./runtime/driver/unix.rs"]
mod driver;

pub(self) use self::container::*;
pub(self) use self::data::*;
pub(self) use self::driver::*;
pub(self) use self::entry::*;
pub(self) use self::pool::ThreadPool;
pub(self) use self::queue::*;

static RUNTIME: Controller<'static, 'static> = Controller::new();

pub struct Controller<'a, 'b>(Lazy<Box<Inner<'a, 'b>>>);

struct Inner<'a, 'b> {
    dirty:  AtomicBool,
    queue:  Queue<'b>,
    active: AtomicBool,
    signal: Event,
    _p:     PhantomData<&'a ()>,
}

impl<'a, 'b> Inner<'a, 'b> {
    #[inline]
    fn new() -> IoResult<Inner<'a, 'b>> {
        let (e, q) = Queue::new()?;
        Ok(Inner {
            dirty:  AtomicBool::new(false),
            queue:  q,
            active: AtomicBool::new(true),
            signal: e,
            _p:     PhantomData,
        })
    }

    #[inline]
    fn close(mut self) {
        self.active.store(false, Ordering::Release);
        bugtrack!("(Controller).close(): Shutdown called, waiting on lock..");
        self.queue.close();
        bugtrack!("(Controller).shutdown(): Shutdown complete.");
    }
    #[inline]
    fn run_until_empty(&self) {
        while self.pop(true, None) {
            yield_now();
        }
    }
    fn run(&self, cond: &AtomicBool) {
        loop {
            self.run_until_empty();
            if !cond.load(Ordering::Acquire) || !self.active.load(Ordering::Acquire) {
                break;
            }
            self.signal.wait();
            if !cond.load(Ordering::Acquire) || !self.active.load(Ordering::Acquire) {
                break;
            }
        }
    }
    fn pop(&self, empty: bool, link: Option<&Link<'_>>) -> bool {
        let mut s = match self.queue.pop(&self.signal, &self.dirty, &self.active) {
            QueueResult::Entry(v) => v,
            QueueResult::Empty if empty => return false, // Tell the caller they can bail, no work.
            QueueResult::Shutdown => return false,       // Tell the caller to bail, we're shutting down
            _ => return true,                            // Tell the caller to continue checking.
        };
        //
        debug!("pop(): RUNNING STATE {s:?} ===========");
        //
        if let Some(v) = link {
            // Check if the entry matches, if it does, keep the Flag set, if not
            // reset it.
            v.pre(&s.entry);
        }
        //
        let r = s.run();
        //
        debug!("pop(): DONE STATE {s:?} r={r} ===========");
        //
        let v = link.is_some_and(|v| v.post(&s.entry));
        if r || v {
            // Mark the queue as dirty, requested by the Event.
            self.dirty.store(true, Ordering::Release);
            self.signal.set();
            // If the post check passed, bail out.
            if v {
                return false;
            }
        }
        true
    }
    #[inline]
    fn add<F: Future<Output = ()> + 'b>(&mut self, f: F) -> EntryReference<'b> {
        self.queue.add(&self.signal, &self.dirty, f)
    }
}
impl<'a, 'b> Controller<'a, 'b> {
    #[inline]
    const fn new() -> Controller<'a, 'b> {
        Controller(Lazy::new())
    }

    #[inline]
    pub fn run_one(&self) {
        let _ = self.get().pop(true, None);
    }
    #[inline]
    pub fn close(&mut self) {
        // 'take' resets the inner Lazy so it can be re-initialized if needed.
        if let Some(v) = unsafe { self.0.take() } {
            v.close(); // Will also drop 'v'.
        }
    }
    #[inline]
    pub fn run_until_empty(&self) {
        self.get().run_until_empty();
    }
    #[inline]
    pub fn set_theads_max(&self, v: u8) {
        self.get().queue.set_theads_max(v);
    }
    #[inline]
    pub fn run(&self, cond: &AtomicBool) {
        self.get().run(cond);
    }
    #[inline]
    pub fn set_theads_initial(&self, v: u8) {
        self.get().queue.set_theads_initial(v);
    }
    #[inline]
    pub fn threads(&self) -> Option<&[Thread]> {
        // Don't init the Lazy if it's not running.
        self.0.get_mut_no_init().and_then(|v| v.queue.threads())
    }
    #[inline]
    pub fn add<F: Future<Output = ()> + 'b>(&self, f: F) {
        self.get().add(f);
    }

    #[inline]
    fn get(&self) -> &mut Inner<'a, 'b> {
        self.0.get_mut(|| Box::new(abort_unlikely!(Inner::new())))
    }
    #[inline]
    fn get_ptr(&self) -> *mut () {
        &raw mut **(self.0.get_mut(|| Box::new(abort_unlikely!(Inner::new())))) as *mut ()
    }
}

unsafe impl Sync for Inner<'_, '_> {}
unsafe impl Sync for Controller<'_, '_> {}

#[inline]
pub fn run() {
    let v = AtomicBool::new(true);
    RUNTIME.run(&v);
}
#[inline]
pub fn run_one() {
    RUNTIME.run_one();
}
#[inline]
pub fn run_until_empty() {
    RUNTIME.run_until_empty();
}
#[inline]
pub fn run_while(cond: &AtomicBool) {
    RUNTIME.run(cond);
}
#[inline]
pub fn add<F: Future<Output = ()> + 'static>(f: F) {
    RUNTIME.add(f);
}
#[inline]
pub fn controller<'a, 'b>() -> &'a Controller<'a, 'b> {
    unsafe { &*((&RUNTIME as *const Controller as *const ()) as *const Controller) }
}
/// Wires the current [`Thread`] to the runtime. This will use the current
/// [`Thread`] to run additional tasks while the supplied [`Future`] is pending.
/// This function blocks until the supplied [`Future`] completes.
///
/// Once the supplied [`Future`] completes, this function will return the result
/// value of the [`Future`].
///
/// The "turnaround" time for this may be slightly longer than the [`Future`]
/// requests due to scheduling or other tasks being ran at the time of
/// completion.
///
/// The lifetime indicates that this will be valid ONLY for the lifetime of this
/// function
#[inline]
pub fn link<'a, T: 'a>(f: impl Future<Output = T> + 'a) -> T {
    let r = unsafe { &mut *(RUNTIME.get_ptr() as *mut Inner) };
    let t = Ticket::new(r, f);
    t.run(r)
}
