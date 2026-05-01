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

use core::marker::{Send, Sync};
use core::panic::{RefUnwindSafe, UnwindSafe};

pub use self::inner::*;

impl UnwindSafe for Flag {}
impl RefUnwindSafe for Flag {}

unsafe impl Send for Flag {}
unsafe impl Sync for Flag {}

#[cfg(target_family = "windows")]
mod inner {
    extern crate core;

    extern crate xrmt_winapi;

    use core::convert::AsRef;
    use core::fmt::{Debug, Formatter, Result};
    use core::mem::{transmute, ManuallyDrop};
    use core::ops::Deref;
    use core::option::Option::{None, Some};
    use core::result::Result::Ok;
    use core::time::Duration;

    use xrmt_winapi::functions::{duration_to_micros, PulseEvent, QueryEvent, ResetEvent, SetEvent, WaitForSingleObject};
    use xrmt_winapi::structs::OwnedHandle;
    use xrmt_winapi::INFINITE;

    use crate::abort_unlikely;
    use crate::io::IoResult;
    use crate::os::Handle;
    use crate::sync::extra::{LazyHandle, LazyValue, SignalManual};

    #[repr(transparent)]
    pub struct Flag(SignalManual);
    #[repr(transparent)]
    pub struct FlagHandle(Handle);
    #[repr(transparent)]
    pub struct FlagConstant(LazyHandle<Flag>);

    impl Flag {
        #[inline]
        pub fn new() -> IoResult<Flag> {
            Ok(Flag(SignalManual::new_error(false)?))
        }

        #[inline]
        pub fn set(&self) {
            self.0.set();
        }
        #[inline]
        pub fn wait(&self) {
            self.0.wait(None);
        }
        #[inline]
        pub fn clear(&self) {
            self.0.clear();
        }
        #[inline]
        pub fn is_set(&self) -> bool {
            self.0.is_set()
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) {
            self.0.wait(Some(d));
        }
        #[inline]
        pub fn handle(&self) -> FlagHandle {
            FlagHandle(*self.0.as_ref())
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            let _ = unsafe { OwnedHandle::take(&mut self.0) }.close();
        }
    }
    impl FlagHandle {
        #[inline]
        pub fn set(&self) {
            let _ = SetEvent(&self.0);
        }
        #[inline]
        pub fn wait(&self) {
            let _ = WaitForSingleObject(&self.0, INFINITE, false);
        }
        #[inline]
        pub fn pulse(&self) {
            let _ = PulseEvent(&self.0);
        }
        #[inline]
        pub fn reset(&self) {
            let _ = ResetEvent(&self.0);
        }
        #[inline]
        pub fn is_set(&self) -> bool {
            QueryEvent(&self.0).map_or(false, |v| v > 0)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) {
            let _ = WaitForSingleObject(&self.0, duration_to_micros(d), false);
        }
    }
    impl FlagConstant {
        #[inline]
        pub const fn new() -> FlagConstant {
            FlagConstant(LazyHandle::new())
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            if self.0.is_ready() {
                unsafe { self.0.take().close() };
            }
        }
    }

    impl Debug for Flag {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(self.0.as_ref(), f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for Flag {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }

    impl Debug for FlagHandle {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagHandle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }

    impl Debug for FlagConstant {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl Deref for FlagConstant {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            self.0.get()
        }
    }
    impl AsRef<Handle> for FlagConstant {
        #[inline]
        fn as_ref(&self) -> &Handle {
            self.0.get().as_ref()
        }
    }

    impl LazyValue for Flag {
        #[inline]
        fn lazy_new() -> isize {
            unsafe { transmute(ManuallyDrop::new(abort_unlikely!(Flag::new()))) }
        }
    }
}
#[cfg(all(
    not(target_os = "netbsd"),
    not(target_os = "solaris"),
    not(target_vendor = "apple"),
    not(target_family = "windows")
))]
mod inner {
    extern crate core;

    extern crate libc;

    use core::convert::AsRef;
    use core::fmt::{Debug, Formatter, Result};
    use core::mem::{drop, replace, ManuallyDrop};
    use core::ops::{Deref, Drop};
    use core::option::Option::{self, None, Some};
    use core::result::Result::{Err, Ok};
    use core::time::Duration;

    use libc::{close, eventfd, fcntl, poll, pollfd, read, write, EAI_AGAIN, EFD_CLOEXEC, F_SETFL, O_NONBLOCK, POLLIN};

    use crate::abort_unlikely;
    use crate::io::{IoError, IoResult};
    use crate::os::Handle;
    use crate::sync::extra::{LazyHandle, LazyValue, ZERO};

    pub struct Flag(Handle);
    pub struct FlagHandle(ManuallyDrop<Flag>);
    pub struct FlagConstant(LazyHandle<Flag>);

    impl Flag {
        pub fn new() -> IoResult<Flag> {
            match unsafe { eventfd(0, 0) } {
                -1 => Err(IoError::last_os_error()),
                f => {
                    let _ = unsafe { fcntl(f, F_SETFL, O_NONBLOCK | EFD_CLOEXEC) };
                    Ok(Flag(Handle::new(f)))
                },
            }
        }

        #[inline]
        pub fn set(&self) {
            let v = 1u64.to_ne_bytes();
            let _ = unsafe { write(*self.0, v.as_ptr() as _, 8) };
        }
        #[inline]
        pub fn wait(&self) {
            let _ = self.poll(None);
        }
        #[inline]
        pub fn clear(&self) {
            // Read until EAGAIN
            while self.read() {}
        }
        #[inline]
        pub fn is_set(&self) -> bool {
            self.poll(ZERO)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) {
            let _ = self.poll(Some(d));
        }
        #[inline]
        pub fn handle(&self) -> FlagHandle {
            FlagHandle(ManuallyDrop::new(Flag(self.0)))
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            unsafe { close(replace(&mut self.0, 0)) };
        }

        #[inline]
        fn read(&self) -> bool {
            let mut v = [0u8; 8];
            let r = unsafe { read(*self.0, v.as_mut_ptr() as _, 8) } as i32;
            r != EAI_AGAIN && r > 0
        }
        #[inline]
        fn poll(&self, dur: Option<Duration>) -> bool {
            let mut f = pollfd {
                fd:      *self.0,
                events:  POLLIN,
                revents: 0,
            };
            (unsafe { poll(&mut f, 1, dur.map_or(-1, |v| v.as_millis() as _)) }) > 0
        }
    }
    impl FlagConstant {
        #[inline]
        pub const fn new() -> FlagConstant {
            FlagConstant(LazyHandle::new())
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            if self.0.is_ready() {
                drop(unsafe { self.0.take() });
            }
        }
    }

    impl Drop for Flag {
        #[inline]
        fn drop(&mut self) {
            let _ = unsafe { close(*self.0) };
        }
    }
    impl Debug for Flag {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for Flag {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }

    impl Deref for FlagHandle {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            &self.0
        }
    }
    impl Debug for FlagHandle {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagHandle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0 .0
        }
    }

    impl Deref for FlagConstant {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            self.0.get()
        }
    }
    impl Debug for FlagConstant {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagConstant {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0 .0
        }
    }

    impl LazyValue for Flag {
        #[inline]
        fn lazy_new() -> isize {
            *ManuallyDrop::new(abort_unlikely!(Flag::new())).0 as isize
        }
    }
}
#[cfg(all(
    any(target_os = "netbsd", target_vendor = "apple"),
    not(target_family = "windows")
))]
mod inner {
    extern crate core;

    extern crate libc;

    use core::convert::AsRef;
    use core::fmt::{Debug, Formatter, Result};
    use core::mem::{drop, replace, zeroed, ManuallyDrop};
    use core::ops::{Deref, Drop};
    use core::option::Option::{self, None, Some};
    use core::ptr::{null, null_mut};
    use core::result::Result::{Err, Ok};
    use core::time::Duration;

    use libc::{close, fcntl, kevent, kqueue, timespec, EVFILT_USER, EV_ADD, EV_ENABLE, F_SETFL, NOTE_TRIGGER, O_CLOEXEC, O_NONBLOCK};

    use crate::abort_unlikely;
    use crate::io::{IoError, IoResult};
    use crate::os::Handle;
    use crate::sync::extra::{LazyHandle, LazyValue, ZERO};

    pub struct Flag(Handle);
    pub struct FlagHandle(ManuallyDrop<Flag>);
    pub struct FlagConstant(LazyHandle<Flag>);

    impl Flag {
        pub fn new() -> IoResult<Flag> {
            let f = unsafe { kqueue() };
            if f == -1 {
                return Err(IoError::last_os_error());
            }
            let _ = unsafe { fcntl(f, F_SETFL, O_NONBLOCK | O_CLOEXEC) };
            let e = Flag(Handle::new(f));
            e.trigger(false)?;
            Ok(e)
        }

        #[inline]
        pub fn set(&self) {
            let _ = self.trigger(true);
        }
        #[inline]
        pub fn wait(&self) {
            let _ = self.poll(None);
        }
        #[inline]
        pub fn clear(&self) {
            let _ = self.trigger(false);
        }
        #[inline]
        pub fn is_set(&self) -> bool {
            self.poll(ZERO)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) {
            let _ = self.poll(Some(d));
        }
        #[inline]
        pub fn handle(&self) -> FlagHandle {
            FlagHandle(ManuallyDrop::new(Flag(self.0)))
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            let _ = unsafe { close(replace(&mut self.0, 0)) };
        }

        #[inline]
        fn trigger(&self, en: bool) -> IoResult<()> {
            let (mut e, t) = unsafe { (zeroed::<kevent>(), zeroed::<timespec>()) };
            (e.ident, e.filter, e.flags) = (*self.0 as _, EVFILT_USER, EV_ADD | EV_ENABLE);
            if en {
                e.fflags = NOTE_TRIGGER;
            }
            if unsafe { kevent(*self.0, &e, 1, null_mut(), 0, &t) } == -1 {
                return Err(IoError::last_os_error());
            }
            Ok(())
        }
        #[inline]
        fn poll(&self, dur: Option<Duration>) -> bool {
            let mut e = unsafe { zeroed::<kevent>() };
            let r = unsafe {
                match dur {
                    Some(v) => {
                        let t = timespec {
                            tv_sec:  v.as_secs() as _,
                            tv_nsec: v.subsec_nanos() as _,
                        };
                        kevent(*self.0, null(), 0, &mut e, 1, &t)
                    },
                    None => kevent(*self.0, null(), 0, &mut e, 1, null()),
                }
            };
            r > 0
        }
    }
    impl FlagConstant {
        #[inline]
        pub const fn new() -> FlagConstant {
            FlagConstant(LazyHandle::new())
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            if self.0.is_ready() {
                drop(unsafe { self.0.take() });
            }
        }
    }

    impl Drop for Flag {
        #[inline]
        fn drop(&mut self) {
            let _ = unsafe { close(*self.0) };
        }
    }
    impl Debug for Flag {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for Flag {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0
        }
    }

    impl Deref for FlagHandle {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            &self.0
        }
    }
    impl Debug for FlagHandle {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagHandle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0 .0
        }
    }

    impl Deref for FlagConstant {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            self.0.get()
        }
    }
    impl Debug for FlagConstant {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagConstant {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0 .0
        }
    }

    impl LazyValue for Flag {
        #[inline]
        fn lazy_new() -> isize {
            *ManuallyDrop::new(abort_unlikely!(Flag::new())).0 as isize
        }
    }
}
#[cfg(target_os = "solaris")]
mod inner {
    extern crate core;

    extern crate libc;

    use core::convert::AsRef;
    use core::ffi::c_int;
    use core::fmt::{Debug, Formatter, Result};
    use core::mem::{replace, ManuallyDrop};
    use core::ops::{Deref, Drop};
    use core::option::Option::{self, None, Some};
    use core::result::Result::{Err, Ok};
    use core::time::Duration;

    use libc::{close, fcntl, pipe, poll, pollfd, read, write, EAI_AGAIN, F_SETFL, O_CLOEXEC, O_NONBLOCK, POLLIN};

    use crate::io::{IoError, IoResult};
    use crate::os::Handle;
    use crate::sync::extra::ZERO;

    pub struct Flag {
        r: Handle,
        w: Handle,
    }
    pub struct FlagHandle(ManuallyDrop<Flag>);

    impl Flag {
        pub fn new() -> IoResult<Flag> {
            let mut i = [0i32, 2];
            if unsafe { pipe(i.as_mut_ptr()) } != 0 {
                return Err(IoError::last_os_error());
            }
            unsafe {
                let _ = fcntl(i[0], F_SETFL, O_NONBLOCK | O_CLOEXEC);
                let _ = fcntl(i[1], F_SETFL, O_NONBLOCK | O_CLOEXEC);
            };
            Ok(Flag {
                r: Handle::new(i[0]),
                w: Handle::new(i[1]),
            })
        }

        #[inline]
        pub fn set(&self) {
            let v = 1u64.to_ne_bytes();
            let _ = unsafe { write(*self.w, v.as_ptr() as _, 8) };
        }
        #[inline]
        pub fn wait(&self) {
            let _ = self.poll(None);
        }
        #[inline]
        pub fn clear(&self) {
            // Read until EAGAIN
            while self.read() {}
        }
        #[inline]
        pub fn is_set(&self) -> bool {
            self.poll(ZERO)
        }
        #[inline]
        pub fn wait_for(&self, d: Duration) {
            let _ = self.poll(Some(d));
        }
        #[inline]
        pub fn handle(&self) -> FlagHandle {
            FlagHandle(ManuallyDrop::new(Flag { r: self.r, w: self.w }))
        }

        #[inline]
        pub unsafe fn close(&mut self) {
            unsafe {
                let _ = close(replace(&mut self.r, *Handle::EMPTY));
                let _ = close(replace(&mut self.w, *Handle::EMPTY));
            }
        }

        #[inline]
        fn read(&self) -> bool {
            let mut v = [0u8; 1];
            let r = unsafe { read(*self.r, v.as_mut_ptr() as _, 1) } as c_int;
            r != EAI_AGAIN && r > 0
        }
        #[inline]
        fn poll(&self, dur: Option<Duration>) -> bool {
            let mut f = pollfd {
                fd:      *self.r,
                events:  POLLIN,
                revents: 0,
            };
            (unsafe { poll(&mut f, 1, dur.map_or(-1, |v| v.as_millis() as _)) }) > 0
        }
    }

    impl Drop for Flag {
        #[inline]
        fn drop(&mut self) {
            unsafe {
                let _ = close(*self.r);
                let _ = close(*self.w);
            }
        }
    }
    impl Debug for Flag {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.r, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for Flag {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.r
        }
    }

    impl Deref for FlagHandle {
        type Target = Flag;

        #[inline]
        fn deref(&self) -> &Flag {
            &self.0
        }
    }
    impl Debug for FlagHandle {
        #[cfg(not(feature = "strip"))]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(&self.0, f)
        }
        #[cfg(feature = "strip")]
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Ok(())
        }
    }
    impl AsRef<Handle> for FlagHandle {
        #[inline]
        fn as_ref(&self) -> &Handle {
            &self.0.r
        }
    }
}
