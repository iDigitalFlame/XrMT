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

// Unsupported Guard
//
// Unsupported Operating Systems
//
// psp     - PSP: BSD-like, but not much support.
// vita    - PSP Vita: BSD-like, but not much support.
// uefi    - UEFI Firmware: Missing alot, not really a full "OS".
// cuda    - Nvidia Cuda Cluster OS: Missing Process, Thread support.
// teeos   - Embedded Os: Missing Network, Process, Thread support.
// espidf  - Arduino-like: Missing Network, Process, Thread support.
// hermit  - Unikernel Micro Services: Missing Process, Thread support.
// horizon - Docker-ish/Containers: Missing Process, Thread, User support.
// vxworks - Embedded Os: Missing Network, Process, Thread support.
// unknown - Unknown: No support for many things.

// Unsupported Architectures
//
// avr     - Arduino-like: Missing Network, Process, Thread support.
// bpf     - Unknown?
// m68k    - Motorolla Mobile?: No support for many things.
// msp430  - Microcontroller: Missing Network, Process, Thread support.
// nvptx64 - Nvidia Cuda Cluster OS: Missing Process, Thread support.
// hexagon - Docker-ish/Containers: Missing Process, Thread, User support.
//
// These are unsupported dude to lack of testing.
//
// haiku   - https://www.haiku-os.org
// l4re
#[cfg(all(
    any(
        target_arch = "avr",
        target_arch = "bpf",
        target_arch = "m68k",
        target_arch = "msp430",
        target_arch = "nvptx64",
        target_arch = "hexagon",
        target_os = "psp",
        target_os = "vita",
        target_os = "l4re",
        target_os = "uefi",
        target_os = "cuda",
        target_os = "teeos",
        target_os = "haiku",
        target_os = "espidf",
        target_os = "hermit",
        target_os = "horizon",
        target_os = "vxworks",
        target_os = "unknown",
    ),
    not(target_vendor = "fortanix")
))]
compile_error!("Sorry the current target arch/os is unsupported!");

// Supported Architectures
//
// ARM     - arm | aarch64 | arm64ec
// LOONG   - loongarch64 (no Windows support)
// MIPS    - mips | mips32r6 | mips64 | mips64r6 (no Windows support)
// POWERPC - powerpc | powerpc64 (no Windows support)
// RISCV   - riscv32 | riscv64 (no Windows support)
// SPARC   - sparc | sparc64 (no Windows support) (NEW!!)
// x86     - s390x | x86
// amd64   - x86_64

// Supported Operating Systems
// Linux-like - android | linux | fuchsia | illumos | solaris
// BSD-Like   - dragonfly | freebsd | hurd | ios | macos | netbsd | openbsd |
//               redox | tvos | visionos | watchos
// Windows    - windows

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::cmp::Ordering;
use core::str::from_utf8_unchecked;

use crate::data::str::Fiber;
use crate::data::time::Time;
use crate::prelude::*;
use crate::{env, io, number_like};

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::id::ID;
pub use self::machine::{capabilities, local_id, Machine};
pub use self::network::{Address, HardwareAddress, Interface, Network};
pub use self::sys::*;

mod id;
mod machine;
mod network;
pub mod winapi;

number_like!(Evasion, u8);

#[repr(u8)]
pub enum Status {
    Active         = 0x00,
    Connected      = 0x01,
    ConnectedQuery = 0x02,
    Shadow         = 0x03,
    Disconnected   = 0x04,
    Idle           = 0x05,
    Listen         = 0x06,
    Reset          = 0x07,
    Down           = 0x08,
    Init           = 0x09,
    Unknown        = 0xFF,
}

pub struct Evasion(u8);
pub struct Shell<A: Allocator = Global> {
    pub sh:   Fiber<A>,
    pub pwsh: Option<Fiber<A>>,
}
pub struct Login<A: Allocator = Global> {
    pub login_time: Time,
    pub last_input: Time,
    pub user:       Fiber<A>,
    pub host:       Fiber<A>,
    pub from:       Address,
    pub id:         u32,
    pub status:     u8,
}

impl Evasion {
    pub const NONE: Evasion = Evasion(0x00);
    pub const WIN_PATCH_TRACE: Evasion = Evasion(0x01);
    pub const WIN_PATCH_AMSI: Evasion = Evasion(0x02);
    pub const WIN_HIDE_THREADS: Evasion = Evasion(0x04);
    pub const ERASE_HEADER: Evasion = Evasion(0x08);
    pub const ALL: Evasion = Evasion(0xFF);
}

impl From<u16> for Evasion {
    #[inline]
    fn from(v: u16) -> Evasion {
        Evasion::from(v as u8)
    }
}
impl From<u32> for Evasion {
    #[inline]
    fn from(v: u32) -> Evasion {
        Evasion::from(v as u8)
    }
}
impl From<u64> for Evasion {
    #[inline]
    fn from(v: u64) -> Evasion {
        Evasion::from(v as u8)
    }
}
impl From<usize> for Evasion {
    #[inline]
    fn from(v: usize) -> Evasion {
        Evasion::from(v as u8)
    }
}

impl Shell {
    #[inline]
    pub fn new() -> Shell {
        Shell::new_in(Global)
    }
}
impl<A: Allocator + Clone> Shell<A> {
    #[inline]
    pub fn new_in(alloc: A) -> Shell<A> {
        Shell {
            sh:   shell_in(alloc.clone()),
            pwsh: powershell_in(alloc),
        }
    }
}

impl<A: Allocator> Eq for Login<A> {}
impl<A: Allocator> Ord for Login<A> {
    #[inline]
    fn cmp(&self, other: &Login<A>) -> Ordering {
        if self.id == other.id {
            if self.login_time.eq(&other.login_time) {
                self.user.cmp(&other.user)
            } else {
                self.login_time.cmp(&other.login_time)
            }
        } else {
            self.id.cmp(&other.id)
        }
    }
}
impl<A: Allocator> PartialEq for Login<A> {
    #[inline]
    fn eq(&self, other: &Login<A>) -> bool {
        self.login_time.eq(&other.login_time) && self.last_input.eq(&other.last_input) && self.user.eq(&other.user) && self.host.eq(&other.host) && self.from.eq(&other.from) && self.id == other.id && self.status == other.status
    }
}
impl<A: Allocator> PartialOrd for Login<A> {
    #[inline]
    fn partial_cmp(&self, other: &Login<A>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<A: Allocator + Clone> Clone for Login<A> {
    #[inline]
    fn clone(&self) -> Login<A> {
        Login {
            id:         self.id.clone(),
            user:       self.user.clone(),
            host:       self.host.clone(),
            from:       self.from.clone(),
            status:     self.status.clone(),
            login_time: self.login_time.clone(),
            last_input: self.last_input.clone(),
        }
    }
}

#[inline]
pub fn shell() -> Fiber {
    shell_in(Global)
}
#[inline]
pub fn powershell() -> Option<Fiber> {
    powershell_in(Global)
}
#[inline]
pub fn whoami() -> io::Result<Fiber> {
    whoami_in(Global)
}
#[inline]
pub fn hostname() -> io::Result<Fiber> {
    hostname_in(Global)
}
#[inline]
pub fn logins() -> io::Result<Vec<Login>> {
    logins_in(Global)
}
#[inline]
pub fn mounts() -> io::Result<Vec<Fiber>> {
    mounts_in(Global)
}
#[inline]
pub fn expand(src: impl AsRef<str>) -> String {
    expand_fiber_in(src, Global).into()
}
#[inline]
pub fn expand_fiber(src: impl AsRef<str>) -> Fiber {
    expand_fiber_in(src, Global)
}
pub fn expand_fiber_in<A: Allocator>(src: impl AsRef<str>, alloc: A) -> Fiber<A> {
    let s = src.as_ref().as_bytes();
    let mut b = Fiber::with_capacity_in(s.len(), alloc);
    if s.len() == 0 {
        return b;
    }
    // NOTE(dij): RUST DIVERGENCE: Account for Windows formatting, ie (~\ instead of
    // just ~/).
    if s.len() >= 2 && s[0] == b'~' && (s[1] == b'/' || s[1] == b'\\') {
        if let Some(d) = home_dir() {
            b.push_str(&d.to_string_lossy());
            unsafe { b.as_mut_vec().push(b'/') }
        }
    }
    let r = unsafe { b.as_mut_vec() };
    let (mut l, mut c) = (-1isize, 0);
    for i in 0..s.len() {
        match s[i] {
            b'$' => {
                if c > 0 {
                    r.extend_from_slice(&s[(l - if c == b'{' { 1 } else { 0 }) as usize..i])
                }
                (c, l) = (s[i], i as isize);
            },
            b'%' if c == b'%' && i != l as usize => {
                match env::var(unsafe { from_utf8_unchecked(&s[l as usize + 1..i]) }) {
                    Ok(v) => r.extend_from_slice(v.as_bytes()),
                    Err(_) => r.extend_from_slice(&s[l as usize..i + 1]),
                }
                (c, l) = (0, 0)
            },
            b'%' => {
                if c > 0 {
                    r.extend_from_slice(&s[l as usize..i])
                }
                (c, l) = (s[i], i as isize)
            },
            b'}' if c == b'{' => {
                match env::var(unsafe { from_utf8_unchecked(&s[l as usize + 1..i]) }) {
                    Ok(v) => r.extend_from_slice(v.as_bytes()),
                    Err(_) => r.extend_from_slice(&s[l as usize - 1..i + 1]),
                }
                (c, l) = (0, 0)
            },
            b'{' if i > 0 && c == b'$' => (c, l) = (s[i], i as isize),
            _ if s[i] == b'_' || (b'a' <= s[i] && s[i] <= b'z') || (b'A' <= s[i] && s[i] <= b'Z') => {
                // We don't limit to 'c' here as we want to capture when 'c' > 0
                if c == 0 {
                    r.push(s[i])
                }
            },
            _ if b'0' <= s[i] && s[i] <= b'9' => {
                if c > 0 && i > l as usize && i - l as usize == 1 {
                    (c, l) = (0, 0)
                }
                if c == 0 {
                    r.push(s[i])
                }
            },
            _ => {
                if c == b'$' {
                    match env::var(unsafe { from_utf8_unchecked(&s[l as usize + 1..i]) }) {
                        Ok(v) => r.extend_from_slice(v.as_bytes()),
                        Err(_) => r.extend_from_slice(&s[l as usize..i]),
                    }
                    (c, l) = (0, 0)
                } else if c > 0 {
                    r.extend_from_slice(&s[(l - if c == b'{' { 1 } else { 0 }) as usize..i]);
                    (c, l) = (0, 0)
                }
                r.push(s[i])
            },
        }
    }
    if l == -1 {
        return b;
    }
    if (l as usize) < s.len() && c > 0 {
        match c {
            b'$' => match env::var(unsafe { from_utf8_unchecked(&s[l as usize + 1..]) }) {
                Ok(v) => r.extend_from_slice(v.as_bytes()),
                Err(_) => r.extend_from_slice(&s[l as usize..]),
            },
            b'{' => r.extend_from_slice(&s[l as usize - 1..]),
            _ => r.extend_from_slice(&s[l as usize..]),
        }
    }
    b
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
pub mod unix {
    pub use crate::device::sys::unix::unix::{sysctl, sysctl_by_name};
}

#[cfg(all(
    target_family = "windows",
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64")
))]
mod sys {
    mod windows;
    pub use self::windows::*;
}
#[cfg(all(
    not(target_family = "windows"),
    not(target_os = "wasi"),
    not(target_os = "emscripten"),
    not(target_arch = "wasm32"),
    not(target_arch = "wasm64")
))]
mod sys {
    pub use self::unix::*;
    pub mod unix;
}
#[cfg(all(
    not(target_family = "unix"),
    not(target_family = "windows"),
    any(
        target_os = "wasi",
        target_os = "emscripten",
        target_arch = "wasm32",
        target_arch = "wasm64"
    ),
))]
mod sys {
    mod wasm;
    pub use self::wasm::*;
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Formatter};

    use crate::device::Login;
    use crate::prelude::*;

    impl Debug for Login {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Login")
                .field("login_time", &self.login_time)
                .field("last_input", &self.last_input)
                .field("user", &self.user)
                .field("host", &self.host)
                .field("from", &self.from)
                .field("id", &self.id)
                .field("status", &self.status)
                .finish()
        }
    }
}
