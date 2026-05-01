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

extern crate xrmt_stx;

use alloc::boxed::Box;
use core::cmp::PartialEq;
use core::future::Future;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::option::Option::Some;
use core::pin::Pin;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll};

use xrmt_stx::abort_unlikely;
use xrmt_stx::sync::extra::Flag;

use crate::future::{State, Status};
use crate::runtime::{EntryReference, Inner};

pub struct Link<'a> {
    ptr:  EntryReference<'a>,
    done: AtomicBool,
    flag: Flag,
}
pub struct Ticket<'a, T: 'a>(Box<Handle<'a, T>>);

struct Handle<'a, T> {
    data: MaybeUninit<T>,
    link: Link<'a>,
}
struct Connector<'a, T, F: Future<Output = T> + 'a> {
    f:    F,
    link: NonNull<Handle<'a, T>>,
}

impl Link<'_> {
    #[inline]
    pub fn pre(&self, e: &EntryReference<'_>) {
        if !self.ptr.cast().eq(e) {
            // Clear the Flag to indicate we're not ready since this isn't
            // the Entry we want.
            self.flag.clear();
        }
    }
    #[inline]
    pub fn post(&self, e: &EntryReference<'_>) -> bool {
        // Check it this Entry is ours.
        if !self.ptr.cast().eq(e) {
            return false; // Not ours, return it.
        }
        // Check if the initial Future is complete.
        if !self.done.load(Ordering::Acquire) {
            return false; // Not complete, return.
        }
        // The Future is done, mark it is done.
        unsafe { &*e.as_ptr() }.set_done();
        true // Tell the runtime we can bail.
    }
}
impl<'a, T> Ticket<'a, T> {
    #[inline]
    pub fn new<F: Future<Output = T> + 'a>(r: &mut Inner<'_, 'a>, f: F) -> Ticket<'a, T> {
        let mut t = Ticket(Box::new(Handle {
            data: MaybeUninit::uninit(),
            link: Link {
                ptr:  NonNull::dangling(),
                done: AtomicBool::new(false),
                flag: abort_unlikely!(Flag::new()),
            },
        }));
        t.0.link.ptr = r.add(Connector {
            f,
            link: unsafe { NonNull::new_unchecked(&mut *t.0) },
        });
        t.0.link.flag.set(); // Pre-mark flag as ready.
        t
    }

    #[inline]
    pub fn run(self, r: &mut Inner<'_, '_>) -> T {
        while r.pop(true, Some(&self.0.link)) {
            self.0.link.flag.set(); //  Set the Flag to indicate we are ready.
        }
        unsafe { self.0.data.assume_init() }
    }
}

impl<T, F: Future<Output = T>> Deref for Connector<'_, T, F> {
    type Target = F;

    #[inline]
    fn deref(&self) -> &F {
        &self.f
    }
}
impl<T, F: Future<Output = T>> DerefMut for Connector<'_, T, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut F {
        &mut self.f
    }
}
impl<'a, T, F: Future<Output = T>> Future for Connector<'a, T, F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let e = unsafe { &mut *self.link.as_ptr() };
        // Does work need to be done?
        if !e.link.done.load(Ordering::Acquire) {
            // Take out the Future since we can't directly Pin it oddly, but it won't move
            // so thats ok!
            let p = unsafe { self.as_mut().get_unchecked_mut() };
            let _ = match unsafe { Pin::new_unchecked(&mut p.f) }.poll(cx) {
                Poll::Ready(v) => e.data.write(v), // Write the completed result
                Poll::Pending => return Poll::Pending,
            };
            e.link.done.store(true, Ordering::Release); // Mark as done.
        }
        if let Status::Setup(v) = State::from_context(cx).status(&self) {
            // Set the Handle so we can trigger on it.
            v.clear();
            let _ = v.register_handle(false, &e.link.flag); // Won't error
        }
        // Always return Pending, the "owner" will mark us as "done".
        Poll::Pending
    }
}
