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

use crate::device::winapi;
use crate::util::stx::prelude::*;

#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct Session {
    pub user:       String,
    pub host:       String,
    pub domain:     String,
    pub login_time: i64,
    pub last_input: i64,
    pub id:         u32,
    pub addr:       [u8; 16],
    pub is_remote:  bool,
    pub status:     u8,
}
#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct SessionProcess {
    pub name:       String,
    pub user:       String,
    pub session_id: u32,
    pub pid:        u32,
}
#[repr(transparent)]
pub struct SessionHandle(pub usize);

impl Clone for Session {
    #[inline]
    fn clone(&self) -> Session {
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
impl Clone for SessionProcess {
    #[inline]
    fn clone(&self) -> SessionProcess {
        SessionProcess {
            pid:        self.pid,
            name:       self.name.clone(),
            user:       self.user.clone(),
            session_id: self.session_id,
        }
    }
}

impl Drop for SessionHandle {
    #[inline]
    fn drop(&mut self) {
        if self.0 == 0 {
            return;
        }
        winapi::WTSCloseServer(self)
    }
}
impl Default for SessionHandle {
    #[inline]
    fn default() -> SessionHandle {
        SessionHandle(0)
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::SessionHandle;
    use crate::util::stx::prelude::*;

    impl Debug for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "SessionHandle: 0x{:X}", self.0)
        }
    }
    impl LowerHex for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for SessionHandle {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
