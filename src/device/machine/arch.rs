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

pub const CURRENT: Architecture = if cfg!(target_arch = "x86_64") {
    Architecture::X64
} else if cfg!(any(target_arch = "x86", target_arch = "s390x")) {
    Architecture::X86
} else if cfg!(target_arch = "arm") {
    Architecture::Arm
} else if cfg!(target_arch = "wasm") {
    Architecture::Wasm
} else if cfg!(target_arch = "aarch64") {
    Architecture::Arm64
} else if cfg!(target_arch = "loong64") {
    Architecture::Loong64
} else if cfg!(any(target_arch = "mips", target_arch = "mips64")) {
    Architecture::Mips
} else if cfg!(any(target_arch = "riscv", target_arch = "riscv64")) {
    Architecture::Risc
} else if cfg!(any(target_arch = "powerpc", target_arch = "powerpc64")) {
    Architecture::PowerPc
} else {
    Architecture::Unknown
};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Architecture {
    X64        = 0x0,
    X86        = 0x1,
    Arm        = 0x2,
    PowerPc    = 0x3,
    Mips       = 0x4,
    Risc       = 0x5,
    Arm64      = 0x6,
    Wasm       = 0x7,
    Loong64    = 0x8,
    X86OnX64   = 0x9,
    ArmOnArm64 = 0xA,
    Unknown    = 0xF,
}

impl From<u8> for Architecture {
    #[inline]
    fn from(v: u8) -> Architecture {
        match v {
            0x0 => Architecture::X64,
            0x1 => Architecture::X86,
            0x2 => Architecture::Arm,
            0x3 => Architecture::PowerPc,
            0x4 => Architecture::Mips,
            0x5 => Architecture::Risc,
            0x6 => Architecture::Arm64,
            0x7 => Architecture::Wasm,
            0x8 => Architecture::Loong64,
            0x9 => Architecture::X86OnX64,
            0xA => Architecture::ArmOnArm64,
            _ => Architecture::Unknown,
        }
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use super::Architecture;
    use crate::util::stx::prelude::*;

    impl Debug for Architecture {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Architecture {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                Architecture::X64 => f.write_str("64bit"),
                Architecture::X86 => f.write_str("32bit"),
                Architecture::Arm => f.write_str("ARM"),
                Architecture::Wasm => f.write_str("WASM"),
                Architecture::Risc => f.write_str("RiscV"),
                Architecture::Mips => f.write_str("MIPS"),
                Architecture::PowerPc => f.write_str("PowerPC"),
                Architecture::X86OnX64 => f.write_str("32bit [64bit]"),
                Architecture::ArmOnArm64 => f.write_str("ARM [ARM64]"),
                _ => f.write_str("Unknown"),
            }
        }
    }
}
