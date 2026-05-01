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

extern crate core;

use core::result::Result::Err;
use core::sync::atomic::{AtomicU8, Ordering};

use crate::structs::SystemVersion;

// Use values from https://en.wikipedia.org/wiki/List_of_Microsoft_Windows_versions
const WINVER_WIN_UNKNOWN: u8 = 0u8;
/// Windows Xp Product Line (Except Server 2003/x64).
/// - Also Covers Windows 2000/NT
///
/// NT version: 5.0
const WINVER_WIN_XP: u8 = 1u8;
/// Windows Server 2003 or Xp Anvil (x64 Edition).
///
/// NT version: 5.x
const WINVER_WIN_XP64: u8 = 2u8;
/// Windows Vista or Windows Server 2008
///
/// NT version 6.0
const WINVER_WIN_VISTA: u8 = 3u8;
/// Windows 7 or Windows Server 2008 R2
///
/// NT version 6.1
const WINVER_WIN_7: u8 = 4u8;
/// Windows 8 or Windows Server 2012
///
/// NT version 6.2
const WINVER_WIN_8: u8 = 5u8;
/// Windows 8.1 or Windows Server 2012 R2
///
/// NT version 6.2
const WINVER_WIN_8_1: u8 = 6u8;
/// Windows 10 or Windows Server 2016
///
/// NT version 10
const WINVER_WIN_10: u8 = 7u8;
/// Windows 10 or Windows Server 2019
///
/// NT version 11
const WINVER_WIN_11: u8 = 8u8;

static VERSION: AtomicU8 = AtomicU8::new(0xFFu8);

#[inline]
pub fn is_windows_xp() -> bool {
    version() == WINVER_WIN_XP
}
#[cfg(target_pointer_width = "64")]
#[inline]
pub fn is_wow_process() -> bool {
    false // x64/AARCH64 is never WoW
}
#[cfg(not(target_pointer_width = "64"))]
#[inline]
pub fn is_wow_process() -> bool {
    // We cache this value to prevent from having to call it a lot.
    inner::is_wow_process()
}
#[inline]
pub fn is_windows_xp64() -> bool {
    version() == WINVER_WIN_XP64
}
#[inline]
pub fn is_min_windows_7() -> bool {
    version() >= WINVER_WIN_7
}
#[inline]
pub fn is_min_windows_8() -> bool {
    version() >= WINVER_WIN_8
}
#[inline]
pub fn is_min_windows_10() -> bool {
    version() >= WINVER_WIN_10
}
#[inline]
pub fn is_min_windows_8_1() -> bool {
    version() >= WINVER_WIN_8
}
#[inline]
pub fn is_min_windows_vista() -> bool {
    version() >= WINVER_WIN_VISTA
}

fn version() -> u8 {
    if let Err(r) = VERSION.compare_exchange(0xFF, 0xFE, Ordering::AcqRel, Ordering::Relaxed) {
        return r;
    }
    let v = SystemVersion::get();
    let r = match v.major {
        11 => WINVER_WIN_11,
        10 => WINVER_WIN_10,
        6 => match v.minor {
            0 => WINVER_WIN_VISTA,
            1 => WINVER_WIN_7,
            2 => WINVER_WIN_8,
            3 => WINVER_WIN_8_1,
            _ => WINVER_WIN_UNKNOWN,
        },
        5 => match v.minor {
            1 => WINVER_WIN_XP,
            // Test server 2003
            _ => WINVER_WIN_XP64,
            // 5.2 is used for Server 2003 and x64 Xp.
            // They both have the same stats and avaliable feature sets.
            //
            // https://en.wikipedia.org/wiki/List_of_Microsoft_Windows_versions#Server_versions
        },
        _ => WINVER_WIN_UNKNOWN,
    };
    VERSION.store(r, Ordering::Release);
    r
}

#[cfg(not(target_pointer_width = "64"))]
mod inner {
    extern crate core;

    use core::result::Result::Err;
    use core::sync::atomic::{AtomicU8, Ordering};

    use crate::functions::IsWoW64Process;
    use crate::{ntdll, CURRENT_PROCESS};

    const WOW_PRESENT: u8 = 1u8;
    const WOW_NOT_PRESENT: u8 = 0u8;

    static WOW: AtomicU8 = AtomicU8::new(0xFFu8);

    #[inline]
    pub fn is_wow_process() -> bool {
        // We cache this value to prevent from having to call it a lot.
        wow() == WOW_PRESENT
    }

    fn wow() -> u8 {
        if let Err(r) = WOW.compare_exchange(0xFF, 0xFE, Ordering::AcqRel, Ordering::Relaxed) {
            return r;
        }
        // Fastpath check. If the function exists inside ntdll.dll, then
        // we are 100% in a WoW64 process. It's an easy way without having to
        // run a syscall.
        let r = if ntdll().NtWow64QueryInformationProcess64.is_loaded() {
            WOW_PRESENT
        } else if IsWoW64Process(CURRENT_PROCESS).unwrap_or(false) {
            WOW_PRESENT
        } else {
            WOW_NOT_PRESENT
        };
        WOW.store(r, Ordering::Release);
        r
    }
}
