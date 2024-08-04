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

use core::mem::size_of;
use core::ptr;

use crate::data::str::MaybeString;
use crate::device::winapi::functions::{HighContrast, Input};
use crate::device::winapi::{self, user32, Handle, WChars, Win32Error, Win32Result, Window, WindowState, _enum_window};
use crate::ignore_error;
use crate::prelude::*;

pub fn close_window(h: usize) -> Win32Result<()> {
    if h > 0 {
        return CloseWindow(h);
    }
    winapi::init_user32();
    let func = unsafe {
        winapi::make_syscall!(
            *user32::SendNotifyMessage,
            extern "stdcall" fn(usize, u32, *const usize, *const usize) -> u32
        )
    };
    for w in top_level_windows()? {
        // 0x2 - WM_DESTROY
        if func(w.handle, 0x2, ptr::null(), ptr::null()) == 0 {
            return Err(winapi::last_error());
        }
    }
    Ok(())
}
pub fn top_level_windows() -> Win32Result<Vec<Window>> {
    winapi::init_user32();
    let mut o = Vec::new();
    let r = unsafe {
        winapi::syscall!(
            *user32::EnumWindows,
            extern "stdcall" fn(unsafe extern "stdcall" fn(usize, *mut Vec<Window>) -> u32, *mut Vec<Window>) -> u32,
            _enum_window,
            &mut o
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(o)
    }
}
#[inline]
pub fn swap_mouse_buttons(swap: bool) -> Win32Result<()> {
    // 0x21 - SPI_SETMOUSEBUTTONSWAP
    SystemParametersInfo(
        0x21,
        if swap { 1 } else { 0 },
        ptr::null_mut::<usize>(),
        0x3,
    )
}
pub fn set_high_contrast(enable: bool) -> Win32Result<()> {
    let mut c = HighContrast::default();
    c.flags = if enable { 1 } else { 0 };
    // 0x43 - SPI_SETHIGHCONTRAST
    SystemParametersInfo(0x43, 0, &mut c, 0x3)
}
#[inline]
pub fn set_wallpaper(path: impl AsRef<str>) -> Win32Result<()> {
    let mut f: WChars = path.as_ref().into();
    // 0x14 - SPI_SETDESKWALLPAPER
    SystemParametersInfo(0x14, 0x1, f.as_mut_ptr(), 0x3)
}
pub fn enable_window(h: usize, enable: bool) -> Win32Result<()> {
    if h > 0 {
        return EnableWindow(h, enable).map(|_| ());
    }
    winapi::init_user32();
    let func = unsafe {
        winapi::make_syscall!(
            *user32::EnableWindow,
            extern "stdcall" fn(usize, u32) -> u32
        )
    };
    let e = if enable { 1 } else { 0 };
    for w in top_level_windows()? {
        ignore_error!(func(w.handle, e));
    }
    Ok(())
}
pub fn show_window(h: usize, s: WindowState) -> Win32Result<()> {
    if h > 0 {
        return ShowWindow(h, s).map(|_| ());
    }
    winapi::init_user32();
    let func = unsafe { winapi::make_syscall!(*user32::ShowWindow, extern "stdcall" fn(usize, u32) -> u32) };
    let v = s as u32;
    for w in top_level_windows()? {
        // 0x2 - WM_DESTROY
        ignore_error!(func(w.handle, v));
    }
    Ok(())
}
#[inline]
pub fn set_window_transparency(h: usize, t: u8) -> Win32Result<()> {
    if h > 0 {
        return SetWindowTransparency(h, t);
    }
    // This is the only one we don't compact and call it directly as it's a
    // more compex function.
    for w in top_level_windows()? {
        SetWindowTransparency(w.handle, t)?;
    }
    Ok(())
}

pub fn GetDC(h: Handle) -> Win32Result<Handle> {
    winapi::init_user32();
    let r = unsafe { winapi::syscall!(*user32::GetDC, extern "stdcall" fn(Handle) -> Handle, h) };
    if r.is_invalid() {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}
pub fn CloseWindow(h: usize) -> Win32Result<()> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::SendNotifyMessage,
            extern "stdcall" fn(usize, u32, usize, usize) -> u32,
            h,
            0x2, // 0x2 - WM_DESTROY
            0,
            0
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn GetDesktopWindow() -> Win32Result<Handle> {
    winapi::init_user32();
    let h = unsafe { winapi::syscall!(*user32::GetDesktopWindow, extern "stdcall" fn() -> Handle,) };
    if h.is_invalid() {
        Err(Win32Error::InvalidHandle)
    } else {
        Ok(h)
    }
}
pub fn BlockInput(block: bool) -> Win32Result<()> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::BlockInput,
            extern "stdcall" fn(u32) -> u32,
            if block { 1 } else { 0 }
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn SetFocus(h: usize) -> Win32Result<Option<usize>> {
    winapi::init_user32();
    let h = unsafe { winapi::syscall!(*user32::SetFocus, extern "stdcall" fn(usize) -> usize, h) };
    let e = winapi::GetLastError();
    // 0x578 - INVALID_WINDOW_HANDLE
    // ^ This is the only error we care about, this function does NOT clear
    // the last error and sometimes it gets weird. This is the only error
    // it will technically throw.
    if e == 0x578 {
        Err(Win32Error::from_code(e))
    } else {
        Ok(if h == 0 { None } else { Some(h) })
    }
}
pub fn SetForegroundWindow(h: usize) -> Win32Result<()> {
    ignore_error!(SetFocus(h));
    // No need to call init, we already did that above.
    let r = unsafe {
        winapi::syscall!(
            *user32::SetForegroundWindow,
            extern "stdcall" fn(usize) -> u32,
            h
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn ReleaseDC(h: Handle, dc: Handle) -> Win32Result<()> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::ReleaseDC,
            extern "stdcall" fn(Handle, Handle) -> u32,
            h,
            dc
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn ShowWindow(h: usize, s: WindowState) -> Win32Result<bool> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::ShowWindow,
            extern "stdcall" fn(usize, u32) -> u32,
            h,
            s as u32
        ) == 0
    };
    let e = winapi::GetLastError();
    // 0x578 - INVALID_WINDOW_HANDLE
    // ^ This is the only error we care about, this function does NOT clear
    // the last error and sometimes it gets weird. This is the only error
    // it will technically throw.
    if e == 0x578 {
        Err(Win32Error::from_code(e))
    } else {
        Ok(r)
    }
}
pub fn EnableWindow(h: usize, enable: bool) -> Win32Result<bool> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::EnableWindow,
            extern "stdcall" fn(usize, u32) -> u32,
            h,
            if enable { 1 } else { 0 }
        ) == 0
    };
    let e = winapi::GetLastError();
    // 0x578 - INVALID_WINDOW_HANDLE
    // ^ This is the only error we care about, this function does NOT clear
    // the last error and sometimes it gets weird. This is the only error
    // it will technically throw.
    if e == 0x578 {
        Err(Win32Error::from_code(e))
    } else {
        Ok(r)
    }
}
pub fn SetWindowTransparency(h: usize, t: u8) -> Win32Result<()> {
    winapi::init_user32();
    // Set the window attributes to have the "Layered" attribute first.
    let v = unsafe {
        winapi::syscall!(
            *user32::GetWindowLongW,
            extern "stdcall" fn(usize, i32) -> u32,
            h,
            -20
        )
    };
    // 'v' might be zero, if it errors oh well lol.
    // 0x80000 - WS_EX_LAYERED
    unsafe {
        winapi::syscall!(
            *user32::SetWindowLongW,
            extern "stdcall" fn(usize, i32, u32) -> u32,
            h,
            -20,
            v | 0x80000
        )
    };
    // We don't check the error in Go either, so no need to do it here.
    // 0x3 - LWA_ALPHA | LWA_COLORKEY
    let r = unsafe {
        winapi::syscall!(
            *user32::SetLayeredWindowAttributes,
            extern "stdcall" fn(usize, u32, u8, u32) -> u32,
            h,
            0,
            t,
            0x3
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
#[inline]
pub fn SendInput(h: usize, text: impl AsRef<str>) -> Win32Result<()> {
    winapi::init_user32();
    if h > 0 {
        ignore_error!(SetForegroundWindow(h));
    }
    let s = text.as_ref().as_bytes();
    if s.is_empty() {
        return Ok(());
    }
    send_text(s)
}
pub fn SetWindowPos(h: usize, x: i32, y: i32, width: i32, height: i32) -> Win32Result<()> {
    winapi::init_user32();
    // 0x14 - SWP_NOZORDER | SWP_NOACTIVATE
    // 0x01 - SWP_NOSIZE
    // 0x02 - SWP_NOMOVE
    let f = 0x14u32 | if width == -1 && height == -1 { 0x1 } else { 0 } | if x == -1 && y == -1 { 0x2 } else { 0 };
    let r = unsafe {
        winapi::syscall!(
            *user32::SetWindowPos,
            extern "stdcall" fn(usize, usize, i32, i32, i32, i32, u32) -> u32,
            h,
            0,
            x,
            y,
            width,
            height,
            f
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn SystemParametersInfo<T>(action: u32, param: u32, buf: *mut T, signal_change: u32) -> Win32Result<()> {
    winapi::init_user32();
    let r = unsafe {
        winapi::syscall!(
            *user32::SystemParametersInfo,
            extern "stdcall" fn(u32, u32, *mut T, u32) -> u32,
            action,
            param,
            buf,
            signal_change
        ) == 0
    };
    if r {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn MessageBox(h: usize, text: impl MaybeString, title: impl MaybeString, flags: u32) -> Win32Result<u32> {
    winapi::init_user32();
    let d: WChars = text.into_string().into();
    let t: WChars = title.into_string().into();
    // Check if h == -1. If it's -1, we choose the desktop as the parent.
    let p = if h == winapi::CURRENT_PROCESS.0 {
        top_level_windows()
            .map(|v| v.iter().find(|i| i.flags & 0x80 != 0).map(|i| i.handle))
            .ok()
            .flatten()
            .unwrap_or(h)
    } else {
        h
    };
    let r = unsafe {
        winapi::syscall!(
            *user32::MessageBox,
            extern "stdcall" fn(usize, *const u16, *const u16, u32) -> u32,
            p,
            d.as_ptr(),
            t.as_ptr(),
            flags
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(r)
    }
}

fn key_code(k: u8) -> (u16, bool) {
    if k > 0x2F && k < 0x38 {
        return (k as u16, false);
    }
    if k > 0x40 && k < 0x5B {
        return (k as u16, true);
    }
    if k > 0x60 && k < 0x7B {
        return (k as u16 - 0x20, false);
    }
    match k {
        b'\r' | b'\n' => (0x0D, false),
        b'\t' => (0x09, false),
        b'-' => (0xBD, false),
        b'=' => (0xBB, false),
        b';' => (0xBA, false),
        b'[' => (0xDB, false),
        b']' => (0xDD, false),
        b'\\' => (0xDC, false),
        b',' => (0xBC, false),
        b'.' => (0xBE, false),
        b'`' => (0xC0, false),
        b'/' => (0xBF, false),
        b' ' => (0x20, false),
        b'\'' => (0xDE, false),
        b'~' => (0xC0, true),
        b'!' => (0x31, true),
        b'@' => (0x32, true),
        b'#' => (0x33, true),
        b'$' => (0x34, true),
        b'%' => (0x35, true),
        b'^' => (0x36, true),
        b'&' => (0x37, true),
        b'*' => (0x38, true),
        b'(' => (0x39, true),
        b')' => (0x30, true),
        b'_' => (0xBD, true),
        b'+' => (0xBB, true),
        b'{' => (0xDB, true),
        b'}' => (0xDD, true),
        b'|' => (0xDC, true),
        b':' => (0xBA, true),
        b'"' => (0xDE, true),
        b'<' => (0xBC, true),
        b'>' => (0xBE, true),
        _ => (0xBF, true),
    }
}
fn send_text(s: &[u8]) -> Win32Result<()> {
    let mut b = [Input::default(); 256];
    if s.len() < 64 {
        return send_keys(&mut b, s);
    }
    let mut i = 0;
    while i < s.len() {
        let mut e = i + 64;
        if e > s.len() {
            e = s.len();
        }
        send_keys(&mut b, &s[i..e])?;
        i = e;
    }
    Ok(())
}
fn send_keys(buf: &mut [Input; 256], s: &[u8]) -> Win32Result<()> {
    let (mut i, mut n) = (0, 0);
    while i < s.len() && i < 64 && n < 256 {
        let (k, u) = key_code(s[i]);
        if u {
            (buf[n].key_type, buf[n].key.key, buf[n].key.flags) = (1, 0x10, 0);
            n += 1;
        }
        (buf[n].key_type, buf[n].key.key, buf[n].key.flags) = (1, k, 0);
        n += 1;
        (buf[n].key_type, buf[n].key.key, buf[n].key.flags) = (1, k, 2);
        n += 1;
        if u || k == 0x20 {
            (buf[n].key_type, buf[n].key.key, buf[n].key.flags) = (1, 0x10, 2);
            n += 1;
        }
        i += 1;
    }
    let r = unsafe {
        winapi::syscall!(
            *user32::SendInput,
            extern "stdcall" fn(u32, *const Input, u32) -> u32,
            n as u32,
            buf.as_ptr(),
            size_of::<Input>() as u32
        )
    } as usize;
    if r != n {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
