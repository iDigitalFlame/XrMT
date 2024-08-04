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

use alloc::collections::VecDeque;
use core::pin::Pin;
use core::time::Duration;

use crate::c2::event::{Pinned, Poll, Reason, Task};
use crate::c2::task::{Beacon, Driver, Fd};
use crate::com::Packet;
use crate::data::time::Time;
use crate::device::ID;
use crate::prelude::*;
use crate::{ignore_error, io};

pub struct Queue {
    when:    Ticker,
    dirty:   bool,
    parked:  Vec<Entry>,
    driver:  Driver,
    backlog: VecDeque<Entry>,
}
pub struct Entry {
    #[cfg(feature = "bugs")]
    pub job: u16,
    fd:      Fd,
    wake:    Time,
    task:    Option<Task>,
    event:   Pin<Pinned>,
    first:   bool,
}

struct Ticker(Time);

impl Entry {
    #[inline]
    pub fn new(job: u16, fd: Option<Fd>, t: &mut Task, dev: ID) -> Entry {
        let _ = job;
        let mut e = Entry {
            #[cfg(feature = "bugs")]
            job,
            fd: fd.unwrap_or_default(),
            wake: Time::ZERO,
            task: None,
            event: Pinned::new(),
            first: false,
        };
        t.setup(&mut e.event, dev);
        e
    }

    #[inline]
    pub fn fd(&self) -> bool {
        self.fd.is_valid()
    }
    #[inline]
    pub fn as_fd(&self) -> &Fd {
        &self.fd
    }
    #[inline]
    pub fn task(&mut self, t: Task) {
        self.wake = t.duration().map(|v| Time::now() + v).unwrap_or(Time::ZERO);
        self.task = Some(t);
    }
    #[inline]
    pub fn timeout(&mut self, dur: Duration) {
        self.wake = Time::now() + dur;
    }
    #[inline]
    pub fn run(&mut self, now: Time, r: Reason) -> Poll {
        let t = some_or_return!(self.task.as_mut(), Poll::Done);
        let v = t.do_poll(r);
        if let Some(d) = t.duration() {
            self.wake = now + d;
        }
        v
    }
}
impl Queue {
    #[inline]
    pub fn new() -> io::Result<Queue> {
        Ok(Queue {
            when:    Ticker::new(),
            dirty:   true,
            parked:  Vec::new(),
            driver:  Driver::new()?,
            backlog: VecDeque::new(),
        })
    }

    #[inline]
    pub fn reset_wake(&self) {
        self.driver.reset()
    }
    #[inline]
    pub fn beacon(&self) -> Beacon {
        self.driver.beacon()
    }
    #[inline]
    pub fn add(&mut self, e: Entry) {
        if self.parked.len() >= 63 {
            bugtrack!(
                "c2::event::Queue.add(): Adding Task Entry {} to Backlog.",
                e.job
            );
            self.backlog.push_back(e)
        } else {
            #[cfg(feature = "bugs")]
            if !e.wake.is_zero() {
                bugtrack!(
                    "c2::event::Queue.add(): Adding Task Entry {} to Event queue with a wake time on {}!",
                    e.job,
                    e.wake
                );
            } else {
                bugtrack!(
                    "c2::event::Queue.add(): Adding Task Entry {} to Event queue!",
                    e.job
                );
            }
            self.parked.push(e);
            self.dirty = true;
        }
    }
    pub fn run(&mut self) -> Option<Option<Packet>> {
        if self.dirty {
            if let Some(i) = self.mark() {
                return self.cleanup(i);
            }
            // Resort the Vec to make Entires with a smaller timeout in the front.
            self.parked.sort_by(|a, b| a.wake.cmp(&b.wake));
            // Since the thread that adds them is the same as this one, the
            // above is safe.
            self.driver.update(&self.parked);
            self.when.update(&self.parked, true);
            self.dirty = false;
        }
        let n = Time::now();
        let t = self.when.next(n);
        bugtrack!("c2::event::Queue.run(): Entering driver poll with timeout {t:?}.");
        let x = self.driver.poll(t);
        bugtrack!(
            "c2::event::Queue.run(): Driver poll returned {x:?} {}.",
            self.parked.len()
        );
        let (i, r) = match x {
            Some(i) if i >= self.parked.len() => return None,
            Some(i) => (i, Reason::Wake),
            None if !self.when.is_zero() => {
                bugtrack!("c2::event::Queue.run(): Checking wake timeouts..");
                (some_or_return!(self.timeouts(), None), Reason::Timeout)
            },
            _ => return None,
        };
        self.when.update(&self.parked, false);
        bugtrack!("c2::event::Queue.run(): Poll returned Task index {i} with reason {r}.");
        if self.parked[i].run(n, r).is_pending() {
            return None;
        }
        self.cleanup(i)
    }

    fn mark(&mut self) -> Option<usize> {
        bugtrack!("c2::event::Queue.mark(): Event array is dirty, updating it..");
        let n = Time::now();
        for (i, e) in self.parked.iter_mut().enumerate() {
            if e.first {
                continue;
            }
            bugtrack!(
                "c2::event::Queue.mark(): Running first poll for Task {}.",
                e.job
            );
            let r = e.run(n, Reason::Wake).is_pending();
            e.first = true;
            if r {
                continue;
            }
            bugtrack!(
                "c2::event::Queue.mark(): First poll completed Task {}!",
                e.job
            );
            return Some(i);
        }
        None
    }
    #[inline]
    fn timeouts(&mut self) -> Option<usize> {
        let n: Time = Time::now();
        let i = self.parked.iter().position(|e| !e.wake.is_zero() && e.wake <= n);
        match i {
            Some(i) => {
                bugtrack!("c2::event::Queue.timeouts(): Found Task index {i} with a timeout past {n}, triggering it.");
                Some(i)
            },
            None => None,
        }
    }
    fn cleanup(&mut self, index: usize) -> Option<Option<Packet>> {
        // We don't care about ordering since it'll be resorted on the next
        // 'poll' call.
        let e = self.parked.swap_remove(index);
        bugtrack!(
            "c2::event::Queue.cleanup(): Removing completed/canceled Task {}.",
            e.job
        );
        if self.parked.len() <= 63 {
            if let Some(v) = self.backlog.pop_front() {
                bugtrack!(
                    "c2::event::Queue.cleanup(): Moving Backlog Task {} Entry to Event queue!",
                    v.job
                );
                self.parked.push(v);
            }
        }
        let r = match e.task {
            Some(v) => Task::finish(v),
            None => {
                ignore_error!(e.event.set());
                None
            },
        };
        self.dirty = true;
        Some(r)
    }
}
impl Ticker {
    #[inline]
    fn new() -> Ticker {
        Ticker(Time::ZERO)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    fn update(&mut self, e: &[Entry], sorted: bool) {
        // Quickpath
        if e.is_empty() || (e.len() == 1 && e[0].wake.is_zero()) {
            bugtrack!("c2::event::Ticker.update(): Setting next wake Time to None.");
            self.0 = Time::ZERO;
            return;
        }
        if sorted {
            // Len is at least 1 here.
            let n = if e.len() == 1 {
                // Quickpath for one.
                Some(e[0].wake)
                // This can't be zero as the above sanity check will not land
                // here if len() == 1 and e[0].is_zero().
            } else {
                // Find the first non-zero entry.
                e.iter().find(|i| !i.wake.is_zero()).map(|i| i.wake)
            };
            match n {
                Some(v) if v == self.0 => (), // Ignore as we already have it.
                Some(v) => {
                    bugtrack!("c2::event::Ticker.update(): Setting next wake Time to {v}.");
                    self.0 = v;
                },
                None => {
                    bugtrack!("c2::event::Ticker.update(): Setting next wake Time to None.");
                    self.0 = Time::ZERO;
                },
            }
            return;
        }
        for i in e.iter() {
            if i.wake.is_zero() || (!self.0.is_zero() && i.wake.is_after(self.0)) {
                continue;
            }
            self.0 = i.wake;
        }
        bugtrack!(
            "c2::event::Ticker.update(): Setting next wake Time to {}.",
            self.0
        );
    }
    #[inline]
    fn next(&mut self, now: Time) -> Option<Duration> {
        if self.0.is_zero() {
            None
        } else {
            Some(self.0 - now)
        }
    }
}

impl Drop for Queue {
    #[inline]
    fn drop(&mut self) {
        for i in self.parked.iter_mut() {
            // Cancel all waiting.
            ignore_error!(i.event.set());
        }
        self.parked.clear();
        self.backlog.clear();
    }
}
