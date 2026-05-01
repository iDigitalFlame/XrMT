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

use core::cmp::PartialOrd;
use core::default::Default;
use core::hint::unlikely;
use core::mem::align_of;
use core::option::Option::{self, None, Some};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU8, Ordering};
use core::unreachable;

use xrmt_stx::os::Handle;
use xrmt_stx::time::extra::Time;

use crate::runtime::container::queue::KQueue;
use crate::runtime::container::slot::{ContainerSlot, Slot};
use crate::runtime::Driver;

const DONE: u8 = 7u8;
const READY: u8 = 0u8;
const RUNNING: u8 = 1u8;
const SLEEPING: u8 = 2u8;

pub struct Container {
    kq:    KQueue, // Manages KQueue/IOCP Handles
    fds:   [Handle; 3],
    state: AtomicU8,
    // State is a Bitmap
    // - This will keep details of the overall state but also the details for all the Handles kept inside.
    //
    // | 128    | 64     | 32     | 16       8 | 4       | 2          1 |
    // | S0 R/W | S1 R/W | S2 R/W | Slot Count | Timeout | Status [0-3] |
    slots: [ContainerSlot; 3],
}
pub struct ContainerHandle<T>(NonNull<T>);

impl Container {
    pub const KQUEUE: bool = KQueue::ENABLED;

    #[inline]
    pub fn new() -> Container {
        Container {
            kq:    KQueue::new(),
            fds:   [Handle::default(), Handle::default(), Handle::default()],
            state: AtomicU8::new(READY),
            slots: [ContainerHandle::new(), ContainerHandle::new(), ContainerHandle::new()],
        }
    }

    #[inline]
    pub fn resume(&self) {
        let (n, w, r) = self.counts();
        // 0x3 - Bit 128 (shifted down 5), S0 R/W
        // 0x2 - Bit  64 (shifted down 5), S1 R/W
        // 0x1 - Bit  32 (shifted down 5), S2 R/W
        match n {
            0 => return,
            1 => unsafe {
                self.slots.get_unchecked(0).set(r, w & 0x4 != 0, self.fds.get_unchecked(0));
            },
            2 => unsafe {
                self.slots.get_unchecked(0).set(r, w & 0x4 != 0, self.fds.get_unchecked(0));
                self.slots.get_unchecked(1).set(r, w & 0x2 != 0, self.fds.get_unchecked(1));
            },
            3 => unsafe {
                self.slots.get_unchecked(0).set(r, w & 0x4 != 0, self.fds.get_unchecked(0));
                self.slots.get_unchecked(1).set(r, w & 0x2 != 0, self.fds.get_unchecked(1));
                self.slots.get_unchecked(2).set(r, w & 0x1 != 0, self.fds.get_unchecked(2));
            },
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn set_done(&self) {
        // 0xF8 - Bitmask for 0b11111000
        //        Clears only the Status + Timeout bits
        //        'DONE' states are all bottom 3 bits set 0b111 (7)
        let _ = self
            .state
            .update(Ordering::Release, Ordering::Acquire, |v| (v & 0xF8) | DONE);
    }
    #[inline]
    pub fn count(&self) -> u8 {
        // 0x3 - Bitmask for 0b11
        //       Shows only the Slot Count bits
        unsafe { self.state.load(Ordering::Acquire).unchecked_shr(3) & 0x3 }
    }
    #[inline]
    pub fn set_sleeping(&self) {
        // 0xF8 - Bitmask for 0b11111000
        //        Clears only the Status + Timeout bits
        let _ = self.state.update(Ordering::Release, Ordering::Acquire, |v| {
            (v & 0xF8) | SLEEPING
        });
        self.resume();
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        // 0x7 - Bitmask for 0b111
        //       Shows only the Status + Timeout bits
        //       If all the Status bits are '1' (0b111 = 7), this indicates
        //       the 'DONE' state.
        self.state.load(Ordering::Acquire) & 0x7 == DONE
    }
    #[inline]
    pub fn set_ready(&self) -> bool {
        // 0x3  - Bitmask for 0b11
        //        Shows only thr Status bits
        // 0xFC - Bitmask for 0b11111100
        //        Clears only the Status bits
        //        Keep the TIMEOUT bit here so we know if it did timeout.
        self.state
            .fetch_update(Ordering::Release, Ordering::Acquire, |v| match v & 0x3 {
                // TODO(dij): Should we allow READY to return true?
                //            I mean mark would always return true then?
                SLEEPING => Some((v & 0xFC) | READY),
                DONE | READY | RUNNING => None,
                _ => unreachable!(),
            })
            .is_ok()
    }
    #[inline]
    pub fn is_avaliable(&self) -> bool {
        // 0x3 - Bitmask for 0b11
        //       Shows only the Status bits
        // Check for valid state. We shouldn't get anything thats not "good" to run.
        match self.state.load(Ordering::Acquire) & 0x3 {
            READY | SLEEPING => true,
            DONE | RUNNING => false,
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn clear(&mut self, d: &Driver) {
        // 0x7 - Bitmask for 0b111
        //       Clear everything except the Status + Timeout bits.
        // Reset Handles and State.
        self.fds.fill(Handle::default());
        self.kq.clear(d);
        self.state.fetch_and(0x3, Ordering::Release);
    }
    #[inline]
    pub fn suspend(&self, zero: &Handle) {
        match self.count() {
            0 => return,
            1 => unsafe { self.slots.get_unchecked(0).clear(zero) },
            2 => unsafe {
                self.slots.get_unchecked(0).clear(zero);
                self.slots.get_unchecked(1).clear(zero);
            },
            3 => unsafe {
                self.slots.get_unchecked(0).clear(zero);
                self.slots.get_unchecked(1).clear(zero);
                self.slots.get_unchecked(2).clear(zero);
            },
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn add_queue(&mut self, h: &Handle) {
        self.kq.add(h);
    }
    #[inline]
    pub fn set_running(&self) -> Option<bool> {
        // 0x3  - Bitmask for 0b11
        //        Shows only the Status bits
        // 0xFC - Bitmask for 0b11111000
        //        Clears only the Status + Timeout bits
        // 0x4  - Bitmask for 0b100
        //        Shows only the Timeout bit
        // Clear the timeout bit here, it'll be returned to us.
        self.state
            .fetch_update(Ordering::Release, Ordering::Acquire, |v| match v & 0x3 {
                READY | SLEEPING => Some((v & 0xF8) | RUNNING),
                DONE | RUNNING => None,
                _ => unreachable!(),
            })
            .ok()
            .map(|v| v & 0x4 != 0) // Check the Timeout bit
    }
    /*#[inline]
    pub fn in_queue(&mut self, h: &Handle) -> bool {
        self.kq.contains(h)
    }*/
    pub fn add_handle(&mut self, write: bool, h: &Handle) -> bool {
        let n = self.count();
        if n >= 0x3 {
            return false;
        }
        match n {
            // 0x67 - Bitmask for 0b01100111
            //        Clears the S0 R/W and the count bits
            //        Setting to one, so it'll be updated to '01' (0x8)
            //        If write is 'true', the 0x80 bit will be set.
            0 => {
                let _ = self.state.update(Ordering::Release, Ordering::Acquire, |v| {
                    (v & 0x67) | 0x8 | if write { 0x80 } else { 0 }
                });
                // Always in bounds
                unsafe { *self.fds.get_unchecked_mut(0) = *h };
            },
            // 0xA7 - Bitmask for 0b10100111
            //        Clears the S1 R/W and the count bits
            //        Setting to two, so it'll be updated to '10' (0x10)
            //        If write is 'true', the 0x40 bit will be set.
            1 => {
                let _ = self.state.update(Ordering::Release, Ordering::Acquire, |v| {
                    (v & 0xA7) | 0x10 | if write { 0x40 } else { 0 }
                });
                // Always in bounds
                unsafe { *self.fds.get_unchecked_mut(1) = *h };
            },
            // 0xA7 - Bitmask for 0b11000111
            //        Clears the S2 R/W and the count bits
            //        Setting to three, so it'll be updated to '11' (0x18)
            //        If write is 'true', the 0x40 bit will be set.
            2 => {
                let _ = self.state.update(Ordering::Release, Ordering::Acquire, |v| {
                    (v & 0xC7) | 0x18 | if write { 0x20 } else { 0 }
                });
                // Always in bounds
                unsafe { *self.fds.get_unchecked_mut(2) = *h };
            },
            _ => unreachable!(),
        }
        true
    }
    #[inline]
    pub fn set_ready_timeout(&self, t: &mut Time, now: &Time) -> bool {
        // 0x3  - Bitmask for 0b11
        //        Shows only the Status bits
        // 0xF8 - Bitmask for 0b11111100
        //        Clears only the Status bits
        // 0x4  - Bitmask for 0b100
        //        Sets only the Timeout bit
        // Keep the TIMEOUT bit here so we know if it did timeout.
        // Unless we need to set it now, so we will.
        self.state
            .fetch_update(Ordering::Release, Ordering::Acquire, |v| match v & 0x3 {
                SLEEPING if !t.is_zero() && now.ge(t) => {
                    t.clear(); // Add the Timeout bit (4).
                    Some((v & 0xF8) | 0x4 | READY)
                },
                READY => Some((v & 0xF8) | READY),
                DONE | RUNNING | SLEEPING => None,
                _ => unreachable!(),
            })
            .is_ok()
    }
    pub fn link(&mut self, zero: &Handle, v: &mut [Slot]) -> Option<usize> {
        let (n, w, r) = self.counts();
        if unlikely(n > v.len()) {
            return None;
        }
        // This one is more complex since we set the value of the
        // Slot here if we're not RUNNING otherwise we set the 'zero' value.
        //
        // This function combines 'resume' and 'suspend' but does less math work
        // to determine Slot status.
        //
        // 0x4 - Bit 128 (shifted down 5), S0 R/W
        // 0x2 - Bit  64 (shifted down 5), S1 R/W
        // 0x1 - Bit  32 (shifted down 5), S2 R/W
        match n {
            0 => return Some(0),
            1 => unsafe {
                // Always in bounds
                self.slots
                    .get_unchecked_mut(0)
                    .link(v.get_unchecked_mut(0))
                    .setup(r, w & 0x4 != 0, zero, self.fds.get_unchecked(0));
            },
            2 => unsafe {
                // Always in bounds
                self.slots
                    .get_unchecked_mut(0)
                    .link(v.get_unchecked_mut(0))
                    .setup(r, w & 0x4 != 0, zero, self.fds.get_unchecked(0));
                self.slots
                    .get_unchecked_mut(1)
                    .link(v.get_unchecked_mut(1))
                    .setup(r, w & 0x2 != 0, zero, self.fds.get_unchecked(1));
            },
            3 => unsafe {
                // Always in bounds
                self.slots
                    .get_unchecked_mut(0)
                    .link(v.get_unchecked_mut(0))
                    .setup(r, w & 0x4 != 0, zero, self.fds.get_unchecked(0));
                self.slots
                    .get_unchecked_mut(1)
                    .link(v.get_unchecked_mut(1))
                    .setup(r, w & 0x2 != 0, zero, self.fds.get_unchecked(1));
                self.slots
                    .get_unchecked_mut(2)
                    .link(v.get_unchecked_mut(2))
                    .setup(r, w & 0x1 != 0, zero, self.fds.get_unchecked(2));
            },
            _ => unreachable!(),
        }
        Some(n)
    }

    #[inline]
    fn counts(&self) -> (usize, u8, bool) {
        let v = self.state.load(Ordering::Acquire);
        (
            unsafe { (v.unchecked_shr(3) & 0x3) as usize },
            unsafe { v.unchecked_shr(5) },
            v & 0x1 == RUNNING,
        )
    }
}
impl<T> ContainerHandle<T> {
    #[inline]
    const fn new() -> ContainerHandle<T> {
        ContainerHandle(NonNull::dangling())
    }

    #[inline]
    pub fn slot(&self) -> &mut T {
        unsafe { &mut *self.0.as_ptr() }
    }
    #[inline]
    pub fn is_dangling(&self) -> bool {
        self.0.as_ptr() as usize == align_of::<T>()
    }
    #[inline]
    pub fn link(&mut self, v: &mut T) -> &mut ContainerHandle<T> {
        self.0 = unsafe { NonNull::new_unchecked(v) };
        self
    }
}

#[cfg(target_family = "windows")]
mod slot {
    extern crate xrmt_stx;

    use xrmt_stx::os::Handle;

    use crate::runtime::ContainerHandle;

    pub type Slot = Handle;
    pub type ContainerSlot = ContainerHandle<Handle>;

    impl ContainerSlot {
        #[inline]
        pub fn clear(&self, h: &Handle) {
            if self.is_dangling() {
                return;
            }
            *self.slot() = *h;
        }
        #[inline]
        pub fn set(&self, act: bool, _w: bool, h: &Handle) {
            if !act || self.is_dangling() {
                return;
            }
            *self.slot() = *h;
        }
        #[inline]
        pub fn setup(&self, act: bool, _w: bool, zero: &Handle, h: &Handle) {
            // No dangling check here as we just set the value anyway, so
            // it's not possible to dangle.
            *self.slot() = if act { *zero } else { *h };
        }
    }
}
#[cfg(not(target_family = "windows"))]
mod slot {
    extern crate libc;
    extern crate xrmt_stx;

    use libc::pollfd;
    use xrmt_stx::os::Handle;

    use crate::runtime::{ContainerHandle, FD_READ, FD_WRITE};

    pub type Slot = pollfd;
    pub type ContainerSlot = ContainerHandle<pollfd>;

    impl ContainerSlot {
        #[inline]
        pub fn clear(&self, h: &Handle) {
            if self.is_dangling() {
                return;
            }
            let v = self.slot();
            (v.events, v.revents, v.fd) = (0, 0, **h);
        }
        #[inline]
        pub fn set(&self, act: bool, w: bool, h: &Handle) {
            if act || self.is_dangling() {
                return;
            }
            let v = self.slot();
            v.events = if w { FD_WRITE } else { FD_READ };
            (v.revents, v.fd) = (0, **h);
        }
        #[inline]
        pub fn setup(&self, act: bool, w: bool, zero: &Handle, h: &Handle) {
            // No dangling check here as we just set the value anyway, so
            // it's not possible to dangle.
            let v = self.slot();
            (v.revents, v.events) = (0, if w { FD_WRITE } else { FD_READ });
            v.fd = if act { **zero } else { **h };
        }
    }
}

#[cfg(any(
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_vendor = "apple",
    target_family = "windows"
))]
// For any OS's that support a KQueue-like mechanism.
mod queue {
    extern crate alloc;

    extern crate xrmt_stx;

    use alloc::vec::Vec;

    use xrmt_stx::os::Handle;

    use crate::runtime::Driver;

    pub struct KQueue(Vec<Handle>);

    impl KQueue {
        pub const ENABLED: bool = true;

        #[inline]
        pub fn new() -> KQueue {
            KQueue(Vec::new())
        }

        #[inline]
        pub fn add(&mut self, h: &Handle) {
            self.0.push(*h);
        }
        #[inline]
        pub fn clear(&mut self, d: &Driver) {
            for i in 0..self.0.len() {
                let _ = d.queue_remove(unsafe { self.0.get_unchecked(i) });
            }
            self.0.clear();
        }
        /*#[inline]
        pub fn contains(&self, h: &Handle) -> bool {
            self.0.contains(h)
        }*/
    }
}
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "openbsd"),
    not(target_os = "dragonfly"),
    not(target_vendor = "apple"),
    not(target_family = "windows")
))]
mod queue {
    extern crate xrmt_stx;

    use xrmt_stx::os::Handle;

    use crate::runtime::Driver;

    pub struct KQueue(());

    impl KQueue {
        pub const ENABLED: bool = false;

        #[inline]
        pub fn new() -> KQueue {
            KQueue(())
        }

        #[inline]
        pub fn add(&mut self, _h: &Handle) {}
        #[inline]
        pub fn clear(&mut self, _d: &Driver) {}
        /*#[inline]
        pub fn contains(&self, _h: &Handle) -> bool {
            false
        }*/
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::sync::atomic::Ordering;
    use core::unreachable;

    use crate::runtime::container::{READY, RUNNING, SLEEPING};
    use crate::runtime::Container;

    impl Debug for Container {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            if self.is_done() {
                return f.write_str("Done");
            }
            match self.state.load(Ordering::Acquire) & 0x3 {
                READY => f.write_str("Ready"),
                RUNNING => f.write_str("Running"),
                SLEEPING => f.write_str("Sleeping"),
                _ => unreachable!(),
            }
        }
    }
}
