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
#![cfg(windows)]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::mem::{self, MaybeUninit};
use core::ops::Deref;
use core::time::Duration;

use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle, Win32Error};
use crate::util::stx;
use crate::util::stx::io::{self, Error};
use crate::util::stx::prelude::*;

const STACK_SIZE: usize = 0x200000;

pub struct Builder {
    stack_size: Option<usize>,
}
pub struct ThreadId(u32);
pub struct JoinHandle<T> {
    thread: Thread,
    result: Arc<UnsafeCell<Option<T>>>,
}
pub struct Thread(OwnedHandle);

pub type Result<T> = io::Result<T>;

struct MaybeDangling<T>(MaybeUninit<T>);

impl Thread {
    #[inline]
    pub fn id(&self) -> ThreadId {
        winapi::GetThreadID(&self.0).map_or(ThreadId(0), |i| ThreadId(i))
    }
    #[inline]
    pub fn name(&self) -> Option<&str> {
        None
    }
}
impl Builder {
    #[inline]
    pub fn new() -> Builder {
        Builder { stack_size: None }
    }

    #[inline]
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Some(size);
        self
    }
    #[inline]
    pub fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static, {
        unsafe { self.spawn_unchecked(f) }
    }
    #[inline]
    pub unsafe fn spawn_unchecked<'a, F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'a,
        T: Send + 'a, {
        let x: Arc<UnsafeCell<Option<T>>> = Arc::new(UnsafeCell::new(None));
        let i = x.clone();
        let m = MaybeDangling::new(f);
        let func = move || {
            let r = (m.into_inner())();
            unsafe { *i.get() = Some(r) };
            drop(i);
        };
        let b = mem::transmute::<Box<dyn FnOnce() + 'a>, Box<dyn FnOnce() + 'static>>(Box::new(func));
        let a = Box::into_raw(Box::new(b));
        match winapi::CreateThreadEx(
            winapi::CURRENT_PROCESS,
            self.stack_size.unwrap_or(STACK_SIZE),
            thread_main as usize,
            a as *mut Box<dyn FnOnce()> as usize,
            false,
        ) {
            Err(e) => {
                drop(Box::from_raw(a));
                Err(e.into())
            },
            Ok(h) => {
                crate::bugtrack!("thread::spawn_unchecked(): Created a new thread 0x{h:X}!");
                Ok(JoinHandle { result: x, thread: Thread(h) })
            },
        }
    }
}
impl<T> JoinHandle<T> {
    #[inline]
    pub fn join(self) -> Result<T> {
        self.join_inner()
    }
    #[inline]
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
    #[inline]
    pub fn is_finished(&self) -> bool {
        Arc::strong_count(&self.result) == 1
    }
    #[inline]
    pub fn as_raw_handle(&self) -> Handle {
        *self.thread.0
    }

    #[inline]
    fn join_inner(mut self) -> Result<T> {
        winapi::WaitForSingleAsHandle(self.thread, -1, false).map_err(Error::from)?;
        stx::take(Arc::get_mut(&mut self.result))
            .get_mut()
            .take()
            .ok_or_else(|| Win32Error::IoPending.into())
    }
}
impl<T> MaybeDangling<T> {
    #[inline]
    fn new(x: T) -> MaybeDangling<T> {
        MaybeDangling(MaybeUninit::new(x))
    }

    #[inline]
    fn into_inner(self) -> T {
        let r = unsafe { self.0.assume_init_read() };
        mem::forget(self);
        r
    }
}

impl Deref for Thread {
    type Target = OwnedHandle;

    #[inline]
    fn deref(&self) -> &OwnedHandle {
        &self.0
    }
}
impl AsHandle for Thread {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.0
    }
}

impl Eq for ThreadId {}
impl Copy for ThreadId {}
impl Clone for ThreadId {
    #[inline]
    fn clone(&self) -> ThreadId {
        ThreadId(self.0)
    }
}
impl Deref for ThreadId {
    type Target = u32;

    #[inline]
    fn deref(&self) -> &u32 {
        &self.0
    }
}
impl PartialEq for ThreadId {
    #[inline]
    fn eq(&self, other: &ThreadId) -> bool {
        self.0 == other.0
    }
}

impl<T> AsHandle for JoinHandle<T> {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.thread.0
    }
}

impl<T> Drop for MaybeDangling<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.0.assume_init_drop() };
    }
}

impl From<Thread> for OwnedHandle {
    #[inline]
    fn from(v: Thread) -> OwnedHandle {
        v.0
    }
}
impl<T> From<JoinHandle<T>> for OwnedHandle {
    #[inline]
    fn from(v: JoinHandle<T>) -> OwnedHandle {
        v.thread.0
    }
}

unsafe impl<T> Send for JoinHandle<T> {}
unsafe impl<T> Sync for JoinHandle<T> {}

#[inline]
pub fn yield_now() {
    let _ = winapi::NtYieldExecution(); // IGNORE ERROR
}
#[inline]
pub fn current() -> Thread {
    // 0x1FFFFF - ALL_ACCESS
    Thread(winapi::OpenThread(0x1FFFFF, false, winapi::GetCurrentThreadID()).unwrap_or_default())
}
#[inline]
pub fn sleep(dur: Duration) {
    let _ = winapi::SleepEx(dur.as_micros() as i64, false); // IGNORE ERROR
}
#[inline]
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static, {
    stx::unwrap(Builder::new().spawn(f))
}

extern "system" fn thread_main(func: usize) -> u32 {
    unsafe { Box::from_raw(func as *mut Box<dyn FnOnce()>)() };
    0
}
