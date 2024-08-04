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

use core::{cmp, ptr};

use crate::data::blob::Blob;
use crate::data::str::MaybeString;
use crate::device::winapi::loader::kernel32;
use crate::device::winapi::{self, DecodeUtf16, Handle, OsVersionInfo, ProcessInfo, SecAttrs, SecurityAttributes, StartInfo, WChars, Win32Error, Win32Result};
use crate::prelude::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
#[cfg(feature = "snap")]
pub use self::snap::*;

const TEMP_1: [u16; 3] = [b'T' as u16, b'M' as u16, b'P' as u16];
const TEMP_2: [u16; 4] = [b'T' as u16, b'E' as u16, b'M' as u16, b'P' as u16];
const TEMP_3: [u16; 11] = [
    b'U' as u16,
    b'S' as u16,
    b'E' as u16,
    b'R' as u16,
    b'P' as u16,
    b'R' as u16,
    b'O' as u16,
    b'F' as u16,
    b'I' as u16,
    b'L' as u16,
    b'E' as u16,
];

pub fn GetTempPath() -> String {
    let mut d = winapi::GetEnvironment()
        .iter()
        .find(|v| v.is_key(&TEMP_1) || v.is_key(&TEMP_2))
        .and_then(|v| v.value_as_string())
        .unwrap_or_else(|| {
            winapi::GetEnvironment()
                .iter()
                .find(|v| v.is_key(&TEMP_3))
                .and_then(|v| v.value_as_string())
                .unwrap_or_else(|| {
                    winapi::current_token(0x8)
                        .and_then(winapi::GetUserProfileDirectory)
                        .unwrap_or_else(|_| winapi::system_root().to_string())
                })
        });
    // ^
    // 1. Try %TEMP% and %TEMP%
    // 2. Try %USERPROFILE%
    // 3. Use 'GetUserProfileDirectory()'
    // 4. Use the Windows directory.
    if !d.as_bytes().last().map_or(true, |v| *v == b'\\') {
        d.push('\\' as char);
    }
    d
}
#[inline]
pub fn LocalFree<T>(v: *const T) {
    if v.is_null() {
        return;
    }
    winapi::init_kernel32();
    unsafe { winapi::syscall!(*kernel32::LocalFree, extern "stdcall" fn(*const T), v) }
}
pub fn GetComputerName() -> Win32Result<String> {
    winapi::init_kernel32();
    let mut s = 64u32;
    let mut b: Blob<u16, 256> = Blob::new();
    let func = unsafe {
        winapi::make_syscall!(
            *kernel32::GetComputerNameEx,
            extern "stdcall" fn(u32, *mut u16, *mut u32) -> u32
        )
    };
    loop {
        b.resize(s as usize);
        // 0x5 - ComputerNamePhysicalDnsHostname
        if func(0x5, b.as_mut_ptr(), &mut s) == 1 {
            return Ok((&b[0..cmp::min(s as usize, b.len() as usize)]).decode_utf16());
        }
        let e = winapi::GetLastError();
        if e != 0xEA || s < b.len() as u32 {
            // 0xEA - ERROR_MORE_DATA
            return Err(Win32Error::from_code(e));
        }
    }
}
pub fn GetVersion() -> Win32Result<OsVersionInfo> {
    let mut i = OsVersionInfo::default();
    let r = unsafe {
        winapi::syscall!(
            *kernel32::GetVersionExW,
            extern "stdcall" fn(*mut OsVersionInfo) -> u32,
            &mut i
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(i)
    }
}
pub fn WriteConsole(h: Handle, buf: &[u8]) -> Win32Result<usize> {
    winapi::init_kernel32();
    let mut n = 0u32;
    let r = unsafe {
        winapi::syscall!(
            *kernel32::WriteConsoleA,
            extern "stdcall" fn(usize, *const u8, u32, *mut u32, usize) -> u32,
            h.0,
            buf.as_ptr(),
            cmp::min(buf.len(), 0xFFFFFFFF) as u32,
            &mut n,
            0
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(n as usize)
    }
}

pub fn MoveFileEx(from: impl AsRef<str>, to: impl AsRef<str>, flags: u32) -> Win32Result<()> {
    winapi::init_kernel32();
    let r = unsafe {
        let d: WChars = to.as_ref().into();
        let s: WChars = from.as_ref().into();
        winapi::syscall!(
            *kernel32::MoveFileEx,
            extern "stdcall" fn(*const u16, *const u16, u32) -> u32,
            s.as_ptr(),
            d.as_ptr(),
            flags
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn CopyFileEx(from: impl AsRef<str>, to: impl AsRef<str>, flags: u32) -> Win32Result<u64> {
    winapi::init_kernel32();
    let mut n = 0u64;
    let r = unsafe {
        let d: WChars = to.as_ref().into();
        let s: WChars = from.as_ref().into();
        winapi::syscall!(
            *kernel32::CopyFileEx,
            extern "stdcall" fn(*const u16, *const u16, unsafe extern "stdcall" fn(u64, u64, u64, u64, u32, u32, usize, usize, *mut usize) -> u32, *mut u64, *mut u32, u32) -> u32,
            s.as_ptr(),
            d.as_ptr(),
            winapi::_copy_file_ex,
            &mut n,
            &mut 0,
            flags
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(n)
    }
}

pub fn CreateProcess<T: AsRef<str>, E: AsRef<str>, M: MaybeString>(name: T, cmd: T, psa: SecAttrs, tsa: SecAttrs, inherit: bool, flags: u32, env_split: bool, env: &[E], dir: M, start: StartInfo) -> Win32Result<ProcessInfo> {
    winapi::init_kernel32();
    let mut i = ProcessInfo::default();
    let r = unsafe {
        let mut c: WChars = cmd.as_ref().into();
        let n: WChars = name.as_ref().into();
        let d: WChars = dir.into_string().into();
        let e = winapi::build_env(env_split, env);
        // 0x00400 - CREATE_UNICODE_ENVIRONMENT
        // 0x80000 - EXTENDED_STARTUPINFO_PRESENT
        let f = flags | 0x400 | if start.is_extended() { 0x80000 } else { 0 };
        winapi::syscall!(
            *kernel32::CreateProcess,
            extern "stdcall" fn(*const u16, *mut u16, *const SecurityAttributes, *const SecurityAttributes, u32, u32, *const u16, *const u16, *const usize, *mut ProcessInfo) -> u32,
            n.as_ptr(),
            c.as_mut_ptr(),
            psa.map_or_else(ptr::null, |v| v),
            tsa.map_or_else(ptr::null, |v| v),
            if inherit { 1 } else { 0 },
            f,
            e.as_ptr(),
            d.as_null_or_ptr(),
            start.as_ptr(),
            &mut i
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(i)
    }
}

#[cfg(feature = "snap")]
mod snap {
    use crate::device::winapi::loader::kernel32;
    use crate::device::winapi::{self, AsHandle, Handle, OwnedHandle, ProcessEntry32, ThreadEntry32, Win32Result};
    use crate::prelude::*;

    #[inline]
    pub fn Thread32Next(h: impl AsHandle, e: &mut ThreadEntry32) -> Win32Result<()> {
        winapi::init_kernel32();
        let r = unsafe {
            winapi::syscall!(
                *kernel32::Thread32Next,
                extern "stdcall" fn(Handle, *mut ThreadEntry32) -> u32,
                h.as_handle(),
                e
            ) == 0
        };
        if r {
            Err(winapi::last_error())
        } else {
            Ok(())
        }
    }
    #[inline]
    pub fn Thread32First(h: impl AsHandle, e: &mut ThreadEntry32) -> Win32Result<()> {
        winapi::init_kernel32();
        let r = unsafe {
            winapi::syscall!(
                *kernel32::Thread32First,
                extern "stdcall" fn(Handle, *mut ThreadEntry32) -> u32,
                h.as_handle(),
                e
            ) == 0
        };
        if r {
            Err(winapi::last_error())
        } else {
            Ok(())
        }
    }
    #[inline]
    pub fn Process32Next(h: impl AsHandle, e: &mut ProcessEntry32) -> Win32Result<()> {
        winapi::init_kernel32();
        let r = unsafe {
            winapi::syscall!(
                *kernel32::Process32Next,
                extern "stdcall" fn(Handle, *mut ProcessEntry32) -> u32,
                h.as_handle(),
                e
            ) == 0
        };
        if r {
            Err(winapi::last_error())
        } else {
            Ok(())
        }
    }
    #[inline]
    pub fn Process32First(h: impl AsHandle, e: &mut ProcessEntry32) -> Win32Result<()> {
        winapi::init_kernel32();
        let r = unsafe {
            winapi::syscall!(
                *kernel32::Process32First,
                extern "stdcall" fn(Handle, *mut ProcessEntry32) -> u32,
                h.as_handle(),
                e
            ) == 0
        };
        if r {
            Err(winapi::last_error())
        } else {
            Ok(())
        }
    }
    #[inline]
    pub fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Win32Result<OwnedHandle> {
        winapi::init_kernel32();
        let r = unsafe {
            winapi::syscall!(
                *kernel32::CreateToolhelp32Snapshot,
                extern "stdcall" fn(u32, u32) -> Handle,
                flags,
                pid
            )
        };
        if r.is_invalid() {
            Err(winapi::last_error())
        } else {
            Ok(r.into())
        }
    }
}
