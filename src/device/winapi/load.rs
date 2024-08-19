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
#![cfg(target_family = "windows")]

use core::mem::size_of;
use core::ptr;

use crate::device::winapi::ntdll::NtCreateFile;
use crate::device::winapi::{self, FileStandardInformation, Handle, Win32Result};
use crate::prelude::*;

#[repr(C)]
pub(super) struct ImageNtHeader {
    pub(super) signature: u32,
    pub(super) file:      ImageFileHeader,
}
#[repr(C)]
pub(super) struct ImageDosHeader {
    pub(super) magic: u16,
    pad1:             [u8; 56],
    pub(super) pos:   u32,
}

pub fn test_load_dll(file: impl AsRef<str>) -> Win32Result<()> {
    let f = winapi::NtCreateFile(
        file,
        Handle::INVALID,
        0x80000000 | 0x100080,
        None,
        0x80u32,
        0x1u32,
        0x1u32,
        0,
    )?;
    // 0x5 - FileStandardInformation
    let mut i = FileStandardInformation::default();
    winapi::NtQueryInformationFile(f, 0x5, &i, 0x20)?;


    winapi::NtAllocateVirtualMemory(h, base, size, flags, access)


    Ok(())
}
