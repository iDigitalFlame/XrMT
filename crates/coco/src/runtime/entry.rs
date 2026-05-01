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
use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::future::Future;
use core::hint::unlikely;
use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::ops::{Deref, DerefMut};
use core::option::Option::{self, None, Some};
use core::pin::Pin;
use core::ptr::{null, NonNull};
use core::result::Result::{Err, Ok};
use core::task::Context;

use xrmt_bugtrack::bugtrack;
use xrmt_stx::abort;
use xrmt_stx::sync::{Mutex, TryLockError};
use xrmt_stx::time::extra::{from_instant, Time};
use xrmt_stx::time::Instant;

use crate::runtime::{Container, Driver, Reason, State};

pub struct Entry<'a> {
    cont:   Container,
    lock:   Mutex<EntryInner<'a>>,
    wake:   Time,
    reason: Reason,
}
pub struct EntryMap<'a> {
    n: u8,
    v: [EntryReference<'a>; Driver::SIZE],
}
pub struct EntryInner<'a> {
    f:   Pin<Box<dyn Future<Output = ()> + 'a>>,
    ptr: *const (),
    _p:  PhantomData<&'a ()>,
}

pub type EntryPointer<'a> = Box<Entry<'a>>;
pub type EntryReference<'a> = NonNull<Entry<'a>>;
pub type EntrySlots<'a, 'b> = &'b mut EntryMap<'a>;
pub type Entries<'a, 'b> = &'b mut [EntryPointer<'a>];

impl<'a> Entry<'a> {
    #[inline]
    pub fn new<F: Future<Output = ()> + 'a>(f: F) -> Entry<'a> {
        Entry {
            cont:   Container::new(),
            lock:   Mutex::new(EntryInner {
                f:   Box::pin(f),
                ptr: null(),
                _p:  PhantomData,
            }),
            wake:   Time::ZERO,
            reason: Reason::None,
        }
    }

    #[inline]
    pub fn mark(&self) -> bool {
        if self.cont.set_ready() {
            return true;
        }
        bugtrack!("(Entry).mark(): Mark called on Entry with invalid state ({self:?})!");
        false
    }
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.wake.is_zero()
    }
    #[inline]
    pub fn set_sleeping(&mut self) {
        self.cont.set_sleeping();
    }
    #[inline]
    pub fn reason(&self) -> &Reason {
        &self.reason
    }
    pub fn clear(&mut self, d: &Driver) {
        self.wake.clear();
        self.reason = Reason::None;
        self.cont.clear(d);
    }
    #[inline]
    pub fn set_reason(&mut self, r: Reason) {
        self.reason = r
    }
    #[inline]
    pub fn as_ptr(&mut self) -> EntryReference<'a> {
        unsafe { NonNull::new_unchecked(self) }
    }
    #[inline]
    pub fn set_timeout(&mut self, d: Option<Instant>) {
        self.wake = d.map_or(Time::ZERO, from_instant)
    }
    #[inline]
    pub fn is_ready_timeout(&mut self, n: &Time) -> bool {
        self.cont.set_ready_timeout(&mut self.wake, n)
    }
    #[inline]
    pub fn timeout(&self, v: &Time) -> Option<NonZeroU32> {
        if self.wake.is_zero() {
            None
        } else {
            NonZeroU32::new((self.wake.subtract(v).as_millis() & 0xFFFFFFFF) as u32)
        }
    }
    #[inline]
    pub fn lock(&'a mut self, d: &mut Driver) -> Option<State<'a>> {
        let n = self.as_ptr();
        let i = match self.lock.try_lock() {
            Err(TryLockError::WouldBlock) => return None,
            Ok(v) => v,
            _ => core::panic!(), // WUT
        };
        // TODO(dij): Watch for bugs
        self.suspend(d.empty());
        let t = {
            let v = self.cont.set_running();
            if unlikely(v.is_none()) {
                abort!();
            }
            // We verified that is's some so we're ok.
            unsafe { v.unwrap_unchecked() }
        };
        Some(State::new(n, unsafe { NonNull::new_unchecked(d) }, i, t))
    }
}
impl<'a> EntryMap<'a> {
    #[inline]
    pub fn new() -> EntryMap<'a> {
        EntryMap {
            n: 0u8,
            v: [NonNull::dangling(); Driver::SIZE],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.n as usize
    }
    #[inline]
    pub fn set_len(&mut self, v: usize) {
        self.n = v as u8
    }
    #[inline]
    pub fn get(&mut self, v: usize) -> Option<EntryReference<'a>> {
        // Truncate it to 255.
        if ((v & 0xFF) as u8) > self.n {
            None
        } else {
            Some(unsafe { *self.v.get_unchecked(v) })
        }
    }
    #[inline]
    pub fn set(&mut self, c: usize, n: usize, e: EntryReference<'a>) -> usize {
        // EntryMap is the same size as the Driver FD set, so we don't need to
        // check the size as the 'link' function (the caller) already did it for
        // the FD set.
        if n == 0 {
            return n;
        }
        // Can never overflow, the caller checks the length.
        //
        // Fill up the slots in the map slots.
        unsafe { self.v.get_unchecked_mut(c..c + n) }.fill(e);
        n
    }
}
impl<'a> EntryInner<'a> {
    #[inline]
    pub fn is_same(&mut self, ptr: *const ()) -> bool {
        /*println!(
            "check same {:?}, {ptr:?} --> {}",
            self.ptr,
            self.ptr.eq(&ptr)
        );*/
        if self.ptr.eq(&ptr) {
            return true;
        }
        self.ptr = ptr;
        false
    }
    #[inline]
    pub fn run<'b>(&mut self, ctx: &mut Context<'b>) -> bool {
        self.f.as_mut().poll(ctx).is_ready()
    }
}

impl Eq for Entry<'_> {}
impl Ord for Entry<'_> {
    #[inline]
    fn cmp(&self, other: &Entry<'_>) -> Ordering {
        self.wake.cmp(&other.wake)
    }
}
impl Deref for Entry<'_> {
    type Target = Container;

    #[inline]
    fn deref(&self) -> &Container {
        &self.cont
    }
}
impl DerefMut for Entry<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Container {
        &mut self.cont
    }
}
impl PartialEq for Entry<'_> {
    #[inline]
    fn eq(&self, other: &Entry<'_>) -> bool {
        self.wake.eq(&other.wake)
    }
}
impl PartialOrd for Entry<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Entry<'_>) -> Option<Ordering> {
        self.wake.partial_cmp(&other.wake)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::write;

    use crate::runtime::Entry;

    impl Debug for Entry<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "Entry({:?}) {:?}", self as *const Entry<'_>, self.cont)
        }
    }
}
