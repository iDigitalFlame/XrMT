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

extern crate xrmt_bugtrack;
extern crate xrmt_stx;

use core::mem::{replace, transmute, MaybeUninit};
use core::ops::Drop;
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::result::Result::{Err, Ok};
use core::sync::atomic::{AtomicBool, Ordering};

use xrmt_bugtrack::bugtrack;
use xrmt_stx::thread::{self, Builder, JoinHandle, ThreadId};

use crate::runtime::{Driver, RUNTIME};

pub struct Thread {
    flag:   NonNull<AtomicBool>,
    handle: JoinHandle<()>,
}
pub struct ThreadPool {
    cur:     u8,
    max:     u8,
    init:    u8,
    threads: [MaybeUninit<Thread>; ThreadPool::MAX],
}

impl Thread {
    #[inline]
    pub fn id(&self) -> ThreadId {
        self.handle.thread().id()
    }
    #[inline]
    pub fn thread(&self) -> &thread::Thread {
        self.handle.thread()
    }

    #[inline]
    fn close(self) {
        // Let the Thread know we're closing shop.
        unsafe { &*self.flag.as_ptr() }.store(true, Ordering::Release);
        bugtrack!(
            "(Threads).close(): Waiting for Thread {:?} to close..",
            self.handle.thread().id()
        );
        let _ = self.handle.join(); // Wait for Thead to complete.
        bugtrack!("(Threads).close(): Thread close complete!",);
    }
}
impl ThreadPool {
    const MAX: usize = 8usize;

    #[inline]
    pub fn new() -> ThreadPool {
        ThreadPool {
            cur:     0u8,
            max:     ThreadPool::MAX as u8,
            init:    0u8,
            threads: [const { MaybeUninit::uninit() }; ThreadPool::MAX],
        }
    }

    pub fn close(&mut self) {
        if self.cur == 0 {
            return;
        }
        bugtrack!("(ThreadPool).close(): Closing {} Threads!", self.cur);
        for i in 0..self.cur as usize {
            bugtrack!("(Threads).close(): Closing Thread {i}..");
            unsafe {
                // Always in bounds
                replace(self.threads.get_unchecked_mut(i), MaybeUninit::uninit())
                    .assume_init()
                    .close()
            }
        }
        self.cur = 0
    }
    #[inline]
    pub fn set_max(&mut self, v: u8) {
        bugtrack!(
            "(ThreadPool).spawn(): Updating max Thread count to {v}! (Current: {}, pre-Max: {}, Initial: {})",
            self.cur,
            self.max,
            self.init
        );
        self.max = v
    }
    #[inline]
    pub fn set_initial(&mut self, v: u8) {
        bugtrack!(
            "(ThreadPool).spawn(): Updating initial Thread count to {v}! (Current: {}, Max: {}, prev-Initial: {})",
            self.cur,
            self.max,
            self.init
        );
        self.init = v
    }
    pub fn check(&mut self, n: usize, w: u8) {
        let _ = w; // TODO(dij): <-
                   // Quick path, do we have any Entries or is the initial size zero?
        if n == 0 || self.init == 0 || self.cur > self.max {
            return;
        }
        bugtrack!(
            "(ThreadPool).check(): Checking if we should make a thread. (Initial: {}, Max: {}, Current: {})",
            self.init,
            self.max,
            self.cur
        );
        // Check if we have the space to spawn a Thread.
        if (self.cur as usize) >= self.threads.len() || (self.cur as usize + 1) >= self.threads.len() {
            bugtrack!(
                "(ThreadPool).spawn(): Not spawning a thread, too many active Threads! (Current: {}, Max: {}, Initial: {})",
                self.cur,
                self.max,
                self.init
            );
            return;
        }
        // Let's do some math to see if we should spawn a Thread.
        //
        // Spawning conditions:
        // - We have 1/2 of Threads created from our initial ask.
        // - Divide the queue by the Driver max size (see current utilization), then
        //   divide that by the number of current Threads (this is the utilization
        //   percentage per thread, or how many Entries per Thread if ballanced).
        //   Multiply by 100 (make a percentage) and strip to a u8 (shouldn't be more
        //   than 100 tbh). Check if the resulting utilization is over 10%.
        let c = self.init.saturating_div(2);
        let p = ((n.saturating_div(Driver::SIZE).checked_div(self.cur as usize).unwrap_or(1) * 100) & 0xFF) as u8;
        bugtrack!(
            "(ThreadPool).spawn(): Math checkpoint (Initial/2 = {c} >= Current {}) or (Utilization = {p}% >= 10%)",
            self.cur
        );
        if c >= self.cur || p >= 10 {
            bugtrack!("(ThreadPool).spawn(): Attempting a Thread spawn!");
            let _ = self.spawn();
        }
    }
    #[inline]
    pub fn threads(&self) -> Option<&[Thread]> {
        if self.cur > 0 {
            Some(unsafe { transmute(self.threads.get_unchecked(0..self.cur as usize)) })
        } else {
            None
        }
    }

    fn spawn(&mut self) -> bool {
        let mut f = AtomicBool::new(true);
        let n = (self.cur + 1) as usize;
        let d = unsafe { NonNull::new_unchecked(&mut f) };
        let t = match Builder::new().spawn(move || thread_run(f)) {
            Ok(v) => v,
            Err(_e) => {
                bugtrack!("(ThreadPool).spawn(): Spawn failed: {_e:?}!");
                return false;
            },
        };
        // Bounds check happens earlier.
        unsafe { self.threads.get_unchecked_mut(n) }.write(Thread { flag: d, handle: t });
        self.cur = n as u8;
        bugtrack!(
            "(ThreadPool).spawn(): Thread tracking updated! (Current: {}, Initial: {})",
            self.cur,
            self.init
        );
        true
    }
}

impl Drop for ThreadPool {
    #[inline]
    fn drop(&mut self) {
        self.close();
    }
}

#[inline]
fn thread_run(f: AtomicBool) {
    RUNTIME.run(&f)
}
