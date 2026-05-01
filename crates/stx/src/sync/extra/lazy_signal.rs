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

use core::clone::Clone;
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::marker::{PhantomData, Send, Sync};
use core::ops::FnOnce;
use core::option::Option::{self, None};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{self, Err, Ok};
use core::unreachable;

use crate::io::FmtResult;
use crate::sync::extra::inner::lazy_type::{Once, Reusable};
use crate::sync::extra::{Lazy, LazyHandle, LazyResult, SignalManual};

pub struct LazySignal<T, M: LazyType> {
    v:  Lazy<T>,
    s:  LazyHandle<SignalManual>,
    _p: PhantomData<M>,
}

pub trait LazyType {
    fn close_on_complete() -> bool;
}

pub type LazyOnce<T> = LazySignal<T, Once>;
pub type LazyReusable<T> = LazySignal<T, Reusable>;

impl<T> LazySignal<T, Reusable> {
    #[inline]
    pub fn reset(&mut self) -> Option<T> {
        let v = unsafe { self.v.take() };
        self.s.get().clear();
        v
    }
}
impl<T, M: LazyType> LazySignal<T, M> {
    #[inline]
    pub const fn new() -> LazySignal<T, M> {
        LazySignal {
            v:  Lazy::new(),
            s:  LazyHandle::new(),
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn wait(&self) {
        self.spin();
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        self.v.is_ready()
    }
    #[inline]
    pub fn get_unchecked(&self) -> &T {
        self.v.get_unchecked()
    }
    #[inline]
    pub fn get_no_init(&self) -> Option<&T> {
        self.v.get_no_init()
    }
    #[inline]
    pub fn get_mut_unchecked(&self) -> &mut T {
        self.v.get_mut_unchecked()
    }
    #[inline]
    pub fn get_mut_no_init(&self) -> Option<&mut T> {
        self.v.get_mut_no_init()
    }
    #[inline]
    pub fn get(&self, f: impl FnOnce() -> T) -> Result<&T, T> {
        self.get_mut(f).map(|v| &*v)
    }
    #[inline]
    pub fn get_mut(&self, f: impl FnOnce() -> T) -> Result<&mut T, T> {
        match self.v.lock::<!>(|| Ok(f()), || self.spin()) {
            LazyResult::Error(_) => unreachable!(),
            LazyResult::Ok => Ok(self.v.get_mut_unchecked()),
            LazyResult::Filled(v) => Err(v),
        }
    }
    #[inline]
    pub fn get_error<E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<&T, E> {
        self.get_error_mut(f).map(|v| &*v)
    }
    #[inline]
    pub fn get_error_mut<E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<&mut T, E> {
        match self.v.lock(f, || self.spin()) {
            LazyResult::Error(e) => Err(e),
            _ => Ok(self.v.get_mut_unchecked()),
        }
    }

    #[inline]
    fn spin(&self) {
        let _ = self.s.get().wait(None);
    }
}

impl<T> Debug for LazySignal<T, Once> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        core::write!(f, "LazySignal[Once]({})", match self.v.load() {
            0 => "Uninitialized",
            1 => "Spinning",
            2 => "Ready",
            _ => unreachable!(),
        })
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T> Debug for LazySignal<T, Reusable> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        core::write!(f, "LazySignal[Reusable]({})", match self.v.load() {
            0 => "Uninitialized",
            1 => "Spinning",
            2 => "Ready",
            _ => unreachable!(),
        })
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T, M: LazyType> Default for LazySignal<T, M> {
    #[inline]
    fn default() -> LazySignal<T, M> {
        LazySignal::new()
    }
}
impl<T: Clone, M: LazyType> Clone for LazySignal<T, M> {
    #[inline]
    fn clone(&self) -> LazySignal<T, M> {
        LazySignal {
            v:  self.v.clone(),
            s:  LazyHandle::new(),
            _p: PhantomData,
        }
    }
}

impl<T, M: LazyType> UnwindSafe for LazySignal<T, M> {}
impl<T, M: LazyType> RefUnwindSafe for LazySignal<T, M> {}

unsafe impl<T, M: LazyType> Send for LazySignal<T, M> {}
unsafe impl<T, M: LazyType> Sync for LazySignal<T, M> {}

impl LazyType for Once {
    #[inline]
    fn close_on_complete() -> bool {
        true
    }
}
impl LazyType for Reusable {
    #[inline]
    fn close_on_complete() -> bool {
        false
    }
}

pub mod lazy_type {
    pub struct Once(());
    pub struct Reusable(());
}
