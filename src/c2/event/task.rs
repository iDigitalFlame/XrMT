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

use core::any::Any;
use core::matches;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::NonNull;
use core::time::Duration;

use crate::c2::error_to_packet;
use crate::c2::task::TID;
use crate::com::{Flag, Packet};
use crate::device::ID;
use crate::prelude::*;
use crate::sync::Event;
use crate::thread::sleep;
use crate::{ignore_error, io};

pub enum Poll {
    Done,
    Pending,
}
pub enum Reason {
    Wake,
    Timeout,
    Closing,
    Threaded,
}
pub enum Return {
    Output,
    NoOutput,
}

pub struct Task {
    pub job: u16,
    ctx:     Context,
    err:     bool,
    poll:    Option<Box<dyn FnMut(&mut Context, Reason, &mut Packet) -> io::Result<Poll>>>,
    done:    Box<dyn FnOnce(&mut Context, &mut Packet) -> io::Result<Return>>,
    output:  Packet,
}
pub struct Context {
    dur:    Duration,
    data:   Option<Box<dyn Any>>,
    signal: NonNull<Pin<Pinned>>,
}

pub trait TaskPoll = FnMut(&mut Context, Reason, &mut Packet) -> io::Result<Poll> + 'static;
pub trait TaskSubmit = FnOnce(&mut Context, &mut Packet) -> io::Result<Return> + 'static;

pub(super) struct Pinned(Event);

impl Task {
    #[inline]
    pub fn new(job: u16, submit: impl TaskSubmit) -> Task {
        Task::new_with_packet(job, Packet::new(), submit)
    }
    #[inline]
    pub fn new_with_packet(job: u16, mut n: Packet, submit: impl TaskSubmit) -> Task {
        bugtrack!("c2::event::Task::new() New Task {job} created!");
        (n.id, n.job) = (TID::RV_RESULT, job);
        Task {
            job,
            ctx: Context::new(Duration::ZERO),
            err: false,
            poll: None,
            done: Box::new(submit),
            output: n,
        }
    }

    #[inline]
    pub fn duration(&self) -> Option<Duration> {
        if self.ctx.dur.is_zero() {
            None
        } else {
            Some(self.ctx.dur)
        }
    }
    #[inline]
    pub fn arg(mut self, data: impl Any) -> Task {
        self.ctx.data = Some(Box::new(data));
        self
    }
    #[inline]
    pub fn timeout(mut self, dur: Duration) -> Task {
        self.ctx.dur = dur;
        self
    }
    #[inline]
    pub fn poll(mut self, poll: impl TaskPoll) -> Task {
        self.poll = Some(Box::new(poll));
        self
    }
    #[inline]
    pub fn packet(mut self, mut f: impl FnMut(&mut Packet)) -> Task {
        f(&mut self.output);
        self
    }

    #[inline]
    pub(super) fn do_poll(&mut self, r: Reason) -> Poll {
        bugtrack!(
            "c2::event::Task.do_poll(): Polling Task {} with Reason {r}.",
            self.job
        );
        match (some_or_return!(self.poll.as_mut(), Poll::Done))(&mut self.ctx, r, &mut self.output) {
            Ok(v) => {
                bugtrack!(
                    "c2::event::Task.do_poll(): Task {} poll compeletd with state {v}.",
                    self.job
                );
                v
            },
            Err(e) => {
                bugtrack!(
                    "c2::event::Task.do_poll(): Task {} poll failed with error: {e}!",
                    self.job
                );
                self.err = true;
                self.output.clear();
                self.output.flags |= Flag::ERROR;
                error_to_packet(&mut self.output, &e);
                Poll::Done
            },
        }
    }
    #[inline]
    pub(super) fn setup(&mut self, s: &mut Pin<Pinned>, dev: ID) {
        self.output.device = dev;
        self.ctx.signal = unsafe { NonNull::new_unchecked(s) };
    }
    #[inline]

    pub(super) fn finish(mut t: Task) -> Option<Packet> {
        bugtrack!("c2::event::Task::finish(): Completing Task {}.", t.job);
        let mut n = t.output;
        if t.err {
            bugtrack!(
                "c2::event::Task::finish(): Task {} has previously errored, returning error Packet!",
                t.job
            );
            (n.id, n.job) = (TID::RV_RESULT, t.job);
            return Some(n);
        }
        match (t.done)(&mut t.ctx, &mut n) {
            Err(e) => {
                bugtrack!(
                    "c2::event::Task::finish(): Task {} completion filed with error: {e}, return error Packet!",
                    t.job
                );
                n.clear();
                n.flags |= Flag::ERROR;
                (n.id, n.job) = (TID::RV_RESULT, t.job);
                error_to_packet(&mut n, &e);
                return Some(n);
            },
            Ok(r) => match r {
                Return::NoOutput => {
                    bugtrack!(
                        "c2::event::Task::finish(): Task {} completed with no return output.",
                        t.job
                    );
                    return None;
                },
                _ => (),
            },
        }
        bugtrack!(
            "c2::event::Task::finish(): Task {} completed with output {n}!",
            t.job
        );
        (n.id, n.job) = (TID::RV_RESULT, t.job);
        Some(n)
    }
}
impl Poll {
    #[inline]
    pub fn is_done(&self) -> bool {
        matches!(self, Poll::Done)
    }
    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self, Poll::Pending)
    }
}
impl Reason {
    #[inline]
    pub fn is_thread(&self) -> bool {
        matches!(self, Reason::Threaded)
    }
    #[inline]
    pub fn is_closing(&self) -> bool {
        matches!(self, Reason::Closing)
    }
    #[inline]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Reason::Timeout)
    }
}
impl Pinned {
    #[inline]
    pub fn new() -> Pin<Pinned> {
        Pin::new(Pinned(Event::new()))
    }
}
impl Context {
    #[inline]
    fn new(dur: Duration) -> Context {
        Context {
            dur,
            data: None,
            signal: NonNull::dangling(),
        }
    }

    #[inline]
    pub fn wait(&self) {
        if self.dur.is_zero() {
            if (self.signal.as_ptr() as usize) < 8 {
                return;
            }
            ignore_error!(unsafe { &*self.signal.as_ptr() }.wait());
        } else {
            self.wait_for(self.dur);
        }
    }
    #[inline]
    pub fn timeout(&self) -> Duration {
        self.dur
    }
    #[inline]
    pub fn wait_for(&self, dur: Duration) {
        if (self.signal.as_ptr() as usize) < 8 {
            sleep(dur)
        } else {
            ignore_error!(unsafe { &*self.signal.as_ptr() }.wait_for(dur));
        }
    }
    #[inline]
    pub fn arg_ref<T: 'static>(&self) -> Option<&T> {
        self.data.as_ref().map(|v| v.downcast_ref::<T>()).flatten()
    }
    #[inline]
    pub fn arg<T: 'static>(&mut self) -> Option<Box<T>> {
        self.data.take().map(|v| v.downcast::<T>().ok()).flatten()
    }
    #[inline]
    pub fn arg_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.data.as_mut().map(|v| v.downcast_mut::<T>()).flatten()
    }
}

impl Deref for Pinned {
    type Target = Event;

    #[inline]
    fn deref(&self) -> &Event {
        &self.0
    }
}

unsafe impl Send for Task {}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::c2::event::{Poll, Reason};
    use crate::prelude::*;

    impl Debug for Poll {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                Self::Done => write!(f, "Done"),
                Self::Pending => write!(f, "Pending"),
            }
        }
    }
    impl Display for Poll {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(self, f)
        }
    }
    impl Debug for Reason {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                Reason::Wake => write!(f, "Wake"),
                Reason::Timeout => write!(f, "Timeout"),
                Reason::Closing => write!(f, "Closing"),
                Reason::Threaded => write!(f, "Threaded"),
            }
        }
    }
    impl Display for Reason {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Debug::fmt(self, f)
        }
    }
}
