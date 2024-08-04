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
#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, Ordering};

use crate::prelude::*;

pub(super) struct State(AtomicU32);

impl State {
    pub const READY: u32 = 0x2u32;
    pub const CLOSED: u32 = 0x4u32;
    pub const CLOSING: u32 = 0x8u32;
    pub const SHUTDOWN: u32 = 0x10u32;
    pub const SEND_CLOSE: u32 = 0x20u32;
    pub const RECV_CLOSE: u32 = 0x40u32;
    pub const WAKE_CLOSE: u32 = 0x80u32;
    pub const CHANNEL: u32 = 0x100u32;
    pub const CHANNEL_VALUE: u32 = 0x200u32;
    pub const CHANNEL_UPDATED: u32 = 0x400u32;
    pub const CHANNEL_PROXY: u32 = 0x800u32;
    pub const SEEN: u32 = 0x1000u32;
    pub const MOVING: u32 = 0x2000u32;
    pub const REPLACING: u32 = 0x4000u32;
    pub const SHUTDOWN_WAIT: u32 = 0x8000u32;

    #[inline]
    pub fn new() -> State {
        State(AtomicU32::new(0))
    }

    #[inline]
    pub fn tag(&self) -> bool {
        if !self.seen() {
            return false;
        }
        self.unset(State::SEEN);
        true
    }
    #[inline]
    pub fn last(&self) -> u16 {
        (self.0.load(Ordering::Acquire) >> 16) as u16
    }
    #[inline]
    pub fn set(&self, v: u32) {
        self.0.fetch_or(v, Ordering::AcqRel);
    }
    #[inline]
    pub fn seen(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::SEEN != 0
    }
    #[inline]
    pub fn unset(&self, v: u32) {
        self.0.fetch_and(!v, Ordering::AcqRel);
    }
    #[inline]
    pub fn set_last(&self, v: u16) {
        let _ = self.0.fetch_update(Ordering::Acquire, Ordering::Relaxed, |s| {
            Some((v as u32) << 16 | (s as u16) as u32)
        });
    }
    #[inline]
    pub fn is_ready(&self) -> bool {
        !self.is_closed() && self.0.load(Ordering::Acquire) & State::READY != 0
    }
    #[inline]
    pub fn is_moving(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::MOVING != 0
    }
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::CLOSED != 0
    }
    #[inline]
    pub fn in_channel(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::CHANNEL != 0
    }
    #[inline]
    pub fn is_closing(&self) -> bool {
        self.is_closed() || self.0.load(Ordering::Acquire) & State::CLOSING != 0
    }
    #[inline]
    pub fn is_send_closed(&self) -> bool {
        self.is_closed() || self.0.load(Ordering::Acquire) & State::SEND_CLOSE != 0
    }
    #[inline]
    pub fn is_replacing(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::REPLACING != 0
    }
    #[inline]
    pub fn channel_value(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::CHANNEL_VALUE != 0
    }
    #[inline]
    pub fn channel_proxy(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::CHANNEL_PROXY != 0
    }
    #[inline]
    pub fn channel_updated(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::CHANNEL_UPDATED != 0
    }
    #[inline]
    pub fn is_shutdown_wait(&self) -> bool {
        self.0.load(Ordering::Acquire) & State::SHUTDOWN_WAIT != 0
    }
    #[inline]
    pub fn is_shutdown(&self) -> bool {
        self.is_closed() || self.0.load(Ordering::Acquire) & State::SHUTDOWN != 0
    }
    #[inline]
    pub fn can_channel_stop(&self) -> bool {
        if self.is_closing() || !self.in_channel() {
            return true;
        }
        if self.channel_updated() {
            self.unset(State::CHANNEL_UPDATED);
            return !self.channel_value();
        }
        !self.in_channel()
    }
    #[inline]
    pub fn can_channel_start(&self) -> bool {
        if self.is_closed() {
            return false;
        }
        if self.in_channel() {
            return true;
        }
        self.channel_value()
    }
    #[inline]
    pub fn set_channel(&self, enable: bool) -> bool {
        if enable {
            if self.channel_value() {
                return false;
            }
            self.set(State::CHANNEL_VALUE);
        } else {
            if (!self.in_channel() || !self.channel_proxy()) && !self.channel_value() {
                return false;
            }
            self.unset(State::CHANNEL_VALUE);
        }
        self.set(State::CHANNEL_UPDATED);
        true
    }
}
