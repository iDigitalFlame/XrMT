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

use core::ops::Deref;
use core::slice;

use crate::device::winapi::{self, AsHandle, Win32Result};
use crate::util::stx::prelude::*;

#[repr(transparent)]
pub struct Region(pub usize);

impl Region {
    #[inline]
    pub const fn empty() -> Region {
        Region(0)
    }

    #[inline]
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn free(self, proc: impl AsHandle) -> Win32Result<()> {
        if self.is_invalid() {
            return Ok(());
        }
        // 0x8000 - MEM_RELEASE
        winapi::NtFreeVirtualMemory(proc, self, 0, 0x8000)
    }

    #[inline]
    pub unsafe fn as_slice(&self, size: usize) -> &[u8] {
        slice::from_raw_parts(self.0 as *const u8, size)
    }
}

impl Eq for Region {}
impl Deref for Region {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        &self.0
    }
}
impl Default for Region {
    #[inline]
    fn default() -> Region {
        Region(0)
    }
}
impl PartialEq for Region {
    #[inline]
    fn eq(&self, other: &Region) -> bool {
        self.0 == other.0
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::Region;
    use crate::util::stx::prelude::*;

    impl Debug for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "MemoryRegion: 0x{:X}", self.0)
        }
    }
    impl LowerHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
    impl UpperHex for Region {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
