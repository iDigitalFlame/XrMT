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

use core::slice::from_raw_parts;
use core::{cmp, ptr};

use crate::data::str::MaybeString;
use crate::device::winapi::functions::structs::{WTSProcess, WTSSession};
use crate::device::winapi::loader::wtsapi32;
use crate::device::winapi::{self, Session, SessionHandle, SessionProcess, WChar, WChars, Win32Result};
use crate::prelude::*;

#[inline]
pub fn wts_sessions_by_name(server: impl MaybeString) -> Win32Result<Vec<Session>> {
    let h = WTSOpenServer(server)?;
    WTSGetSessions(&h)
}

#[inline]
pub fn WTSCloseServer(h: &SessionHandle) {
    if h.0 == 0 {
        return;
    }
    winapi::init_wtsapi32();
    unsafe { winapi::syscall!(*wtsapi32::WTSCloseServer, extern "stdcall" fn(usize), h.0) }
}
pub fn WTSGetSessions(h: &SessionHandle) -> Win32Result<Vec<Session>> {
    winapi::init_wtsapi32();
    let (mut c, mut b) = (0u32, 0usize);
    let r = unsafe {
        winapi::syscall!(
            *wtsapi32::WTSEnumerateSessions,
            extern "stdcall" fn(usize, u32, u32, *mut usize, *mut u32) -> u32,
            h.0,
            0,
            1,
            &mut b,
            &mut c
        )
    };
    if r == 0 {
        return Err(winapi::last_error());
    }
    let mut o = Vec::with_capacity(c as usize);
    if c > 0 {
        let v = winapi::is_min_windows_7();
        for i in unsafe { from_raw_parts(b as *const WTSSession, c as usize) } {
            o.push(i.into_inner(h, v)?);
        }
    }
    winapi::LocalFree(b as *const u8);
    Ok(o)
}
pub fn WTSOpenServer(server: impl MaybeString) -> Win32Result<SessionHandle> {
    winapi::init_wtsapi32();
    if let Some(v) = server.into_string() {
        let n = WChar::from(v);
        let h = unsafe {
            winapi::syscall!(
                *wtsapi32::WTSOpenServer,
                extern "stdcall" fn(*const u16) -> SessionHandle,
                if n.is_empty() { ptr::null() } else { n.as_ptr() }
            )
        };
        return if h.0 == 0 { Err(winapi::last_error()) } else { Ok(h) };
    }
    Ok(SessionHandle::default())
}
pub fn WTSLogoffSession(h: &SessionHandle, session_id: u32, wait: bool) -> Win32Result<()> {
    winapi::init_wtsapi32();
    let r = unsafe {
        winapi::syscall!(
            *wtsapi32::WTSLogoffSession,
            extern "stdcall" fn(usize, u32, u32) -> u32,
            h.0,
            session_id,
            if wait { 1 } else { 0 }
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WTSDisconnectSession(h: &SessionHandle, session_id: u32, wait: bool) -> Win32Result<()> {
    winapi::init_wtsapi32();
    let r = unsafe {
        winapi::syscall!(
            *wtsapi32::WTSDisconnectSession,
            extern "stdcall" fn(usize, u32, u32) -> u32,
            h.0,
            session_id,
            if wait { 1 } else { 0 }
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(())
    }
}
pub fn WTSEnumerateProcesses(h: &SessionHandle, session_id: i32) -> Win32Result<Vec<SessionProcess>> {
    winapi::init_wtsapi32();
    let (mut c, mut b) = (0u32, 0usize);
    let r = unsafe {
        winapi::syscall!(
            *wtsapi32::WTSEnumerateProcesses,
            extern "stdcall" fn(usize, u32, u32, *mut usize, *mut u32) -> u32,
            h.0,
            0,
            1,
            &mut b,
            &mut c
        )
    };
    if r == 0 {
        return Err(winapi::last_error());
    }
    let mut o = Vec::with_capacity(c as usize);
    if c > 0 {
        for i in unsafe { from_raw_parts(b as *const WTSProcess, c as usize) } {
            if session_id < 0 || session_id as u32 == i.session_id {
                o.push(i.into_inner())
            }
        }
    }
    winapi::LocalFree(b as *const u8);
    Ok(o)
}
pub fn WTSSendMessage(h: &SessionHandle, session_id: u32, title: impl MaybeString, text: impl MaybeString, flags: u32, secs: u32, wait: bool) -> Win32Result<u32> {
    winapi::init_wtsapi32();
    let mut o = 0u32;
    let r = unsafe {
        let d: WChars = text.into_string().into();
        let t: WChars = title.into_string().into();
        winapi::syscall!(
            *wtsapi32::WTSSendMessage,
            extern "stdcall" fn(usize, u32, *const u16, u32, *const u16, u32, u32, u32, *mut u32, u32) -> u32,
            h.0,
            session_id,
            if t.is_empty() { ptr::null() } else { t.as_ptr() },
            cmp::min(t.len_as_bytes(), 0xFFFFFFFF) as u32,
            if d.is_empty() { ptr::null() } else { d.as_ptr() },
            cmp::min(d.len_as_bytes(), 0xFFFFFFFF) as u32,
            flags,
            secs,
            &mut o,
            if wait { 1 } else { 0 }
        )
    };
    if r == 0 {
        Err(winapi::last_error())
    } else {
        Ok(o)
    }
}
