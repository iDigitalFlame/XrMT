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

use crate::util::stx::prelude::*;

pub const CURRENT: OS = if cfg!(target_os = "ios") {
    OS::Ios
} else if cfg!(target_os = "macos") {
    OS::Mac
} else if cfg!(target_os = "linux") {
    OS::Linux
} else if cfg!(target_os = "android") {
    OS::Android
} else if cfg!(target_family = "unix") {
    OS::Unix
} else if cfg!(target_family = "windows") {
    OS::Windows
} else {
    OS::Unsupported
};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum OS {
    Windows     = 0x0,
    Linux       = 0x1,
    Unix        = 0x2,
    Mac         = 0x3,
    Ios         = 0x4,
    Android     = 0x5,
    Plan9       = 0x6,
    Unsupported = 0x7,
}

impl From<u8> for OS {
    #[inline]
    fn from(v: u8) -> OS {
        match v {
            0x0 => OS::Windows,
            0x1 => OS::Linux,
            0x2 => OS::Unix,
            0x3 => OS::Mac,
            0x4 => OS::Ios,
            0x5 => OS::Android,
            0x6 => OS::Plan9,
            _ => OS::Unsupported,
        }
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use super::OS;
    use crate::util::stx::prelude::*;

    impl Debug for OS {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for OS {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match *self {
                OS::Windows => f.write_str("Windows"),
                OS::Linux => f.write_str("Linux"),
                OS::Unix => f.write_str("Unix/BSD"),
                OS::Mac => f.write_str("MacOS"),
                OS::Ios => f.write_str("iOS"),
                OS::Android => f.write_str("Android"),
                OS::Plan9 => f.write_str("Plan9"),
                _ => f.write_str("Unknown"),
            }
        }
    }
}
