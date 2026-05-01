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
#![cfg(target_family = "windows")]

extern crate core;

extern crate xrmt_stx;
extern crate xrmt_winapi;

use core::convert::AsRef;
use core::mem::size_of;
use core::num::NonZeroU32;
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::result::Result::Ok;
use core::slice::from_raw_parts;

use xrmt_stx::io::IoResult;
use xrmt_stx::println;
use xrmt_stx::sync::extra::Event;
use xrmt_winapi::functions::wait_for_multiple_objects;
use xrmt_winapi::structs::{Handle, IoCompletionPort};
use xrmt_winapi::INFINITE;

use crate::runtime::{Entries, Entry, EntryReference, EntrySlots, PollResult};
use crate::signals::{SignalHandler, SignalMask};

pub struct Driver {
    fds:  Handles,
    iop:  IoCompletionPort,
    sigs: SignalHandler,
}

#[repr(C)]
struct Handles {
    event:  Handle,
    signal: Handle,
    iop:    Handle,
    fds:    [Handle; Driver::SIZE],
    count:  u8,
}

impl Driver {
    pub const SIZE: usize = 61usize;

    #[inline]
    pub fn new(wake: &Event) -> IoResult<Driver> {
        let s = SignalHandler::new()?;
        let i = IoCompletionPort::new()?;
        Ok(Driver {
            fds:  Handles::new(wake.as_ref(), i.as_ref(), &s),
            iop:  i,
            sigs: s,
        })
    }

    #[inline]
    pub fn empty(&self) -> &Handle {
        self.iop.as_ref()
    }
    #[inline]
    pub fn signals(&mut self) -> &mut SignalHandler {
        &mut self.sigs
    }
    #[inline]
    pub fn queue_remove(&self, h: &Handle) -> IoResult<()> {
        Ok(self.iop.remove(h)?)
    }
    #[inline]
    pub fn update<'a>(&mut self, q: EntrySlots<'a, '_>, e: Entries<'a, '_>) -> usize {
        self.fds.update(q, e)
    }
    #[inline]
    pub fn poll(&mut self, q: EntrySlots, e: Entries, dur: Option<NonZeroU32>) -> PollResult {
        let r = unsafe {
            let h = self.fds.ptr();
            wait_for_multiple_objects(
                h,
                h.len(),
                false,
                dur.map_or(INFINITE, |v| v.get().saturating_mul(1_000) as u64),
                false,
            )
        };
        println!("read {r:?}, {}", q.len());
        match r {
            // Wake Event was trigged
            Ok(0x000) => PollResult::Wake,
            // Signal Event was triggered
            Ok(0x001) => self.signal(e),
            // IOCP hit.
            Ok(0x002) => self.iocp(),
            // Timeout
            Ok(0x102) => PollResult::None,
            Ok(0x040..) => PollResult::None, // Abandoned Wait
            Ok(v) => PollResult::Entry(unsafe { v.unchecked_sub(Handles::OFFSET as u32) as usize }),
            _ => PollResult::None,
        }
    }
    #[inline]
    pub fn queue_register(&self, _write: bool, h: &Handle, e: &mut Entry<'_>) -> IoResult<()> {
        Ok(self.iop.add(&h, e)?)
    }

    fn iocp(&mut self) -> PollResult {
        let mut r = PollResult::None;
        // When IOCP is triggered, poll the IOCP to see what has returned.
        // The "key" is the Event pointer, so we can directly pull the source
        // Entry with it.
        loop {
            let mut e: EntryReference = NonNull::dangling();
            // We ignore the returned bytes as the Overlapped associated
            // with the File should have it updated for us.
            if self.iop.status(&mut e, 0).is_err() {
                break;
            }
            println!("addr {e:?}");
            // Dangling check.
            if e.as_ptr() as usize == size_of::<usize>() {
                continue;
            }
            if unsafe { &mut *e.as_ptr() }.mark() && r.is_none() {
                r = PollResult::Pointer(e.as_ptr() as _);
            }
        }
        r
    }
    fn signal(&mut self, e: Entries) -> PollResult {
        let mut s = SignalMask::empty();
        self.sigs.read(&mut s);
        for i in 0..e.len() {
            let x = unsafe { e.get_unchecked_mut(i) };
            if !x.reason().is_signal().map(|v| v.contains_any(&s)).unwrap_or(false) {
                continue;
            }
            if x.mark() {
                return PollResult::Entry(i);
            }
        }
        PollResult::Signal
    }
}
impl Handles {
    const OFFSET: u8 = 3u8;

    #[inline]
    fn new(h: &Handle, iop: &Handle, sigs: &SignalHandler) -> Handles {
        Handles {
            fds:    [Handle::EMPTY; Driver::SIZE],
            iop:    *iop,
            count:  0u8,
            event:  *h,
            signal: *sigs.as_ref(),
        }
    }

    #[inline]
    fn ptr(&self) -> &[usize] {
        unsafe {
            from_raw_parts(
                self as *const Handles as *const usize,
                (self.count + Handles::OFFSET) as usize,
            )
        }
    }
    fn update<'a>(&mut self, q: EntrySlots<'a, '_>, e: Entries<'a, '_>) -> usize {
        let mut n = 0usize;
        for i in 0..e.len() {
            if n >= self.fds.len() {
                break;
            }
            let b = unsafe { self.fds.get_unchecked_mut(n..) };
            let x = unsafe { e.get_unchecked_mut(i) };
            n += match x.link(&self.iop, b) {
                Some(v) => q.set(n, v, x.as_ptr()),
                None => break,
            };
        }
        self.count = n as u8;
        n
    }
}
