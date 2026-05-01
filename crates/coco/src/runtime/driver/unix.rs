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
#![cfg(not(target_family = "windows"))]

extern crate alloc;
extern crate core;

extern crate libc;
extern crate xrmt_stx;

use alloc::vec::Vec;
use core::convert::AsRef;
use core::num::NonZeroU32;
use core::option::Option::{self, None, Some};
use core::result::Result::Ok;

use libc::{poll, pollfd, POLLHUP, POLLIN, POLLOUT};
use xrmt_stx::io::IoResult;
use xrmt_stx::os::Handle;
use xrmt_stx::sync::extra::Event;

use crate::runtime::driver::queue::{KPoll, KQueue};
use crate::runtime::{Entries, Entry, EntrySlots, PollResult};
use crate::signals::{SignalHandler, SignalMask};

pub(crate) const FD_READ: i16 = POLLIN | POLLHUP | POLLRDUP;
pub(crate) const FD_WRITE: i16 = POLLOUT | POLLHUP | POLLRDUP;

#[cfg(any(
    target_os = "netbsd",
    target_os = "fuchsia",
    target_os = "solaris",
    target_vendor = "apple"
))]
const POLLRDUP: i16 = libc::POLLRDHUP;
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "fuchsia"),
    not(target_os = "solaris"),
    not(target_vendor = "apple")
))]
const POLLRDUP: i16 = 0i16;

pub struct Driver {
    kq:   KQueue,
    fds:  Handles,
    sigs: SignalHandler,
}

#[repr(C)]
struct Handles {
    wake:    pollfd,
    signals: pollfd,
    kq:      KPoll,
    list:    [pollfd; Driver::SIZE],
}

impl Driver {
    pub const SIZE: usize = unsafe { 62usize.unchecked_sub(KQueue::SIZE) };

    const EMPTY: Handle = Handle::new(0i32);

    #[inline]
    pub fn new(wake: &Event) -> IoResult<Driver> {
        let s = SignalHandler::new()?;
        let k = KQueue::new()?;
        Ok(Driver {
            fds:  Handles::new(*wake.as_ref(), &s, k.poll()),
            kq:   k,
            sigs: s,
        })
    }

    #[inline]
    pub fn empty(&self) -> &Handle {
        &Driver::EMPTY
    }
    #[inline]
    pub fn signals(&mut self) -> &mut SignalHandler {
        &mut self.sigs
    }
    #[cfg(any(
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_vendor = "apple"
    ))]
    #[inline]
    pub fn queue_remove(&self, h: &Handle) -> IoResult<()> {
        self.kq.remove(h)
    }
    #[inline]
    pub fn update<'a>(&mut self, q: EntrySlots<'a, '_>, e: Entries<'a, '_>) -> usize {
        let mut n = 0usize;
        for i in 0..e.len() {
            if n >= self.fds.list.len() {
                break;
            }
            let b = unsafe { self.fds.list.get_unchecked_mut(n..) };
            let x = unsafe { e.get_unchecked_mut(i) };
            n += match x.link(&Driver::EMPTY, b) {
                Some(v) => q.set(n, v, x.as_ptr()),
                None => break,
            };
            if let Some(p) = x.reason().pid() {
                // Fire errors with a panic on debug builds only since we
                // don't have a way to propagate the errors.
                #[cfg(debug_assertions)]
                let _ = self.sigs.proc(true, p).unwrap();
                #[cfg(not(debug_assertions))]
                let _ = self.sigs.proc(true, p);
            }
        }
        n
    }
    #[inline]
    pub fn poll(&mut self, q: EntrySlots, e: Entries, dur: Option<NonZeroU32>) -> PollResult {
        // Reset signals and events
        self.fds.reset();
        let r = unsafe {
            poll(
                self.fds.ptr(),
                self.fds.list.len() as _,
                dur.map_or(-1 as _, |v| v.get() as _),
            )
        };
        if r <= 0 {
            return PollResult::None;
        }
        let mut o = PollResult::None;
        if self.fds.kq.is_signaled() {
            self.kq.read(&mut o);
        }
        let (mut p, mut s) = (Vec::new(), SignalMask::empty());
        // If there's any signals hit, read the pending signals into a mask.
        if self.fds.signals.revents > 0 {
            self.sigs.read(&mut p, &mut s);
        }
        self.read(q, e, &p, s, &mut o);
        if self.fds.wake.revents > 0 {
            PollResult::Wake
        } else {
            o
        }
    }
    #[inline]
    pub fn queue_register(&self, write: bool, h: &Handle, e: &mut Entry<'_>) -> IoResult<()> {
        self.kq.add(write, h, e)
    }

    fn read(&mut self, q: EntrySlots, e: Entries, pids: &[u32], s: SignalMask, r: &mut PollResult) {
        for i in 0..q.len() {
            // 'q' will always be less than or equal to the fds size.
            let b = unsafe { self.fds.list.get_unchecked_mut(i) };
            let x = match q.get(i) {
                Some(v) => unsafe { &mut *v.as_ptr() },
                None => break,
            };
            if b.revents > 0 && x.mark() && r.is_none() {
                *r = PollResult::Entry(i);
            }
        }
        for i in 0..e.len() {
            let x = unsafe { e.get_unchecked_mut(i) };
            match (x.reason().is_pid(pids), x.reason().is_signal()) {
                (Some(v), _) => {
                    // Fire errors with a panic on debug builds only since we
                    // don't have a way to propagate the errors.
                    #[cfg(debug_assertions)]
                    let _ = self.sigs.proc(false, v).unwrap();
                    #[cfg(not(debug_assertions))]
                    let _ = self.sigs.proc(false, v);
                },
                (_, Some(v)) if v.contains_any(&s) => (),
                _ => continue,
            }
            if x.mark() && r.is_none() {
                *r = PollResult::Entry(i);
            }
        }
        if r.is_none() && !s.is_empty() {
            *r = PollResult::Signal;
        }
    }
}
impl Handles {
    #[inline]
    fn new(wake: Handle, sigs: &SignalHandler, kp: KPoll) -> Handles {
        Handles {
            kq:      kp,
            list:    [pollfd {
                fd:      0,
                events:  0,
                revents: 0,
            }; Driver::SIZE],
            wake:    pollfd {
                fd:      *wake,
                events:  FD_READ,
                revents: 0,
            },
            signals: pollfd {
                fd:      **sigs.as_ref(),
                events:  FD_READ,
                revents: 0,
            },
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.kq.reset();
        (self.wake.revents, self.signals.revents) = (0, 0)
    }
    #[inline]
    fn ptr(&mut self) -> *mut pollfd {
        self as *mut Handles as *mut pollfd
    }
}

#[cfg(any(
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_vendor = "apple"
))]
// For any OS's that support a KQueue-like mechanism.
mod queue {
    extern crate core;

    extern crate libc;
    extern crate xrmt_stx;

    use core::mem::zeroed;
    use core::ops::Drop;
    use core::ptr::{null, null_mut};
    use core::result::Result::Ok;

    use libc::{close, fcntl, kevent, kqueue, pollfd, timespec, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_CLEAR, EV_DELETE, EV_ENABLE, EV_ERROR, FD_CLOEXEC, F_SETFL, O_NONBLOCK};
    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::Handle;

    use crate::errcheck;
    use crate::runtime::{Entry, PollResult, FD_READ};

    #[repr(transparent)]
    pub struct KPoll(pollfd);
    pub struct KQueue(Handle);

    impl KPoll {
        #[inline]
        pub fn reset(&mut self) {
            self.0.revents = 0;
        }
        #[inline]
        pub fn is_signaled(&self) -> bool {
            self.0.revents > 0
        }
    }
    impl KQueue {
        pub const SIZE: usize = 1usize;

        #[inline]
        pub fn new() -> IoResult<KQueue> {
            let f = errcheck!(kqueue())?;
            errcheck!(fcntl(f, F_SETFL, O_NONBLOCK | FD_CLOEXEC))?;
            Ok(KQueue(Handle::new(f)))
        }

        #[inline]
        pub fn poll(&self) -> KPoll {
            KPoll(pollfd {
                fd:      *self.0,
                events:  FD_READ,
                revents: 0,
            })
        }
        #[inline]
        pub fn read(&mut self, r: &mut PollResult) {
            let (mut b, t) = unsafe { (zeroed::<kevent>(), zeroed::<timespec>()) };
            loop {
                if unsafe { kevent(*self.0, null(), 0, &mut b, 1, &t) } != 1 {
                    break;
                }
                if b.flags & EV_ERROR == 0 {
                    continue;
                }
                let p = b.udata as *mut ();
                if p.is_null() {
                    continue;
                }
                match b.filter {
                    EVFILT_READ | EVFILT_WRITE => (),
                    _ => continue,
                }
                if unsafe { &mut *(p as *mut Entry) }.mark() && r.is_none() {
                    // Return the pointer to an Entry instead.
                    *r = PollResult::Pointer(p);
                }
            }
        }
        #[inline]
        pub fn remove(&self, h: &Handle) -> IoResult<()> {
            let mut v = unsafe { zeroed::<kevent>() };
            v.flags = EV_DELETE;
            (v.ident, v.filter) = (**h as _, EVFILT_WRITE);
            if unsafe { kevent(*self.0, &v, 1, null_mut(), 0, null_mut()) } == -1 {
                v.filter = EVFILT_READ;
                errcheck!(kevent(*self.0, &v, 1, null_mut(), 0, null_mut()))?;
            }
            Ok(())
        }
        #[inline]
        pub fn add(&self, write: bool, h: &Handle, e: &mut Entry<'_>) -> IoResult<()> {
            let mut v = unsafe { zeroed::<kevent>() };
            v.flags = EV_ADD | EV_ENABLE | EV_CLEAR;
            v.udata = e as *mut Entry<'_> as _;
            (v.ident, v.filter) = (**h as _, if write { EVFILT_WRITE } else { EVFILT_READ });
            errcheck!(kevent(*self.0, &v, 1, null_mut(), 0, null_mut()))?;
            Ok(())
        }
    }

    impl Drop for KQueue {
        #[inline]
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                let _ = unsafe { close(*self.0) };
            }
        }
    }
}
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "openbsd"),
    not(target_os = "dragonfly"),
    not(target_vendor = "apple")
))]
mod queue {
    extern crate core;

    extern crate xrmt_stx;

    use core::convert::From;
    use core::result::Result::{Err, Ok};

    use xrmt_stx::io::{ErrorKind, IoError, IoResult};
    use xrmt_stx::os::Handle;

    use crate::runtime::{Entry, PollResult};

    #[repr(transparent)]
    pub struct KPoll(());
    #[repr(transparent)]
    pub struct KQueue(());

    impl KPoll {
        #[inline]
        pub fn reset(&mut self) {}
        #[inline]
        pub fn is_signaled(&self) -> bool {
            false
        }
    }
    impl KQueue {
        pub const SIZE: usize = 0usize;

        #[inline]
        pub fn new() -> IoResult<KQueue> {
            Ok(KQueue(()))
        }

        #[inline]
        pub fn poll(&self) -> KPoll {
            KPoll(())
        }
        #[inline]
        pub fn read(&mut self, _r: &mut PollResult) {}
        #[inline]
        pub fn add(&self, _write: bool, _h: &Handle, _e: &mut Entry<'_>) -> IoResult<()> {
            Err(IoError::from(ErrorKind::Unsupported))
        }
    }
}
