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

extern crate xrmt_stx;

use core::convert::{AsRef, From};
use core::option::Option::{self, None, Some};
use core::pin::Pin;
use core::ptr::NonNull;
use core::result::Result::{self, Err, Ok};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use core::time::Duration;

use xrmt_stx::io::{ErrorKind, IoError, IoResult};
use xrmt_stx::os::Handle;
use xrmt_stx::process::Child;
use xrmt_stx::sync::MutexGuard;
use xrmt_stx::time::Instant;

use crate::future::Status;
use crate::runtime::{Container, Driver, Entry, EntryInner, Reason};
use crate::signals::SignalMask;

static WAKER_TABLE: RawWakerVTable = RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

pub struct State<'a> {
    pub(super) entry: NonNull<Entry<'a>>,

    lock:    MutexGuard<'a, EntryInner<'a>>,
    driver:  NonNull<Driver>,
    timeout: bool,
    updated: bool,
}

impl<'a> State<'a> {
    #[inline]
    pub fn timeout<T>() -> Poll<IoResult<T>> {
        Poll::Ready(Err(IoError::from(ErrorKind::TimedOut)))
    }
    #[inline]
    pub fn from_context<'b>(cx: &mut Context<'b>) -> &'b mut State<'b> {
        unsafe { &mut *(cx.waker().data() as *mut State<'b>) }
    }
    #[inline]
    pub fn make_poll<T, E>(r: Result<Option<T>, E>) -> Poll<Result<T, E>> {
        match r {
            Ok(Some(v)) => Poll::Ready(Ok(v)),
            Ok(None) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    #[inline]
    pub fn clear(&self) {
        self.entry().clear(self.driver());
    }
    #[inline]
    pub fn reason(&self) -> &Reason {
        self.entry().reason()
    }
    #[inline]
    pub fn is_timeout(&self) -> bool {
        self.timeout
    }
    #[cfg(target_family = "windows")]
    #[inline]
    pub fn iocp_handle(&self) -> &Handle {
        self.driver().empty()
    }
    #[inline]
    pub fn set_timeout(&mut self, dur: Duration) {
        if dur.is_zero() {
            self.set_deadline(None);
        } else {
            self.set_deadline(Some(Instant::now() + dur))
        }
    }
    #[inline]
    pub fn register_signals(&mut self, s: SignalMask) {
        self.entry().set_reason(Reason::Signal(s))
    }
    #[inline]
    pub fn set_deadline(&mut self, when: Option<Instant>) {
        self.entry().set_timeout(when);
    }
    #[inline]
    pub fn is_ready<T>(&mut self, v: &Pin<&mut T>) -> bool {
        self.status(v).is_ready()
    }
    #[inline]
    pub fn set_process(&mut self, c: &Child) -> IoResult<()> {
        inner::register_child(self, c)
    }
    #[inline]
    pub fn status<'b, T>(&'b mut self, v: &Pin<&mut T>) -> Status<'a, 'b> {
        // NOTE(dij): If the pointer value fails or overlaps, use the following
        //            instead:
        //      let y = &*v as *const Pin<&mut T> as *const ();
        //
        let v = self.lock.is_same(&**v as *const T as *const ());
        if v {
            return Status::Ready(self);
        }
        // Reset the Entry if we're different and have been updated.
        (self.timeout, self.updated) = (false, true);
        self.entry().clear(self.driver());
        Status::Setup(self)
    }
    #[inline]
    pub fn register_queue(&mut self, write: bool, h: impl AsRef<Handle>) -> IoResult<()> {
        if !Container::KQUEUE {
            return self.register_handle(write, h);
        }
        let e: &mut Entry<'_> = self.entry();
        let v = h.as_ref();
        //if !e.in_queue(h.as_ref()) {
        self.driver().queue_register(write, v, e)?;
        e.add_queue(v);
        //
        Ok(())
    }
    #[inline]
    pub fn register_handle(&mut self, write: bool, h: impl AsRef<Handle>) -> IoResult<()> {
        if !self.entry().add_handle(write, h.as_ref()) {
            Err(IoError::from(ErrorKind::StorageFull))
        } else {
            Ok(())
        }
    }

    #[inline]
    pub(super) fn new(e: NonNull<Entry<'a>>, d: NonNull<Driver>, i: MutexGuard<'a, EntryInner<'a>>, t: bool) -> State<'a> {
        State {
            lock:    i,
            entry:   e,
            driver:  d,
            timeout: t,
            updated: false,
        }
    }

    #[inline]
    pub(super) fn run(&mut self) -> bool {
        let r = {
            let w = unsafe {
                Waker::from_raw(RawWaker::new(
                    self as *mut State<'_> as *const (),
                    &WAKER_TABLE,
                ))
            };
            let mut c = Context::from_waker(&w);
            self.lock.run(&mut c)
        };
        if r {
            self.entry().set_done();
        } else {
            self.entry().set_sleeping();
        }
        self.updated || r
    }

    #[inline]
    fn driver(&self) -> &'a mut Driver {
        unsafe { &mut *self.driver.as_ptr() }
    }
    #[inline]
    fn entry(&self) -> &'a mut Entry<'a> {
        unsafe { &mut *self.entry.as_ptr() }
    }
}

impl<'a> From<&mut Context<'a>> for &'a mut State<'a> {
    #[inline]
    fn from(v: &mut Context<'a>) -> &'a mut State<'a> {
        State::from_context(v)
    }
}

#[inline]
unsafe fn waker_wake(v: *const ()) {
    let _ = unsafe { &mut *(v as *mut State<'_>) }.entry().mark();
}
#[inline]
unsafe fn waker_drop(_v: *const ()) {}
#[inline]
unsafe fn waker_wake_by_ref(v: *const ()) {
    let _ = unsafe { &mut *(v as *mut State<'_>) }.entry().mark();
}
#[inline]
unsafe fn waker_clone(v: *const ()) -> RawWaker {
    RawWaker::new(v, &WAKER_TABLE)
}

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_stx;

    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::windows::io::AsHandle;
    use xrmt_stx::process::Child;

    use crate::runtime::State;

    #[inline]
    pub fn register_child(state: &mut State<'_>, c: &Child) -> IoResult<()> {
        state.register_handle(false, c.as_handle())
    }
}
#[cfg(not(target_family = "windows"))]
mod inner {
    extern crate core;

    extern crate xrmt_stx;

    use core::result::Result::Ok;

    use xrmt_stx::io::IoResult;
    use xrmt_stx::process::Child;

    use crate::runtime::{Reason, State};

    #[inline]
    pub fn register_child(state: &mut State<'_>, c: &Child) -> IoResult<()> {
        state.entry().set_reason(Reason::Process(c.id()));
        Ok(())
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::write;

    use crate::runtime::State;

    impl Debug for State<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "State[{:?}]", self.entry())
        }
    }
}
