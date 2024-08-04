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

use crate::device::winapi::{self, gdi32, BitmapInfo, Handle, Win32Result};
use crate::prelude::*;

pub fn DeleteDC(h: Handle) -> Win32Result<()> {
    winapi::init_gdi32();
    let r = unsafe { winapi::syscall!(*gdi32::DeleteDC, extern "stdcall" fn(Handle) -> u32, h) == 0 };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn DeleteObject(h: Handle) -> Win32Result<()> {
    winapi::init_gdi32();
    let r = unsafe { winapi::syscall!(*gdi32::DeleteObject, extern "stdcall" fn(Handle) -> u32, h) == 0 };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn CreateCompatibleDC(h: Handle) -> Win32Result<Handle> {
    winapi::init_gdi32();
    let r = unsafe {
        winapi::syscall!(
            *gdi32::CreateCompatibleDC,
            extern "stdcall" fn(Handle) -> Handle,
            h
        )
    };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn SelectObject(dc: Handle, h: Handle) -> Win32Result<Handle> {
    winapi::init_gdi32();
    let r = unsafe {
        winapi::syscall!(
            *gdi32::SelectObject,
            extern "stdcall" fn(Handle, Handle) -> Handle,
            dc,
            h
        )
    };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn CreateCompatibleBitmap(h: Handle, width: u32, height: u32) -> Win32Result<Handle> {
    winapi::init_gdi32();
    let r = unsafe {
        winapi::syscall!(
            *gdi32::CreateCompatibleBitmap,
            extern "stdcall" fn(Handle, u32, u32) -> Handle,
            h,
            width,
            height
        )
    };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn BitBlt(dc: Handle, x: u32, y: u32, width: u32, height: u32, src: Handle, x1: u32, y1: u32, rop: u32) -> Win32Result<()> {
    winapi::init_gdi32();
    let r = unsafe {
        winapi::syscall!(
            *gdi32::BitBlt,
            extern "stdcall" fn(Handle, u32, u32, u32, u32, Handle, u32, u32, u32) -> u32,
            dc,
            x,
            y,
            width,
            height,
            src,
            x1,
            y1,
            rop
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn GetDIBits(dc: Handle, bitmap: Handle, start: u32, lines: u32, buf: *mut u8, info: &mut BitmapInfo, usage: u32) -> Win32Result<u32> {
    winapi::init_gdi32();
    let r = unsafe {
        winapi::syscall!(
            *gdi32::GetDIBits,
            extern "stdcall" fn(Handle, Handle, u32, u32, *mut u8, *mut BitmapInfo, u32) -> u32,
            dc,
            bitmap,
            start,
            lines,
            buf,
            info,
            usage
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
