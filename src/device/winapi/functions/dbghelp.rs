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
#![allow(non_snake_case)]

use core::ptr;

use crate::device::winapi::{self, dbghelp, AsHandle, Handle, MinDumpOutput, MiniDumpCallback, MiniDumpData, Win32Error, Win32Result};
use crate::io::Write;
use crate::prelude::*;

pub fn MiniDumpWriteDump(h: impl AsHandle, pid: u32, flags: u32, out: MinDumpOutput) -> Win32Result<usize> {
    winapi::init_dbghelp();
    match out {
        MinDumpOutput::Handle(d) => dump_to_handle(h.as_handle(), d, pid, flags),
        MinDumpOutput::File(f) => dump_to_handle(h.as_handle(), f.as_handle(), pid, flags),
        MinDumpOutput::Writer(_) if !winapi::is_min_windows_vista() => Err(Win32Error::InvalidArgument),
        MinDumpOutput::Writer(w) => dump_to_writer(h.as_handle(), w, pid, flags),
    }
}

fn dump_to_handle(h: Handle, f: Handle, pid: u32, flags: u32) -> Win32Result<usize> {
    let r = unsafe {
        winapi::syscall!(
            *dbghelp::MiniDumpWriteDump,
            extern "stdcall" fn(Handle, u32, Handle, u32, *const u8, *const u8, *mut u8) -> u32,
            h,
            pid,
            f,
            flags,
            ptr::null(),
            ptr::null(),
            ptr::null_mut()
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(0)
    }
}
fn dump_to_writer(h: Handle, w: &mut dyn Write, pid: u32, flags: u32) -> Win32Result<usize> {
    let mut d = Box::new(MiniDumpData::new()?);
    let r = {
        let mut a = MiniDumpCallback::new(&mut d);
        unsafe {
            winapi::syscall!(
                *dbghelp::MiniDumpWriteDump,
                extern "stdcall" fn(Handle, u32, Handle, u32, *const u8, *const u8, *mut MiniDumpCallback) -> u32,
                h.as_handle(),
                pid,
                Handle::INVALID,
                flags,
                ptr::null(),
                ptr::null(),
                &mut a
            )
        }
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        d.finish(w)
    }
}
