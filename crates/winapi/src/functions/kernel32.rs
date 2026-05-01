// Copyright (C) 2023 - 2025 iDigitalFlame
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

extern crate core;

use core::convert::Into;
use core::default::Default;
use core::result::Result::{Err, Ok};

use crate::functions::len_to_u32;
use crate::loader::kernel32_or_base;
use crate::structs::{copy_file_ex, CtrlHandlerFunc, Handle, ProcessInfo, SecAttrs, StartInfo, StringLike, WCharLike};
use crate::{Win32Error, Win32Result};

pub fn WriteConsole(h: Handle, buf: &[u8]) -> Win32Result<usize> {
    let mut n = 0u32;
    // NOTE(dij): We could eliminate this call, but that would require setting
    //            up calls to the Csr (Console Server Runtime) service, which
    //            is a total pain in the ass, and requires a lot of work for
    //            something that is really only done while debugging (mostly).
    //            The overhead for loading kernel32/kernelbase for this to write
    //            to stdout/stderr is negligible to all that code.
    let r = syscall!(
        kernel32_or_base().WriteConsoleA,
       (Handle, *const u8, u32, *mut u32, usize) -> u32,
        h,
        buf.as_ptr(),
        len_to_u32(buf.len()),
        &mut n,
        0
    ) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n as usize)
    }
}
#[inline]
pub fn SetConsoleCtrlHandler(handler: CtrlHandlerFunc, add: bool) -> Win32Result<()> {
    let r = syscall!(kernel32_or_base().SetConsoleCtrlHandler, (CtrlHandlerFunc, u32) -> u32, handler, if add { 1 } else { 0 }) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}

#[inline]
pub fn LocalFree<T>(v: *const T) {
    if v.is_null() {
        return;
    }
    syscall!(kernel32_or_base().LocalFree, (*const T) -> (), v)
}

pub fn MoveFileEx<'a>(from: impl Into<WCharLike<'a>>, to: impl Into<WCharLike<'a>>, flags: u32) -> Win32Result<()> {
    let d = to.into();
    if d.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let s = from.into();
    if s.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    if syscall!(kernel32_or_base().MoveFileExW, (*const u16, *const u16, u32) -> u32, s.as_ptr(), d.as_ptr(), flags) == 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(())
    }
}
pub fn CopyFileEx<'a>(from: impl Into<WCharLike<'a>>, to: impl Into<WCharLike<'a>>, flags: u32) -> Win32Result<u64> {
    let d = to.into();
    if d.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let s = from.into();
    if s.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let mut n = 0u64;
    let r = syscall!(
        kernel32_or_base().CopyFileExW,
        (*const u16, *const u16, unsafe extern "system" fn(u64, u64, u64, u64, u32, u32, usize, usize, *mut usize) -> u32, *mut u64, *mut u32, u32) -> u32,
        s.as_ptr(),
        d.as_ptr(),
        copy_file_ex,
        &mut n,
        &mut 0,
        flags
    ) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(n)
    }
}

pub fn CreateProcess<'a, T: Into<WCharLike<'a>>>(name: T, cmd: T, psa: SecAttrs, tsa: SecAttrs, inherit: bool, flags: u32, env: T, dir: T, start: StartInfo<'a>) -> Win32Result<ProcessInfo> {
    let c = cmd.into();
    if c.is_empty() {
        return Err(Win32Error::InvalidArgument);
    }
    let (n, d, e) = (name.into(), dir.into(), env.into());
    // 0x00400 - CREATE_UNICODE_ENVIRONMENT
    // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
    let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
    let mut i = ProcessInfo::default();
    let r = syscall!(
        kernel32_or_base().CreateProcessW,
        (*const u16, *const u16, SecAttrs, SecAttrs, u32, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
        n.as_ptr(),
        c.as_ptr(),
        psa,
        tsa,
        if inherit { 1 } else { 0 },
        f,
        e.as_ptr(),
        d.as_ptr(),
        start.as_ptr(),
        &mut i
    ) == 0;
    if r {
        Err(Win32Error::last_error())
    } else {
        Ok(i)
    }
}
