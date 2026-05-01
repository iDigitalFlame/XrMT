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

// TODO(dij): Update

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::default::Default;
use core::ops::{Deref, Drop};

use xrmt_data::Fiber;

use crate::structs::SysTime;

#[repr(transparent)]
pub struct SessionHandle(usize);
pub struct Session<A: Allocator = Global> {
    pub user:       Fiber<A>,
    pub host:       Fiber<A>,
    pub domain:     Fiber<A>,
    pub login_time: SysTime,
    pub last_input: SysTime,
    pub id:         u32,
    pub addr:       [u8; 16],
    pub is_remote:  bool,
    pub status:     u8,
}
pub struct SessionProcess<A: Allocator = Global> {
    pub name:       Fiber<A>,
    pub user:       Fiber<A>,
    pub session_id: u32,
    pub pid:        u32,
}

impl SessionHandle {
    #[inline]
    pub const fn empty() -> SessionHandle {
        SessionHandle(0)
    }
}
impl Eq for SessionHandle {}
impl Drop for SessionHandle {
    #[inline]
    fn drop(&mut self) {
        if self.0 == 0 {
            return;
        }
        // TODO(dij):
        // WTSCloseServer(self)
    }
}
impl Deref for SessionHandle {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        &self.0
    }
}
impl Default for SessionHandle {
    #[inline]
    fn default() -> SessionHandle {
        SessionHandle::empty()
    }
}
impl PartialEq for SessionHandle {
    #[inline]
    fn eq(&self, other: &SessionHandle) -> bool {
        self.0 == other.0
    }
}

impl<A: Allocator + Clone> Clone for Session<A> {
    #[inline]
    fn clone(&self) -> Session<A> {
        Session {
            id:         self.id,
            user:       self.user.clone(),
            host:       self.host.clone(),
            addr:       self.addr,
            domain:     self.domain.clone(),
            status:     self.status,
            is_remote:  self.is_remote,
            login_time: self.login_time,
            last_input: self.last_input,
        }
    }
}
impl<A: Allocator + Clone> Clone for SessionProcess<A> {
    #[inline]
    fn clone(&self) -> SessionProcess<A> {
        SessionProcess {
            pid:        self.pid,
            name:       self.name.clone(),
            user:       self.user.clone(),
            session_id: self.session_id,
        }
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::structs::{Session, SessionHandle, SessionProcess};

    impl Debug for Session {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("Session")
                .field("user", &self.user)
                .field("host", &self.host)
                .field("domain", &self.domain)
                .field("login_time", &*self.login_time)
                .field("last_input", &*self.last_input)
                .field("id", &self.id)
                .field("addr", &self.addr)
                .field("is_remote", &self.is_remote)
                .field("status", &self.status)
                .finish()
        }
    }

    impl Debug for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "SessionHandle: 0x{:X}", self.0)
        }
    }
    impl LowerHex for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            UpperHex::fmt(&self.0, f)
        }
    }

    impl Debug for SessionProcess {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("SessionProcess")
                .field("name", &self.name)
                .field("user", &self.user)
                .field("session_id", &self.session_id)
                .field("pid", &self.pid)
                .finish()
        }
    }
}
