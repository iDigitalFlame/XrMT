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

pub use self::inner::*;

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_stx;
    extern crate xrmt_winapi;

    use core::cell::UnsafeCell;
    use core::convert::{AsRef, From};
    use core::marker::Sync;
    use core::ops::{BitOrAssign, Drop};
    use core::option::Option::Some;
    use core::result::Result::{Err, Ok};
    use core::sync::atomic::{AtomicU32, Ordering};

    use xrmt_stx::io::{ErrorKind, IoError, IoResult};
    use xrmt_stx::sync::extra::Event;
    use xrmt_winapi::functions::{SetConsoleCtrlHandler, SetEvent};
    use xrmt_winapi::structs::Handle;

    use crate::signals::{Signal, SignalMask};

    const ENABLED: u32 = 0x80000000u32;

    static SIGNALS: SignalReceiver = SignalReceiver::new();

    pub struct SignalHandler {
        fd:     Event,
        ign:    SignalMask,
        active: SignalMask,
    }

    struct SignalReceiver {
        fd:      UnsafeCell<Handle>,
        sigs:    AtomicU32,
        enabled: AtomicU32,
    }

    impl SignalHandler {
        #[inline]
        pub fn new() -> IoResult<SignalHandler> {
            let e = Event::new()?;
            // Take ownership of the handler. Errors if someone else
            // already owns it.
            SIGNALS.setup(&e)?;
            Ok(SignalHandler {
                fd:     e,
                ign:    SignalMask::empty(),
                active: SignalMask::empty(),
            })
        }

        #[inline]
        pub fn read(&self, buf: &mut SignalMask) {
            // Trim off ignored Signals since Windows can't "ignore" signals.
            // We just read them but don't action on them.
            buf.bitor_assign(SIGNALS.read() ^ *self.ign);
        }
        #[inline]
        pub fn ignore(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.ign = if add { self.ign + v } else { self.ign - v };
            SIGNALS.set(self);
            Ok(())
        }
        #[inline]
        pub fn monitor(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.active = if add { self.active + v } else { self.active - v };
            SIGNALS.set(self);
            Ok(())
        }
    }
    impl SignalReceiver {
        #[inline]
        const fn new() -> SignalReceiver {
            SignalReceiver {
                fd:      UnsafeCell::new(Handle::EMPTY),
                sigs:    AtomicU32::new(0u32),
                enabled: AtomicU32::new(0u32),
            }
        }

        #[inline]
        fn reset(&self) {
            unsafe { &mut *self.fd.get() }.invalidate();
            self.sigs.store(0, Ordering::Release);
            self.enabled.store(0, Ordering::Release);
            let _ = SetConsoleCtrlHandler(signal_receiver, false);
        }
        #[inline]
        fn read(&self) -> u32 {
            self.sigs.swap(0, Ordering::Acquire)
        }
        #[inline]
        fn set(&self, s: &SignalHandler) {
            let _ = self.enabled.swap(ENABLED | *(s.active | s.ign), Ordering::Release);
        }
        fn receive(&self, sig: u32) -> bool {
            let v = self.enabled.load(Ordering::Acquire);
            // Check if the handler is even enabled. We use the MSB to flag as enabled.
            if v & ENABLED == 0 {
                return false;
            }
            // Check our Event Handle.
            let h = unsafe { &*self.fd.get() };
            // If it's invalid, bail.
            if h.is_invalid() {
                return false;
            }
            // Trim off MSB
            let m = SignalMask::new(v & 0x7FFFFFFF);
            // Convert to a Signal to be used with the mask
            let s = match Signal::from_signo(sig as i32) {
                Some(x) if m.contains(&x) => x,
                // If we're not monitoring that Signal or it's not valid,
                // return false.
                _ => return false,
            };
            // Append the mask to our received Signals list.
            let _ = self.sigs.fetch_or(s.mask(), Ordering::Release);
            // Let everyone know we got something.
            let _ = SetEvent(h);
            true // Tell Windows we got this.
        }
        #[inline]
        fn setup(&self, e: &Event) -> IoResult<()> {
            if self.enabled.load(Ordering::Acquire) & ENABLED != 0 {
                return Err(IoError::from(ErrorKind::ResourceBusy));
            }
            SetConsoleCtrlHandler(signal_receiver, true)?;
            unsafe { *self.fd.get() = *e.as_ref() };
            self.enabled.fetch_or(ENABLED, Ordering::Release);
            Ok(())
        }
    }

    impl Drop for SignalHandler {
        #[inline]
        fn drop(&mut self) {
            // Reset the SignalReceiver so someone else can have it.
            SIGNALS.reset();
        }
    }
    impl AsRef<Handle> for SignalHandler {
        #[inline]
        fn as_ref(&self) -> &Handle {
            self.fd.as_ref()
        }
    }

    unsafe impl Sync for SignalReceiver {}

    unsafe extern "system" fn signal_receiver(sig: u32) -> u32 {
        if SIGNALS.receive(sig) {
            1
        } else {
            0
        }
    }
}
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "solaris"),
    not(target_os = "illumos"),
    not(target_vendor = "apple"),
    not(target_family = "windows")
))]
mod inner {
    extern crate alloc;
    extern crate core;

    extern crate libc;
    extern crate xrmt_stx;

    use alloc::vec::Vec;
    use core::convert::{AsRef, Into};
    use core::mem::{size_of, zeroed};
    use core::ops::{BitOrAssign, Drop};
    use core::option::Option::{None, Some};
    use core::ptr::null_mut;
    use core::result::Result::Ok;
    use core::slice::from_raw_parts_mut;

    use libc::{close, fcntl, read, sigaddset, sigemptyset, signalfd, signalfd_siginfo, sigprocmask, sigset_t, FD_CLOEXEC, F_SETFL, O_NONBLOCK, SIGCHLD, SIG_BLOCK, SIG_UNBLOCK};
    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::Handle;

    use crate::errcheck;
    use crate::signals::{Signal, SignalMask};

    pub struct SignalHandler {
        fd:     Handle,
        ign:    SignalMask,
        active: SignalMask,
    }

    impl SignalHandler {
        #[inline]
        pub fn new() -> IoResult<SignalHandler> {
            let s = unsafe { zeroed() };
            let f = errcheck!(signalfd(-1, &s, 0))?;
            errcheck!(fcntl(f, F_SETFL, O_NONBLOCK | FD_CLOEXEC))?;
            Ok(SignalHandler {
                fd:     Handle::new(f),
                ign:    SignalMask::empty(),
                active: SignalMask::empty(),
            })
        }

        #[inline]
        pub fn proc(&mut self, add: bool, _pid: u32) -> IoResult<()> {
            if !add {
                return Ok(());
            }
            if !self.active.contains(&Signal::Child) {
                // Nothing to do but monitor SIGCHLD unlike BSD which uses kevent.
                self.monitor(true, Signal::Child)?;
            }
            Ok(())
        }
        pub fn read(&self, proc: &mut Vec<u32>, buf: &mut SignalMask) {
            let (n, mut i) = (size_of::<signalfd_siginfo>(), unsafe {
                zeroed::<signalfd_siginfo>()
            });
            let b = unsafe { from_raw_parts_mut(&mut i as *mut signalfd_siginfo as *mut u8, n) };
            loop {
                let r = unsafe { read(*self.fd, b.as_ptr() as _, n) } as usize;
                if r < n {
                    break;
                }
                if i.ssi_signo == 0 {
                    continue;
                }
                if i.ssi_signo == SIGCHLD as _ && i.ssi_pid > 0 {
                    proc.push(i.ssi_pid);
                }
                let s = match Signal::from_signo(i.ssi_signo as i32) {
                    Some(v) => v,
                    None => continue,
                };
                if !self.active.contains(&s) || self.ign.contains(&s) {
                    continue;
                }
                buf.bitor_assign(s);
            }
        }
        #[inline]
        pub fn ignore(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.ign = if add { self.ign + v } else { self.ign - v };
            let s = sigset_from_mask(&self.ign)?;
            errcheck!(sigprocmask(SIG_BLOCK, &s, null_mut()))?;
            // TODO(dij): Need to check if Signals removed from the mask will be
            //            reset when applied, since the docs says it "replaces" the
            //            old mask, so are previous ones not valid anymore?
            // BUG(dij): ^Need to test.
            Ok(())
        }
        #[inline]
        pub fn monitor(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.active = if add { self.active + v } else { self.active - v };
            let s = sigset_from_mask(&self.active)?;
            errcheck!(sigprocmask(SIG_BLOCK, &s, null_mut()))?;
            // TODO(dij): Need to check if Signals removed from the mask will be
            //            reset when applied, since the docs says it "replaces" the
            //            old mask, so are previous ones not valid anymore?
            // BUG(dij): ^Need to test.
            self.fd.set(errcheck!(signalfd(*self.fd, &s, 0))?);
            Ok(())
        }
    }

    impl Drop for SignalHandler {
        #[inline]
        fn drop(&mut self) {
            // Clear all blocked signals
            if let Ok(s) = sigset_from_mask(&self.ign) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            if let Ok(s) = sigset_from_mask(&self.active) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            // Close the signalfd handle.
            if !self.fd.is_invalid() {
                let _ = unsafe { close(*self.fd) };
            }
        }
    }
    impl AsRef<Handle> for SignalHandler {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.fd
        }
    }

    fn sigset_from_mask(v: &SignalMask) -> IoResult<sigset_t> {
        let mut s = unsafe { zeroed::<sigset_t>() };
        errcheck!(sigemptyset(&mut s))?;
        if v.is_empty() {
            return Ok(s);
        }
        for i in v.iter() {
            if i.is_invalid() {
                continue;
            }
            if let Some(v) = i.into() {
                errcheck!(sigaddset(&mut s, v))?;
            }
        }
        Ok(s)
    }
}
#[cfg(all(
    not(target_family = "windows"),
    any(target_os = "netbsd", target_os = "freebsd", target_vendor = "apple")
))]
mod inner {
    extern crate alloc;
    extern crate core;

    extern crate libc;
    extern crate xrmt_stx;

    use alloc::vec::Vec;
    use core::convert::{AsRef, Into};
    use core::mem::zeroed;
    use core::ops::{BitOrAssign, Drop};
    use core::option::Option::{None, Some};
    use core::ptr::{null, null_mut};
    use core::result::Result::Ok;

    use libc::{close, fcntl, kevent, kqueue, sigaddset, sigemptyset, sigprocmask, sigset_t, timespec, EVFILT_PROC, EVFILT_SIGNAL, EV_ADD, EV_CLEAR, EV_DELETE, EV_ENABLE, EV_ERROR, FD_CLOEXEC, F_SETFL, NOTE_EXIT, O_NONBLOCK, SIG_BLOCK, SIG_UNBLOCK};
    use xrmt_stx::io::IoResult;
    use xrmt_stx::os::Handle;

    use crate::errcheck;
    use crate::signals::{Signal, SignalMask};

    pub struct SignalHandler {
        fd:     Handle,
        ign:    SignalMask,
        active: SignalMask,
    }

    impl SignalHandler {
        #[inline]
        pub fn new() -> IoResult<SignalHandler> {
            let f = errcheck!(kqueue())?;
            errcheck!(fcntl(f, F_SETFL, O_NONBLOCK | FD_CLOEXEC))?;
            Ok(SignalHandler {
                fd:     Handle::new(f),
                ign:    SignalMask::empty(),
                active: SignalMask::empty(),
            })
        }

        #[inline]
        pub fn proc(&mut self, add: bool, pid: u32) -> IoResult<()> {
            let mut e = unsafe { zeroed::<kevent>() };
            e.flags = if add { EV_ADD | EV_ENABLE | EV_CLEAR } else { EV_DELETE };
            (e.ident, e.filter, e.fflags) = (pid as _, EVFILT_PROC, NOTE_EXIT);
            errcheck!(kevent(*self.fd, &e, 1, null_mut(), 0, null_mut()))?;
            Ok(())
        }
        pub fn read(&self, proc: &mut Vec<u32>, buf: &mut SignalMask) {
            let (mut b, t) = unsafe { (zeroed::<kevent>(), zeroed::<timespec>()) };
            loop {
                if unsafe { kevent(*self.fd, null(), 0, &mut b, 1, &t) } != 1 {
                    break;
                }
                if b.flags & EV_ERROR == 0 {
                    continue;
                }
                match b.filter {
                    EVFILT_PROC => proc.push(b.ident as u32),
                    EVFILT_SIGNAL => {
                        let s = match Signal::from_signo(b.ident as i32) {
                            Some(v) => v,
                            None => continue,
                        };
                        if !self.active.contains(&s) || self.ign.contains(&s) {
                            continue;
                        }
                        buf.bitor_assign(s);
                    },
                    _ => (),
                }
            }
        }
        #[inline]
        pub fn ignore(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.ign = if add { self.ign + v } else { self.ign - v };
            sigset_block(&self.ign)
        }
        #[inline]
        pub fn monitor(&mut self, add: bool, v: Signal) -> IoResult<()> {
            let n = self.active.update(add, v);
            let mut e = unsafe { zeroed::<kevent>() };
            // *BSD just needs to remove/add the difference, so we'll XOR them
            // instead.
            for i in self.active ^ n {
                let s: i32 = match i.into() {
                    Some(s) => s,
                    None => continue,
                };
                e.flags = if add {
                    EV_ADD | EV_ENABLE | EV_CLEAR
                } else {
                    EV_DELETE | EV_DELETE
                };
                (e.ident, e.filter) = (s as _, EVFILT_SIGNAL);
                errcheck!(kevent(*self.fd, &e, 1, null_mut(), 0, null_mut()))?;
            }
            Ok(())
        }
    }

    impl Drop for SignalHandler {
        #[inline]
        fn drop(&mut self) {
            // Clear all blocked signals
            if let Ok(s) = sigset_from_mask(&self.ign) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            if let Ok(s) = sigset_from_mask(&self.active) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            // Close the signalfd handle.
            if !self.fd.is_invalid() {
                let _ = unsafe { close(*self.fd) };
            }
        }
    }
    impl AsRef<Handle> for SignalHandler {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.fd
        }
    }

    #[inline]
    fn sigset_block(v: &SignalMask) -> IoResult<()> {
        // TODO(dij): Need to check if Signals removed from the mask will be
        //            reset when applied, since the docs says it "replaces" the
        //            old mask, so are previous ones not valid anymore?
        // BUG(dij): ^Need to test.
        let s = sigset_from_mask(v)?;
        errcheck!(sigprocmask(SIG_BLOCK, &s, null_mut()))?;
        Ok(())
    }
    fn sigset_from_mask(v: &SignalMask) -> IoResult<sigset_t> {
        let mut s = unsafe { zeroed::<sigset_t>() };
        errcheck!(sigemptyset(&mut s))?;
        if v.is_empty() {
            return Ok(s);
        }
        for i in v.iter() {
            if i.is_invalid() {
                continue;
            }
            if let Some(v) = i.into() {
                errcheck!(sigaddset(&mut s, v))?;
            }
        }
        Ok(s)
    }
}
#[cfg(any(target_os = "solaris", target_os = "illumos"))]
mod inner {
    extern crate alloc;
    extern crate core;

    extern crate libc;
    extern crate xrmt_stx;

    use alloc::vec::Vec;
    use core::cell::UnsafeCell;
    use core::convert::{AsRef, From, Into};
    use core::hint::unlikely;
    use core::marker::Sync;
    use core::mem::zeroed;
    use core::ops::{BitAnd, BitOrAssign, Drop};
    use core::option::Option::{None, Some};
    use core::ptr::null_mut;
    use core::result::Result::{Err, Ok};
    use core::sync::atomic::{AtomicU32, Ordering};

    use libc::{sigaction, sigaddset, sigemptyset, siginfo_t, sigprocmask, sigset_t, SA_NOCLDSTOP, SA_SIGINFO, SIGCHLD, SIG_BLOCK, SIG_DFL, SIG_UNBLOCK};
    use xrmt_stx::io::{ErrorKind, IoError, IoResult};
    use xrmt_stx::os::Handle;
    use xrmt_stx::sync::extra::Event;

    use crate::errcheck;
    use crate::signals::{Signal, SignalMask};

    const ENABLED: u32 = 0x80000000u32;

    static SIGNALS: SignalReceiver = SignalReceiver::new();

    pub struct SignalHandler {
        fd:     Event,
        ign:    SignalMask,
        active: SignalMask,
    }

    struct SignalReceiver {
        fd:   UnsafeCell<Handle>,
        pids: UnsafeCell<Vec<u32>>,
        sigs: AtomicU32,
    }

    impl SignalReceiver {
        #[inline]
        const fn new() -> SignalReceiver {
            SignalReceiver {
                fd:   UnsafeCell::new(Handle::EMPTY),
                pids: UnsafeCell::new(Vec::new()),
                sigs: AtomicU32::new(0u32),
            }
        }

        #[inline]
        fn reset(&self) {
            unsafe { **self.fd.get() = 0 };
            self.sigs.store(0, Ordering::Release);
        }
        fn receive(&self, info: &siginfo_t) {
            let r = self
                .sigs
                .fetch_update(Ordering::Release, Ordering::Acquire, |v| {
                    if v & ENABLED == 0 {
                        None
                    } else {
                        Some(v | info.si_code as u32)
                    }
                })
                .is_err();
            if unlikely(r) {
                return;
            }
            if info.si_code == SIGCHLD {
                let p = unsafe { info.si_pid() };
                if p != 0 {
                    unsafe { &mut *self.pids.get() }.push(p as u32);
                }
            }
        }
        #[inline]
        fn read(&self, b: &mut Vec<u32>) -> u32 {
            // Disable the handler to prevent modification.
            let r = self.sigs.swap(0, Ordering::Acquire) & 0x7FFFFFFF;
            let p = unsafe { &mut *self.pids.get() };
            b.copy_from_slice(p.as_slice()); // TODO(dij): fix
            p.clear();
            // Re-enable handler.
            self.sigs.store(ENABLED, Ordering::Release);
            r
        }
        #[inline]
        fn setup(&self, e: &Event) -> IoResult<()> {
            if unsafe { **self.fd.get() } != 0 {
                return Err(IoError::from(ErrorKind::ResourceBusy));
            }
            unsafe { *self.fd.get() = *e.as_ref() };
            let _ = self.sigs.swap(ENABLED, Ordering::Acquire);
            Ok(())
        }
    }
    impl SignalHandler {
        #[inline]
        pub fn new() -> IoResult<SignalHandler> {
            let e = Event::new()?;
            SIGNALS.setup(&e)?;
            Ok(SignalHandler {
                fd:     e,
                ign:    SignalMask::empty(),
                active: SignalMask::empty(),
            })
        }

        #[inline]
        pub fn proc(&mut self, add: bool, _pid: u32) -> IoResult<()> {
            if !add {
                return Ok(());
            }
            if !self.active.contains(&Signal::Child) {
                // Nothing to do but monitor SIGCHLD unlike BSD which uses kevent.
                self.monitor(true, Signal::Child)?;
            }
            Ok(())
        }
        #[inline]
        pub fn read(&self, proc: &mut Vec<u32>, buf: &mut SignalMask) {
            // Only take monitored signals
            buf.bitor_assign(self.active.bitand(SIGNALS.read(proc)))
        }
        #[inline]
        pub fn ignore(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.ign = if add { self.ign + v } else { self.ign - v };
            let s = sigset_from_mask(&self.ign)?;
            errcheck!(sigprocmask(SIG_BLOCK, &s, null_mut()))?;
            // TODO(dij): Need to check if Signals removed from the mask will be
            //            reset when applied, since the docs says it "replaces" the
            //            old mask, so are previous ones not valid anymore?
            // BUG(dij): ^Need to test.
            Ok(())
        }
        #[inline]
        pub fn monitor(&mut self, add: bool, v: Signal) -> IoResult<()> {
            self.active = if add { self.active + v } else { self.active - v };
            let s = sigset_from_mask(&self.active)?;
            errcheck!(sigprocmask(SIG_BLOCK, &s, null_mut()))?;
            // TODO(dij): Need to check if Signals removed from the mask will be
            //            reset when applied, since the docs says it "replaces" the
            //            old mask, so are previous ones not valid anymore?
            // BUG(dij): ^Need to test.
            sig_attach(add, v)
        }
    }

    impl Drop for SignalHandler {
        #[inline]
        fn drop(&mut self) {
            // Clear all blocked signals
            if let Ok(s) = sigset_from_mask(&self.ign) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            if let Ok(s) = sigset_from_mask(&self.active) {
                let _ = unsafe { sigprocmask(SIG_UNBLOCK, &s, null_mut()) };
            }
            for i in self.active {
                let _ = sig_attach(false, i);
            }
            // Reset the SignalReceiver so someone else can have it.
            SIGNALS.reset();
        }
    }
    impl AsRef<Handle> for SignalHandler {
        #[inline]
        fn as_ref(&self) -> &Handle {
            self.fd.as_ref()
        }
    }

    unsafe impl Sync for SignalReceiver {}

    fn sig_attach(add: bool, v: Signal) -> IoResult<()> {
        let i = match v.signo() {
            Some(v) => v,
            None => return Ok(()),
        };
        if !add {
            errcheck!(sigaction(i, SIG_DFL as _, null_mut()))?;
            return Ok(());
        }
        let mut a = unsafe { zeroed::<sigaction>() };
        a.sa_sigaction = signal_receiver as *const () as _;
        a.sa_flags = SA_SIGINFO | SA_NOCLDSTOP;
        errcheck!(sigaction(i, &a, null_mut()))?;
        Ok(())
    }
    fn sigset_from_mask(v: &SignalMask) -> IoResult<sigset_t> {
        let mut s = unsafe { zeroed::<sigset_t>() };
        errcheck!(sigemptyset(&mut s))?;
        if v.is_empty() {
            return Ok(s);
        }
        for i in v.iter() {
            if i.is_invalid() {
                continue;
            }
            if let Some(v) = i.into() {
                errcheck!(sigaddset(&mut s, v))?;
            }
        }
        Ok(s)
    }

    unsafe extern "C" fn signal_receiver(_sig: u32, info: *const siginfo_t, _ctx: *const ()) {
        SIGNALS.receive(unsafe { &*info });
    }
}
