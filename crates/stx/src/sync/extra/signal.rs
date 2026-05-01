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

extern crate xrmt_winapi;

use core::convert::{AsRef, From};
use core::marker::PhantomData;
use core::mem::{transmute, ManuallyDrop};
use core::ops::{Deref, DerefMut};
use core::option::Option::{self, None};
use core::result::Result::Ok;
use core::time::Duration;

use xrmt_winapi::functions::{duration_option_to_micros, CreateEvent, CreateKeyedEvent, OpenEvent, OpenKeyedEvent, PulseEvent, QueryEvent, ResetEvent, SetEvent, SetKeyedEvent, WaitForKeyedEvent, WaitForSingleObject};
use xrmt_winapi::structs::OwnedHandle;
use xrmt_winapi::Win32Result;

use crate::abort_unlikely;
use crate::os::Handle;
use crate::sync::extra::signal_type::{Auto, Manual};
use crate::sync::extra::LazyValue;

#[repr(transparent)]
pub struct KeySignal(OwnedHandle);
#[repr(transparent)]
pub struct Signal<T: SignalType> {
    v:  OwnedHandle,
    _p: PhantomData<T>,
}

pub trait SignalType {
    fn is_manual() -> bool;
}

pub type SignalAuto = Signal<Auto>;
pub type SignalManual = Signal<Manual>;

impl KeySignal {
    #[inline]
    pub fn new() -> KeySignal {
        abort_unlikely!(KeySignal::new_error())
    }
    #[inline]
    pub fn new_error() -> Win32Result<KeySignal> {
        Ok(KeySignal(CreateKeyedEvent(None, false, None)?))
    }
    #[inline]
    pub fn open(name: impl AsRef<str>) -> Win32Result<KeySignal> {
        // 0x1F0003 - FULL_CONTROL
        Ok(KeySignal(OpenKeyedEvent(0x1F0003, false, name.as_ref())?))
    }
    #[inline]
    pub fn new_with_name(name: impl AsRef<str>) -> Win32Result<KeySignal> {
        Ok(KeySignal(CreateKeyedEvent(None, false, name.as_ref())?))
    }

    #[inline]
    pub fn set(&self, key: usize, dur: Option<Duration>) -> bool {
        unsafe { self.set_raw(key, dur) }.is_ok()
    }
    #[inline]
    pub fn wait(&self, key: usize, dur: Option<Duration>) -> bool {
        unsafe { self.wait_raw(key, dur) }.is_ok()
    }

    #[inline]
    pub unsafe fn set_raw(&self, key: usize, dur: Option<Duration>) -> Win32Result<()> {
        SetKeyedEvent(&self.0, key, duration_option_to_micros(dur), false)
    }
    #[inline]
    pub unsafe fn wait_raw(&self, key: usize, dur: Option<Duration>) -> Win32Result<()> {
        WaitForKeyedEvent(&self.0, key, duration_option_to_micros(dur), false)
    }
}
impl<T: SignalType> Signal<T> {
    #[inline]
    pub fn new(initial: bool) -> Signal<T> {
        abort_unlikely!(Self::new_error(initial))
    }
    #[inline]
    pub fn new_error(initial: bool) -> Win32Result<Signal<T>> {
        Ok(Signal {
            v:  CreateEvent(None, false, initial, T::is_manual(), None)?,
            _p: PhantomData,
        })
    }
    #[inline]
    pub fn open(name: impl AsRef<str>) -> Win32Result<Signal<T>> {
        // 0x1F0003 - FULL_CONTROL
        Ok(Signal {
            v:  OpenEvent(0x1F0003, false, name.as_ref())?,
            _p: PhantomData,
        })
    }
    #[inline]
    pub fn new_with_name(initial: bool, name: impl AsRef<str>) -> Win32Result<Signal<T>> {
        Ok(Signal {
            v:  CreateEvent(None, false, initial, T::is_manual(), name.as_ref())?,
            _p: PhantomData,
        })
    }

    #[inline]
    pub fn set(&self) {
        let _ = SetEvent(&self.v);
    }
    #[inline]
    pub fn clear(&self) {
        let _ = ResetEvent(&self.v);
    }
    #[inline]
    pub fn pulse(&self) {
        let _ = PulseEvent(&self.v);
    }
    #[inline]
    pub fn is_set(&self) -> bool {
        QueryEvent(&self.v).map_or(false, |v| v > 0)
    }
    #[inline]
    pub fn wait(&self, dur: Option<Duration>) -> bool {
        match WaitForSingleObject(&self.v, duration_option_to_micros(dur), false) {
            Ok(0) => true,
            _ => false,
        }
    }

    #[inline]
    pub unsafe fn clear_raw(&self) -> Win32Result<()> {
        ResetEvent(&self.v)
    }
    #[inline]
    pub unsafe fn set_raw(&self, dur: Option<Duration>) -> Win32Result<u32> {
        WaitForSingleObject(&self.v, duration_option_to_micros(dur), false)
    }
}

impl AsRef<Handle> for KeySignal {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

impl<T: SignalType> Deref for Signal<T> {
    type Target = OwnedHandle;

    #[inline]
    fn deref(&self) -> &OwnedHandle {
        &self.v
    }
}
impl<T: SignalType> DerefMut for Signal<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut OwnedHandle {
        &mut self.v
    }
}
impl<T: SignalType> AsRef<Handle> for Signal<T> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.v
    }
}

impl SignalType for Auto {
    #[inline]
    fn is_manual() -> bool {
        false
    }
}
impl SignalType for Manual {
    #[inline]
    fn is_manual() -> bool {
        true
    }
}

impl LazyValue for SignalAuto {
    #[inline]
    fn lazy_new() -> isize {
        unsafe { transmute(ManuallyDrop::new(SignalAuto::new(false))) }
    }
}
impl LazyValue for SignalManual {
    #[inline]
    fn lazy_new() -> isize {
        unsafe { transmute(ManuallyDrop::new(SignalManual::new(false))) }
    }
}

impl From<KeySignal> for OwnedHandle {
    #[inline]
    fn from(v: KeySignal) -> OwnedHandle {
        v.0
    }
}
impl<T: SignalType> From<Signal<T>> for OwnedHandle {
    #[inline]
    fn from(v: Signal<T>) -> OwnedHandle {
        v.v
    }
}

pub mod signal_type {
    pub struct Auto(());
    pub struct Manual(());
}
