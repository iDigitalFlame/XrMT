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
#![cfg(all(
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_family = "windows"),
))]

use core::alloc::Allocator;

use crate::data::str::Fiber;
use crate::device::machine::{arch, os};
use crate::device::whoami_in;
use crate::prelude::*;
use crate::util::crypt;

core::cfg_match! {
    cfg(all(target_vendor = "apple", any(target_os = "ios", target_os = "tvos", target_os = "watchos", target_os = "visionos"))) => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "iOS")}
    }
    cfg(target_vendor = "apple") => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "MacOS")}
    }
    cfg(any(target_os = "redox", target_os = "netbsd", target_os = "openbsd", target_os = "freebsd", target_os = "dragonfly", target_os = "hurd")) => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "Unix")}
    }
    cfg(target_os = "android") => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "Android")}
    }
    cfg(target_family = "unix") => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "Linux")}
    }
    _ => {
        fn _name<'a>() -> &'a str { crypt::get_or(0, "Unknown")}
    }
}

#[inline]
pub fn system() -> u8 {
    #[cfg(all(target_vendor = "apple", target_arch = "x86_64"))]
    if inner::arch_check() {
        // Check for Rosetta Emulation
        return (os::CURRENT as u8) << 4 | arch::Architecture::Emulated as u8;
    }
    #[cfg(all(not(target_pointer_width = "64"), not(target_vendor = "fortanix")))]
    if inner::arch_check() && (arch::CURRENT == arch::Architecture::X86 || arch::CURRENT == arch::Architecture::Arm) {
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
    sys::elevated()
}
#[inline]
pub fn system_id() -> Option<Vec<u8>> {
    inner::system_id()
}
pub fn version<A: Allocator + Clone>(alloc: A) -> Fiber<A> {
    let mut b = Fiber::new_in(alloc.clone());
    let (p, i) = inner::product();
    match p {
        Some(v) => b.push_str(&v),
        None => b.push_str(_name()),
    }
    let r = sys::release();
    let l = r.is_ok();
    if l || i.is_some() {
        b.push_str(" (")
    } else {
        return b;
    }
    if let Ok(v) = r {
        b.push_str(&v);
        if l {
            b.push_str(", ");
        }
    }
    if let Some(v) = i {
        b.push_str(&v)
    }
    unsafe { b.as_mut_vec().push(b')') };
    b
}
#[inline]
pub fn username<A: Allocator + Clone>(alloc: A) -> Fiber<A> {
    whoami_in(alloc.clone()).unwrap_or_else(|_| b'?'.into_alloc(alloc))
}

#[cfg(target_vendor = "fortanix")]
mod sys {
    use crate::io::{self, ErrorKind};
    use crate::prelude::*;

    #[inline]
    pub fn elevated() -> u8 {
        0
    }
    #[inline]
    pub fn release() -> io::Result<String> {
        Err(ErrorKind::Unsupported.into())
    }
}
#[cfg(not(target_vendor = "fortanix"))]
mod sys {
    extern crate libc;

    use core::ffi::CStr;
    use core::mem::zeroed;

    use crate::io::{self, Error};
    use crate::prelude::*;

    #[inline]
    pub fn elevated() -> u8 {
        if unsafe { libc::getuid() == 0 || libc::getegid() == 0 } {
            1
        } else {
            0
        }
    }
    #[inline]
    pub fn release() -> io::Result<String> {
        let mut u = unsafe { zeroed() };
        let r = unsafe { libc::uname(&mut u) };
        // Solais returns 1 for success also, so account for that here.
        if r >= 0 {
            Ok(unsafe { CStr::from_ptr(u.release.as_ptr()) }.to_string_lossy().to_string())
        } else {
            Err(Error::from_raw_os_error(r))
        }
    }
}

#[cfg(not(target_vendor = "apple"))]
mod nix {
    use crate::fs::{read_dir, read_to_string};
    use crate::prelude::*;
    use crate::util::crypt;
    use crate::{ok_or_continue, ok_or_return};

    pub fn product() -> (Option<String>, Option<String>) {
        let (mut x, mut y) = (None, None);
        let g = ok_or_return!(read_dir(crypt::get_or(0, "/etc")), (None, None));
        for i in g {
            let e = ok_or_continue!(i);
            if !check(e.file_name().as_encoded_bytes()) {
                continue;
            }
            if let Ok(d) = read_to_string(e.path()) {
                let (n, v) = read_details(d);
                if n.is_some() && x.is_none() {
                    x = n;
                }
                if v.is_some() && y.is_none() {
                    y = v;
                }
            }
            if x.is_some() && y.is_some() {
                break;
            }
        }
        (x, y)
    }

    fn check(v: &[u8]) -> bool {
        let mut p = 0;
        for i in v {
            match i {
                b'r' if p == 0 => p += 1,
                b'e' if p == 1 => p += 1,
                b'l' if p == 2 => p += 1,
                b'e' if p == 3 => p += 1,
                b'a' if p == 4 => p += 1,
                b's' if p == 5 => p += 1,
                b'e' if p == 6 => p += 1,
                _ => p = 0,
            }
            if p == 7 {
                return true;
            }
        }
        false
    }
    #[inline]
    fn trim<'a>(b: &'a [u8]) -> &'a [u8] {
        let e = b.len() - 1;
        match (b[0], b[e]) {
            (b'"', b'"') => &b[1..e],
            (_, b'"') => &b[0..e],
            (b'"', _) => &b[1..],
            _ => b,
        }
    }
    fn read_details(d: String) -> (Option<String>, Option<String>) {
        let (mut x, mut u) = (None, None);
        for l in d.split(|v| v == '\n') {
            if l.len() <= 1 {
                continue;
            }
            let b = l.as_bytes();
            let p = b.iter().position(|v| *v == b'=').unwrap_or_default();
            if p == 0 || p + 1 >= b.len() {
                continue;
            }
            let v = match b[0] {
                    b'P' | b'p' /* PRETTY_NAME */ if l.len() > 0xB && b[0xB] == b'=' && b[0x6] == b'_' && (b[0x5] == b'y' || b[0x5] == b'Y') => Some((&b[p+1..], 0x2u8)),
                    b'N' | b'n' /* NAME */ if l.len() > 0x5 && b[0x4] == b'=' && (b[0x2] == b'm' || b[0x2] == b'M') => Some((&b[p+1..], 0x1u8)),
                    b'I' | b'i' /* ID */ if l.len() > 0x3 && b[0x2] == b'=' && (b[0x1] == b'd' || b[0x1] == b'D') => {
                        // Set the ID value.
                        Some((&b[p+1..], 0x0u8))
                    },
                    _ => None,
                };
            match v.clone() {
                Some((t, n)) if n == 0 && u.is_none() => u = Some(t),
                _ => (),
            }
            match (x, v) {
                (None, Some((..))) => x = v,
                (Some((_, n)), Some((_, w))) if w > n => x = v,
                _ => (),
            }
        }
        match (x, u) {
            (Some((v, _)), Some(y)) => (
                Some(String::from_utf8_lossy(trim(v)).to_string()),
                Some(String::from_utf8_lossy(trim(y)).to_string()),
            ),
            (Some((v, _)), None) => (Some(String::from_utf8_lossy(trim(v)).to_string()), None),
            (None, Some(y)) => (None, Some(String::from_utf8_lossy(trim(y)).to_string())),
            _ => (None, None),
        }
    }
}

#[cfg(any(
    target_os = "hurd",
    target_os = "redox",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_vendor = "apple",
))]
mod unix {
    extern crate libc;

    use crate::device::unix::{sysctl, sysctl_by_name};
    use crate::fs;
    use crate::prelude::*;
    use crate::util::crypt;

    #[cfg(any(target_vendor = "apple", target_os = "netbsd"))]
    const KERN_HOSTUUID: i32 = 0x24i32;
    #[cfg(all(not(target_vendor = "apple"), not(target_os = "netbsd")))]
    const KERN_HOSTUUID: i32 = libc::KERN_HOSTUUID as _;

    #[cfg(any(
        not(target_pointer_width = "64"),
        all(target_vendor = "apple", target_arch = "x86_64") // Check for Rosseta
    ))]
    #[inline]
    pub fn arch_check() -> bool {
        #[cfg(target_vendor = "apple")]
        {
            sysctl_by_name("sysctl.proc_translated", false).map_or(false, |v| v.len() >= 4 && (v[3] == 1 || v[0] == 1))
        }
        // BSD-Like uses something different. Calling most sysctl values emulate
        // x86 when called from an x86 process. However, when you ask about elf64,
        // BSD will ONLY have this key if it supports elf64 (ie: x64/AARCH64).
        // So we just check if this is Some or None.
        //
        // There doesn't seem to be a constant for for this one either.
        //
        // TODO(dij): This won't work on NetBSD! Not sure how to do that tbh.
        //            This value doesn't exist.
        #[cfg(not(target_vendor = "apple"))]
        {
            sysctl_by_name(crypt::get_or(0, "kern.elf64.aslr.enable"), false).is_ok()
        }
    }
    #[inline]
    pub fn system_id() -> Option<Vec<u8>> {
        // 0x24 - KERN_HOSTUUID
        // 0x1  - CTL_KERN
        //  Same as 'sysctl_by_name("kern.hostuuid", true)'
        //   Won't work on MacOS (mostly), we check just in case and fallback to
        //   'kern.uuid'.
        //
        // I don't see any constants that match "kern.uuid" (which is mostly for
        // MacOS).
        //
        // NetBSD only has "machdep.dmi.system-uuid".
        if let Ok(v) = sysctl([libc::CTL_KERN, KERN_HOSTUUID], true)
            .or_else(|_| sysctl_by_name(crypt::get_or(0, "kern.uuid"), true))
            .or_else(|_| sysctl_by_name(crypt::get_or(0, "machdep.dmi.system-uuid"), true))
        {
            return Some(v.into_vec());
        }
        fs::read(crypt::get_or(0, "/etc/host-id"))
            .or_else(|_| fs::read(crypt::get_or(0, "/etc/machine-id")))
            .ok()
    }
}

#[cfg(target_vendor = "apple")]
mod inner {
    extern crate libc;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use super::unix::*;

    use crate::device::unix::{sysctl, sysctl_by_name};
    use crate::prelude::*;
    use crate::util::crypt;

    #[inline]
    pub fn product() -> (Option<String>, Option<String>) {
        // There doesn't seem to be a constant value for "kern.osproductversion".
        let b = match sysctl_by_name(crypt::get_or(0, "kern.osproductversion"), true) {
            Err(_) => None,
            Ok(v) => {
                let mut s = v.to_string();
                s.reserve(6);
                unsafe {
                    let t = s.as_mut_vec();
                    t.insert(0, b' ');
                    t.insert(0, b'S');
                    t.insert(0, b'O');
                    t.insert(0, b'c');
                    t.insert(0, b'a');
                    t.insert(0, b'M');
                }
                Some(s)
            },
        };
        // 0x41 - KERN_OSVERSION
        // 0x1  - CTL_KERN
        //  Same as 'sysctl_by_name("kern.osversion", true)'
        (
            b,
            sysctl([libc::CTL_KERN, libc::KERN_OSVERSION], true)
                .ok()
                .map(|v| v.to_string()),
        )
    }
}
#[cfg(all(
    not(target_vendor = "apple"),
    any(
        target_os = "hurd",
        target_os = "redox",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "freebsd",
        target_os = "dragonfly",
    )
))]
mod inner {
    extern crate libc;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use super::unix::*;

    use crate::data::blob::Blob;
    use crate::data::{s2a_u16, s2a_u32, s2a_u64};
    use crate::device::unix::sysctl;
    use crate::prelude::*;
    use crate::util::ToStr;

    pub fn product() -> (Option<String>, Option<String>) {
        // 0x1 - KERN_OSTYPE
        // 0x1 - CTL_KERN
        //  Same as 'sysctl_by_name("kern.ostype", true)'
        // 0x3 - KERN_OSREV
        //  Same as 'sysctl_by_name("kern.osrevision", true)'
        let t: Option<Blob<u8>> = sysctl([libc::CTL_KERN, libc::KERN_OSTYPE], true).ok();
        let b = sysctl([libc::CTL_KERN, libc::KERN_OSREV], false)
            .ok()
            .and_then(|b| match b.len() {
                0 | 1 | 3 | 5 | 6 | 7 => None,
                2 => Some(u16::from_ne_bytes(s2a_u16(&b)) as u64),
                4 => Some(u32::from_ne_bytes(s2a_u32(&b)) as u64),
                8 => Some(u64::from_ne_bytes(s2a_u64(&b))),
                _ => None,
            });
        // Quickpath if both pass.
        let (x, y) = match (t.as_ref(), b) {
            (Some(d), Some(i)) => return (Some(d.to_string()), Some(i.into_string())),
            _ => (t, b),
        };
        // Fallback to reading files (non-MacOS).
        let (o, p) = super::nix::product();
        match (o, p, x, y) {
            (Some(_), Some(i), Some(d), None) => (Some(i), Some(d.to_string())),
            (Some(d), Some(_), None, Some(i)) => (Some(d), Some(i.into_string())),
            (Some(d), Some(i), ..) => (Some(d), Some(i)),
            (Some(d), None, ..) => (Some(d), None),
            (None, Some(i), ..) => (None, Some(i)),
            _ => (None, None),
        }
    }
}
#[cfg(all(
    not(target_os = "hurd"),
    not(target_os = "redox"),
    not(target_os = "netbsd"),
    not(target_os = "openbsd"),
    not(target_os = "freebsd"),
    not(target_os = "dragonfly"),
    not(target_vendor = "apple"),
    not(target_vendor = "fortanix"),
))]
mod inner {
    extern crate libc;

    use crate::fs;
    use crate::prelude::*;
    use crate::util::crypt;

    #[cfg(not(target_pointer_width = "64"))]
    #[inline]
    pub fn arch_check() -> bool {
        let mut u = unsafe { core::mem::zeroed() };
        let r = unsafe { libc::uname(&mut u) };
        // The 'machine' section should cover the /real/ arch.
        match r {
            _ if core::intrinsics::unlikely(u.machine.len() < 9) => false,
            0 if u.machine[0xA] == 0 && u.machine[0] == 0x61 && u.machine[6] == 0x34 && u.machine[5] == 0x36 && u.machine[9] == 0x65 => true, // Match aarch64_be
            0 if u.machine[0x7] == 0 && u.machine[0] == 0x61 && u.machine[6] == 0x34 && u.machine[5] == 0x36 => true,                         // Match aarch64
            0 if u.machine[0x6] == 0 && u.machine[0] == 0x61 && u.machine[4] == 0x38 && u.machine[3] == 0x76 => true,                         // Match armv8l and armv8b
            0 if u.machine[0x6] == 0 && u.machine[0] == 0x78 && u.machine[5] == 0x34 && u.machine[4] == 0x36 => true,                         // Match x86_64
            _ => false,
        }
    }
    #[inline]
    pub fn system_id() -> Option<Vec<u8>> {
        #[cfg(all(not(target_os = "fuchsia"), not(target_os = "android")))]
        {
            // TODO(dij): There may be a better way to this on Android.
            let r = unsafe { libc::gethostid() };
            if r > 0 {
                let mut b = Vec::new();
                // Prevent an unused import
                crate::util::ToStr::into_vec(r, &mut b);
                return Some(b);
            }
        }
        fs::read(crypt::get_or(0, "/var/lib/dbus/machine-id"))
            .or_else(|_| fs::read(crypt::get_or(0, "/etc/machine-id")))
            .ok()
    }
    #[inline]
    pub fn product() -> (Option<String>, Option<String>) {
        super::nix::product()
    }
}
#[cfg(target_vendor = "fortanix")]
mod inner {
    use crate::fs;
    use crate::prelude::*;
    use crate::util::crypt;

    #[inline]
    pub fn system_id() -> Option<Vec<u8>> {
        fs::read(crypt::get_or(0, "/var/lib/dbus/machine-id"))
            .or_else(|_| fs::read(crypt::get_or(0, "/etc/machine-id")))
            .ok()
    }
    #[inline]
    pub fn product() -> (Option<String>, Option<String>) {
        super::nix::product()
    }
}
