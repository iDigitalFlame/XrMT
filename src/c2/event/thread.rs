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

use core::mem::MaybeUninit;

use crate::c2::event::{Reason, Task};
use crate::com::Packet;
use crate::ignore_error;
use crate::prelude::*;
use crate::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use crate::thread::{Builder, JoinHandle, Thread};

const MAX_THREADS: u8 = 8u8;

pub struct ThreadQueue {
    mask:    u8,
    threads: [MaybeUninit<ThreadHandle>; 8],
}
pub struct ThreadHandle {
    chan:   SyncSender<Task>,
    handle: JoinHandle<()>,
}

impl ThreadQueue {
    #[inline]
    pub fn new() -> ThreadQueue {
        ThreadQueue {
            mask:    0u8,
            threads: [
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }

    #[inline]
    pub fn thread(&self, index: u8) -> Option<&Thread> {
        if self.mask & (1 << index) != 0 {
            Some(unsafe { self.threads[index as usize].assume_init_ref() }.handle.thread())
        } else {
            None
        }
    }
    #[inline]
    pub fn send(&mut self, t: Task, s: &SyncSender<Packet>) {
        self.send_or_spawn(0, t, s)
    }

    fn spawn(&mut self, pos: u8, s: &SyncSender<Packet>) -> bool {
        bugtrack!("c2::event::ThreadQueue.spawn(): Creating Thread in slot {pos}..");
        let v = s.clone();
        let (x, y) = sync_channel(0);
        match Builder::new().spawn(move || ThreadQueue::run(pos, v, y)) {
            Ok(h) => {
                bugtrack!(
                    "c2::event::ThreadQueue.spawn(): Thread({pos}) in slot {pos} created, thread={:?}.",
                    h.thread()
                );
                self.threads[pos as usize].write(ThreadHandle { chan: x, handle: h });
                self.mask |= 1 << pos;
                true
            },
            Err(e) => {
                let _ = e; // Mark used.
                bugtrack!("c2::event::ThreadQueue.spawn(): Spawn in slot {pos} failed: {e}!");
                false
            },
        }
    }
    fn send_or_spawn(&mut self, pos: u8, t: Task, s: &SyncSender<Packet>) {
        if pos >= MAX_THREADS {
            bugtrack!("c2::event::ThreadQueue.send_or_spawn(): All Thread queues exhausted? Blocking send on first Thread!");
            if self.mask & 1 == 0 && !self.spawn(pos, s) {
                bugtrack!("c2::event::ThreadQueue.send_or_spawn(): All Thread queues exhausted? Spawn for first Thread failed, dropping Task!");
                return;
            }
            ignore_error!(unsafe { self.threads[0].assume_init_ref() }.chan.send(t));
            // Wait here since the Queue is full.
            return;
        }
        if self.mask & (1 << pos) == 0 {
            if self.spawn(pos, s) {
                (unsafe { self.threads[pos as usize].assume_init_ref() }.chan.send(t)).unwrap();
                return;
            } else {
                return self.send_or_spawn(pos, t, s);
            }
        }
        match unsafe { self.threads[pos as usize].assume_init_ref() }.chan.try_send(t) {
            Ok(_) => {
                bugtrack!("c2::event::ThreadQueue.send_or_spawn(): Delivered Task on Thread({pos})");
                return;
            },
            Err(TrySendError::Disconnected(_)) => {
                bugtrack!("c2::event::ThreadQueue.send_or_spawn(): Thread({pos}) is disconnected, dropping it and skipping it.");
                unsafe { self.threads[pos as usize].assume_init_drop() };
                self.mask ^= 1 << pos;
            },
            Err(TrySendError::Full(v)) => {
                bugtrack!("c2::event::ThreadQueue.send_or_spawn(): Thread({pos}) is full, trying next thread..");
                self.send_or_spawn(pos + 1, v, s);
            },
        }
    }

    fn run(pos: u8, s: SyncSender<Packet>, r: Receiver<Task>) {
        let _ = pos; // Mark as Used
        bugtrack!("c2::event::ThreadQueue.run(): Thread({pos}) Started!");
        loop {
            bugtrack!("c2::event::ThreadQueue.run(): Thread({pos}) Waiting for Task..");
            match r.recv() {
                Ok(mut t) => {
                    bugtrack!(
                        "c2::event::ThreadQueue.run(): Thread({pos}) received Task {}!",
                        t.job
                    );
                    while t.do_poll(Reason::Threaded).is_pending() {}
                    let n = some_or_continue!(Task::finish(t));
                    bugtrack!(
                        "c2::event::ThreadQueue.run(): Thread({pos}) completed Task {}, attempting to send..",
                        n.job
                    );
                    if s.send(n).is_err() {
                        bugtrack!("c2::event::ThreadQueue.run(): Thread({pos}), error sending Task, sender was likely disconnected!");
                        break;
                    }
                    bugtrack!("c2::event::ThreadQueue.run(): Thread({pos}) Task sent!")
                },
                Err(_) => break,
            }
        }
        bugtrack!("c2::event::ThreadQueue.run(): Thread({pos}) exiting!");
    }
}

impl Drop for ThreadQueue {
    #[inline]
    fn drop(&mut self) {
        for (i, t) in self.threads.iter_mut().enumerate() {
            if self.mask & (1 << i) == 0 {
                continue;
            }
            unsafe { t.assume_init_drop() };
        }
    }
}
