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

use core::option::Option::{self, None, Some};

use crate::future::State;
use crate::signals::SignalMask;

pub enum Reason {
    None,
    Process(u32),
    Signal(SignalMask),
}
pub enum PollResult {
    None,
    Wake,
    Signal,
    Entry(usize),
    #[cfg(any(
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_vendor = "apple",
        target_family = "windows"
    ))]
    /// KQueue/IOCP only
    Pointer(*mut ()),
}
pub enum QueueResult<'a> {
    None,
    Empty,
    Shutdown,
    Entry(State<'a>),
}

impl Reason {
    #[inline]
    pub fn pid(&self) -> Option<u32> {
        match self {
            Reason::Process(v) => Some(*v),
            _ => None,
        }
    }
    #[inline]
    pub fn is_signal(&self) -> Option<&SignalMask> {
        match self {
            Reason::Signal(v) => Some(v),
            _ => None,
        }
    }
    #[inline]
    pub fn is_pid(&self, pids: &[u32]) -> Option<u32> {
        match self {
            Reason::Process(v) if pids.contains(v) => Some(*v),
            _ => None,
        }
    }
}
impl PollResult {
    #[inline]
    pub fn is_none(&self) -> bool {
        match self {
            PollResult::None | PollResult::Signal => true,
            _ => false,
        }
    }
}
