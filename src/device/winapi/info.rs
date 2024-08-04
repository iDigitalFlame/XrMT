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

use crate::data::blob::Blob;
use crate::device::winapi::{self, DiskGeometry, Handle, SystemBasicInfo, Win32Result};
use crate::prelude::*;

#[inline]
pub fn total_memory() -> Win32Result<u32> {
    let mut i = SystemBasicInfo::default();
    // 0x0 - SystemBasicInformation
    winapi::NtQuerySystemInformation(0, &mut i, size_of::<SystemBasicInfo>() as u32)?;
    Ok((((i.page_size as u64) * (i.number_physical_pages as u64)) / 0x100000) as u32 + 1)
}
#[inline]
pub fn local_user() -> Win32Result<String> {
    // 0x8 - TOKEN_QUERY
    winapi::token_username(winapi::current_token(0x8)?)
}
#[inline]
pub fn current_directory() -> Blob<u8, 256> {
    winapi::GetCurrentProcessPEB()
        .process_params()
        .current_directory
        .dos_path
        .to_u8_blob()
}
#[inline]
pub fn code_integrity_status() -> Win32Result<u32> {
    let mut i = [8u32, 0u32];
    // 0x67 - SystemCodeIntegrityInformation
    winapi::NtQuerySystemInformation(0x67, &mut i, 8).map(|_| i[1])
}
#[inline]
pub fn disk_size(name: impl AsRef<str>) -> Win32Result<u64> {
    let f = winapi::NtCreateFile(name, Handle::INVALID, 0x100080, None, 0, 0x1, 0, 0x40)?;
    let mut g = DiskGeometry::default();
    // 0x700A0 - IOCTL_DISK_GET_DRIVE_GEOMETRY_EX
    winapi::NtDeviceIoControlFile(
        f,
        0x700A0,
        None,
        ptr::null::<usize>(),
        0,
        &mut g,
        0x20 + winapi::PTR_SIZE as u32,
    )
    .map(|_| g.size)
}
