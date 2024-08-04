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

use crate::prelude::*;

pub const CURRENT: Architecture = _arch();

core::cfg_match! {
    cfg(target_arch = "x86_64") => { const fn _arch() -> Architecture { Architecture::X64 } }
    cfg(any(target_arch = "x86", target_arch = "s390x")) => { const fn _arch() -> Architecture { Architecture::X86 } }
    cfg(target_arch = "arm") => { const fn _arch() -> Architecture { Architecture::Arm } }
    cfg(any(target_arch = "wasm32", target_arch = "wasm64")) => { const fn _arch() -> Architecture { Architecture::Wasm } }
    cfg(any(target_arch = "aarch64", target_arch = "arm64ec")) => { const fn _arch() -> Architecture { Architecture::Arm64 } }
    cfg(target_arch = "loongarch64") => { const fn _arch() -> Architecture { Architecture::Loong64 } }
    cfg(any(target_arch = "mips", target_arch = "mips32r6", target_arch = "mips64", target_arch = "mips64r6")) => { const fn _arch() -> Architecture { Architecture::Mips } }
    cfg(any(target_arch = "riscv32", target_arch = "riscv64")) => { const fn _arch() -> Architecture { Architecture::Risc } }
    cfg(any(target_arch = "powerpc", target_arch = "powerpc64")) => { const fn _arch() -> Architecture { Architecture::PowerPc } }
    cfg(any(target_arch = "sparc", target_arch = "sparc64")) => { const fn _arch() -> Architecture { Architecture::Sparc } }
    _ => { const fn _arch() -> Architecture { Architecture::Unknown } }
}

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
    Sparc      = 0xB, // TODO(dij): NEW!! New proc arch.
    Emulated   = 0xE, // TODO(dij): NEW!! Emulated MacOS arch.
    Unknown    = 0xF,
}

impl Eq for Architecture {}
impl Copy for Architecture {}
impl Clone for Architecture {
    #[inline]
    fn clone(&self) -> Architecture {
        *self
    }
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
            0xB => Architecture::Sparc,
            0xE => Architecture::Emulated,
            _ => Architecture::Unknown,
        }
    }
}
impl PartialEq for Architecture {
    #[inline]
    fn eq(&self, other: &Architecture) -> bool {
        *self as u8 == *other as u8
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::device::machine::arch::Architecture;
    use crate::prelude::*;

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
                Architecture::Sparc => f.write_str("SPARC"),
                Architecture::Emulated => f.write_str("Emulated"),
                _ => f.write_str("Unknown"),
            }
        }
    }
}
