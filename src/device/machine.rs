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

use alloc::alloc::Global;
use core::alloc::Allocator;

use crate::data::str::Fiber;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::device::machine::capabilities::Capabilities;
use crate::device::{hostname_in, Network, ID};
use crate::prelude::*;
use crate::process::{id, parent_id};
use crate::{io, process};

mod arch;
pub mod capabilities;
mod os;

pub struct Machine<A: Allocator = Global> {
    pub user:         Fiber<A>,
    pub version:      Fiber<A>,
    pub hostname:     Fiber<A>,
    pub network:      Network<A>,
    pub pid:          u32,
    pub ppid:         u32,
    pub capabilities: Capabilities,
    pub id:           ID,
    pub system:       u8,
    pub elevated:     u8,
}

impl Machine {
    #[inline]
    pub fn local() -> io::Result<Machine> {
        Machine::local_in(Global)
    }
}
impl<A: Allocator> Machine<A> {
    #[inline]
    pub fn os(&self) -> os::OS {
        os::OS::from(self.system >> 4)
    }
    #[inline]
    pub fn is_elevated(&self) -> bool {
        self.elevated & 1 == 1
    }
    #[inline]
    pub fn is_domain_joined(&self) -> bool {
        self.os() == os::OS::Windows && self.elevated & 0x80 != 0
    }
    #[inline]
    pub fn arch(&self) -> arch::Architecture {
        arch::Architecture::from(self.system & 0xF)
    }
}
impl<A: Allocator + Clone> Machine<A> {
    #[inline]
    pub fn empty_in(alloc: A) -> Machine<A> {
        Machine {
            network:      Network::new_in(alloc.clone()),
            hostname:     Fiber::new_in(alloc.clone()),
            id:           ID::default(),
            pid:          0u32,
            ppid:         0u32,
            user:         Fiber::new_in(alloc.clone()),
            system:       0u8,
            version:      Fiber::new_in(alloc.clone()),
            elevated:     0u8,
            capabilities: capabilities::NULL,
        }
    }
    #[inline]
    pub fn local_in(alloc: A) -> io::Result<Machine<A>> {
        Ok(Machine {
            network:      Network::local_in(alloc.clone())?,
            hostname:     hostname_in(alloc.clone())?,
            id:           local_id(),
            pid:          id(),
            ppid:         parent_id(),
            user:         local::username(alloc.clone()),
            system:       local::system(),
            version:      local::version(alloc.clone()),
            elevated:     local::elevated(),
            capabilities: capabilities::ENABLED,
        })
    }

    #[inline]
    pub fn refresh(&mut self) -> io::Result<()> {
        self.network.refresh()?;
        self.hostname = hostname_in(self.hostname.allocator())?;
        self.pid = process::id();
        self.ppid = process::parent_id();
        self.user = local::username(self.user.allocator());
        self.elevated = local::elevated();
        Ok(())
    }
}

impl Default for Machine {
    #[inline]
    fn default() -> Machine {
        Machine {
            id:           ID::default(),
            pid:          0u32,
            ppid:         0u32,
            user:         Fiber::new(),
            system:       0u8,
            version:      Fiber::new(),
            network:      Network::new(),
            hostname:     Fiber::new(),
            elevated:     0u8,
            capabilities: capabilities::NULL,
        }
    }
}
impl<A: Allocator> Writable for Machine<A> {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        self.id.write_stream(w)?;
        w.write_u8(self.system)?;
        w.write_u32(self.pid)?;
        w.write_u32(self.ppid)?;
        w.write_str(&self.user)?;
        w.write_str(&self.version)?;
        w.write_str(&self.hostname)?;
        w.write_u8(self.elevated)?;
        w.write_u32(self.capabilities.0)?;
        self.network.write_stream(w)
    }
}
impl<A: Allocator + Clone> Clone for Machine<A> {
    #[inline]
    fn clone(&self) -> Machine<A> {
        Machine {
            id:           self.id,
            pid:          self.pid,
            user:         self.user.clone(),
            ppid:         self.ppid,
            system:       self.system,
            version:      self.version.clone(),
            network:      self.network.clone(),
            hostname:     self.hostname.clone(),
            elevated:     self.elevated,
            capabilities: self.capabilities,
        }
    }
}
impl<A: Allocator + Clone> Readable for Machine<A> {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        self.id.read_stream(r)?;
        r.read_into_u8(&mut self.system)?;
        r.read_into_u32(&mut self.pid)?;
        r.read_into_u32(&mut self.ppid)?;
        r.read_into_fiber(&mut self.user)?;
        r.read_into_fiber(&mut self.version)?;
        r.read_into_fiber(&mut self.hostname)?;
        r.read_into_u8(&mut self.elevated)?;
        r.read_into_u32(&mut self.capabilities.0)?;
        self.network.read_stream(r)
    }
}

#[inline]
pub fn local_id() -> ID {
    ID::from(local::system_id())
}

#[cfg(all(
    target_family = "windows",
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64")
))]
mod local {
    mod windows;
    pub(super) use self::windows::*;
}
#[cfg(all(
    not(target_family = "windows"),
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64")
))]
mod local {
    mod unix;
    pub(super) use self::unix::*;
}
#[cfg(all(
    not(target_family = "unix"),
    not(target_family = "windows"),
    any(
        target_os = "wasi",
        target_os = "emscripten",
        target_arch = "wasm32",
        target_arch = "wasm64"
    ),
))]
mod local {
    mod wasm;
    pub(super) use self::wasm::*;
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::alloc::Allocator;
    use core::fmt::{self, Debug, Display, Formatter, Write};

    use crate::device::Machine;
    use crate::prelude::*;

    impl<A: Allocator> Debug for Machine<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Machine")
                .field("user", &self.user)
                .field("version", &self.version)
                .field("hostname", &self.hostname)
                .field("network", &self.network)
                .field("pid", &self.pid)
                .field("ppid", &self.ppid)
                .field("capabilities", &self.capabilities)
                .field("id", &self.id)
                .field("system", &self.system)
                .field("elevated", &self.elevated)
                .finish()
        }
    }
    impl<A: Allocator> Display for Machine<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_char('[')?;
            Display::fmt(&self.id, f)?;
            f.write_str("] ")?;
            f.write_str(&self.hostname)?;
            f.write_str(" (")?;
            f.write_str(&self.version)?;
            f.write_str(") ")?;
            if self.is_elevated() {
                f.write_char('*')?;
            }
            f.write_str(&self.user)
        }
    }
}
