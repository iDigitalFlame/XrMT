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

use crate::data::{Readable, Reader, Writable, Writer};
use crate::device::{self, Network, ID};
use crate::process;
use crate::util::stx::io;
use crate::util::stx::prelude::*;

#[cfg(unix)]
mod local {
    mod unix;
    pub(super) use self::unix::*;
}
#[cfg(windows)]
mod local {
    mod windows;
    pub(super) use self::windows::*;
}

mod arch;
mod capabilities;
mod os;

#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct Machine {
    pub user:         String,
    pub version:      String,
    pub hostname:     String,
    pub network:      Network,
    pub pid:          u32,
    pub ppid:         u32,
    pub capabilities: u32,
    pub id:           ID,
    pub system:       u8,
    pub elevated:     u8,
}

impl Machine {
    #[inline]
    pub fn local() -> io::Result<Machine> {
        Ok(Machine {
            network:      Network::local()?,
            hostname:     device::hostname()?,
            id:           system_id(),
            pid:          process::id(),
            ppid:         process::parent_id(),
            user:         local::username(),
            system:       local::system(),
            version:      local::version(),
            elevated:     local::elevated(),
            capabilities: capabilities::ENABLED,
        })
    }

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
    #[inline]
    pub fn refresh(&mut self) -> io::Result<()> {
        self.network.refresh()?;
        self.hostname = device::hostname()?;
        self.pid = process::id();
        self.ppid = process::parent_id();
        self.user = local::username();
        self.elevated = local::elevated();
        Ok(())
    }
}

impl Clone for Machine {
    #[inline]
    fn clone(&self) -> Machine {
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
impl Default for Machine {
    #[inline]
    fn default() -> Machine {
        Machine {
            id:           ID::default(),
            pid:          0,
            ppid:         0,
            user:         String::new(),
            system:       0,
            version:      String::new(),
            network:      Network::new(),
            hostname:     String::new(),
            elevated:     0,
            capabilities: 0,
        }
    }
}
impl Writable for Machine {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        self.id.write_stream(w)?;
        w.write_u8(self.system)?;
        w.write_u32(self.pid)?;
        w.write_u32(self.ppid)?;
        w.write_str(&self.user)?;
        w.write_str(&self.version)?;
        w.write_str(&self.hostname)?;
        w.write_u8(self.elevated)?;
        w.write_u32(self.capabilities)?;
        self.network.write_stream(w)
    }
}
impl Readable for Machine {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        self.id.read_stream(r)?;
        r.read_into_u8(&mut self.system)?;
        r.read_into_u32(&mut self.pid)?;
        r.read_into_u32(&mut self.ppid)?;
        r.read_to_string(&mut self.user)?;
        r.read_to_string(&mut self.version)?;
        r.read_to_string(&mut self.hostname)?;
        r.read_into_u8(&mut self.elevated)?;
        r.read_into_u32(&mut self.capabilities)?;
        self.network.read_stream(r)
    }
}

#[inline]
pub(crate) fn system_id() -> ID {
    ID::from(local::system_id())
}
