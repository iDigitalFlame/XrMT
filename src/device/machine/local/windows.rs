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

use core::alloc::Allocator;

use crate::data::str::Fiber;
use crate::device;
use crate::device::machine::{arch, os};
use crate::device::winapi::{self, registry};
use crate::prelude::*;
use crate::util::{crypt, ToStr};

#[inline]
pub fn system() -> u8 {
    #[cfg(not(target_pointer_width = "64"))]
    if winapi::in_wow64_process() {
        return (os::CURRENT as u8) << 4
            | if cfg!(target_arch = "x86") {
                arch::Architecture::X86OnX64
            } else {
                arch::Architecture::ArmOnArm64
            } as u8;
    }
    (os::CURRENT as u8) << 4 | arch::CURRENT as u8
}
#[inline]
pub fn elevated() -> u8 {
    (if winapi::NetGetJoinInformation(None).map_or(false, |v| v.1 == 3) {
        0x80
    } else {
        0
    }) + if winapi::is_elevated() { 1 } else { 0 }
}
#[inline]
pub fn system_id() -> Option<Vec<u8>> {
    if let Ok(v) = winapi::local_system_sid() {
        return Some(v.into_bytes());
    }
    Some(
        registry::open(
            registry::HKEY_LOCAL_MACHINE,
            Some(crypt::get_or(0, r"Software\Microsoft\Cryptography")),
            0x101, // 0x101 - KEY_WOW64_64KEY | KEY_QUERY_VALUE
        )
        .ok()?
        .value_string(crypt::get_or(0, "MachineGuid"))
        .ok()?
        .0
        .into_bytes(),
    )
}
pub fn version<A: Allocator + Clone>(alloc: A) -> Fiber<A> {
    // 0x101 - KEY_WOW64_64KEY | KEY_QUERY_VALUE
    let q = registry::open(
        registry::HKEY_LOCAL_MACHINE,
        crypt::get_or(0, r"Software\Microsoft\Windows NT\CurrentVersion"),
        0x101,
    )
    .ok()
    .and_then(|k| k.value_string(crypt::get_or(0, "ProductName")).ok().map(|s| s.0));
    let mut o = Fiber::with_capacity_in(16, alloc);
    match q {
        Some(r) => o.push_str(&r),
        None => o.push_str(crypt::get_or(0, "Windows")),
    }
    let (m, n, s) = winapi::GetVersionNumbers();
    if s == 0 && m == 0 && n == 0 {
        return o;
    }
    unsafe {
        let v = o.as_mut_vec();
        v.push(b' ');
        v.push(b'(');
        if m > 0 {
            m.into_vec(v);
            if n > 0 {
                v.push(b'.');
                n.into_vec(v);
            }
        }
        if s > 0 {
            if m > 0 {
                v.push(b',');
                v.push(b' ');
            }
            s.into_vec(v);
        }
        // Check anything before Win7.
        if m <= 6 || (m == 6 && n < 2) {
            // This shouldn't fail., but lets be cautious.
            let a = winapi::GetVersion().map_or(0, |d| d.sp_major);
            if a > 0 {
                v.push(b';');
                v.push(b' ');
                v.push(b'S');
                v.push(b'P');
                a.into_vec(v);
            }
        }
        v.push(b')')
    }
    o
}
#[inline]
pub fn username<A: Allocator + Clone>(alloc: A) -> Fiber<A> {
    device::whoami_in(alloc.clone()).unwrap_or_else(|_| b'?'.into_alloc(alloc))
}
