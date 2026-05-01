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
#![cfg(feature = "bugs")]

extern crate core;

extern crate xrmt_time;

use core::cell::UnsafeCell;
use core::fmt::{Arguments, Write};
use core::marker::{Send, Sync};
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
use core::write;

use xrmt_time::Time;

use crate::bugs::sys::Logger;

#[macro_export]
macro_rules! bugtrack {
    ($($arg:tt)*) => {
        $crate::log(format_args!($($arg)*))
    };
}

const NEWLINE: [u8; 1] = [b'\n'];

static BUGLOG: Bugs = Bugs::new();

struct Bugs {
    v: AtomicBool,
    e: UnsafeCell<MaybeUninit<Logger>>,
}

impl Bugs {
    #[inline]
    const fn new() -> Bugs {
        Bugs {
            v: AtomicBool::new(false),
            e: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn write_args(v: &mut Logger, args: Arguments<'_>) {
        let s = Time::now();
        let (d, t) = (s.date(), s.clock());
        let _ = write!(
            v,
            "{}/{:02}/{:02} {:02}:{:02}:{:02} [BUGTRACK]: ",
            d.0, d.1 as u8, d.2, t.0, t.1, t.2
        );
        let _ = v.write_fmt(args);
        v.write(&NEWLINE);
        v.flush();
    }

    #[inline]
    fn close(&self) {
        if self
            .v
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            unsafe { (&mut *(self.e.get())).assume_init_drop() }
        }
    }
    #[inline]
    fn init(&self) -> &mut Logger {
        if self
            .v
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            unsafe { (&mut *(self.e.get())).write(Logger::new()) };
        }
        unsafe { (&mut *(self.e.get())).assume_init_mut() }
    }
    #[inline]
    fn log(&self, args: Arguments<'_>) {
        Bugs::write_args(self.init(), args)
    }
}

unsafe impl Send for Bugs {}
unsafe impl Sync for Bugs {}

#[doc(hidden)]
#[inline]
pub fn log(args: Arguments<'_>) {
    BUGLOG.log(args);
}

#[inline]
pub unsafe fn close() {
    BUGLOG.close();
}

#[cfg(target_family = "windows")]
mod sys {
    extern crate core;

    extern crate xrmt_text;

    use core::arch::asm;
    use core::cmp::Ord;
    use core::fmt::{Result, Write};
    use core::hint::unlikely;
    use core::iter::Iterator;
    use core::num::NonZeroUsize;
    use core::ops::Drop;
    use core::option::Option::{self, None, Some};
    use core::ptr::{null, null_mut};
    use core::result::Result::Ok;

    use xrmt_text::utf16_to_str;

    use super::Bugs;

    pub struct Logger {
        f:   Option<NonZeroUsize>,
        con: usize,
    }

    impl Logger {
        #[inline]
        pub fn new() -> Logger {
            let mut b = [0u16; 261];
            let mut i = Logger { f: file(&mut b), con: 0usize };
            let e = unsafe { GetStdHandle(0xFFFFFFF4) }; // 0xFFFFFFF4 - STD_ERROR_HANDLE
            if e > 0 {
                i.con = e as usize;
            }
            if i.f.is_some() {
                let mut v = [0u8; 255];
                let s = utf16_to_str(&mut v, b.as_slice());
                Bugs::write_args(
                    &mut i,
                    format_args!("Bugtrack log init complete! Log file located at \"{s}\""),
                );
            }
            i
        }

        #[inline]
        pub fn flush(&mut self) {
            if self.con > 0 {
                let _ = unsafe { FlushFileBuffers(self.con) };
            }
            if let Some(f) = self.f.as_ref() {
                let _ = unsafe { FlushFileBuffers(f.get()) };
            }
        }
        #[inline]
        pub fn write(&mut self, buf: &[u8]) {
            let mut n = 0u32;
            let i = buf.len().min(0xFFFFFFFF) as u32;
            if self.con > 0 {
                let _ = unsafe { WriteConsoleA(self.con, buf.as_ptr(), i, &mut n, 0) };
            }
            if let Some(f) = &self.f {
                let _ = unsafe { WriteFile(f.get(), buf.as_ptr(), i, &mut n, null_mut()) };
            }
        }
    }

    impl Drop for Logger {
        #[inline]
        fn drop(&mut self) {
            if let Some(v) = &self.f {
                let _ = unsafe { CloseHandle(v.get()) };
            }
        }
    }
    impl Write for Logger {
        #[inline]
        fn write_str(&mut self, v: &str) -> Result {
            self.write(v.as_bytes());
            Ok(())
        }
    }

    #[inline(never)]
    fn pid() -> u32 {
        let i: u32;
        #[cfg(target_arch = "arm")]
        unsafe {
            asm!(
                "push {{r11, lr}}
                 mov    r11, sp
                 mrc    p15, 0x0, r3, cr13, cr0, 0x2
                 ldr     {}, [r3, #0x20]", out(reg) i
            );
        }
        #[cfg(target_arch = "aarch64")]
        unsafe {
            asm!(
                "mov    x8, x18
                 ldr {0:w}, [x8, #0x40]", out(reg) i
            );
        }
        #[cfg(target_arch = "x86")]
        unsafe {
            asm!(
                "mov   eax, FS:[0x18]
                 mov {0:e}, dword ptr [eax+0x20]", out(reg) i
            );
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            asm!(
                "mov   rax, qword ptr GS:[0x30]
                 mov {0:e}, dword ptr [rax+0x40]", out(reg) i
            );
        }
        i
    }
    fn uint(b: &mut [u16], s: u32) -> usize {
        let n = match s {
            0 => {
                // Bounds check happens earlier.
                unsafe { *b.get_unchecked_mut(0) = 0x30 }; // 0
                return 1;
            },
            1__________..=9__________ => 1,
            1_________0..=9_________9 => 2,
            1________00..=9________99 => 3,
            1_______000..=9_______999 => 4,
            1_____0_000..=9_____9_999 => 5,
            1____00_000..=9____99_999 => 6,
            1___000_000..=9___999_999 => 7,
            1_0_000_000..=9_9_999_999 => 8,
            100_000_000..=999_999_999 => 9,
            _ => 10,
        };
        let mut v = s;
        for i in (1..n).rev() {
            let t = v.saturating_div(0xA);
            // Bounds check happens earlier.
            unsafe { *b.get_unchecked_mut(i) = 0x30 + (v - (t * 0xA)) as u16 };
            v = t;
            if v < 0xA {
                break;
            }
        }
        // Bounds check happens earlier.
        unsafe { *b.get_unchecked_mut(0) = 0x30 + (v as u16) };
        n
    }
    fn file(b: &mut [u16]) -> Option<NonZeroUsize> {
        let r = unsafe { GetTempPathW(261, b.as_mut_ptr()) } as usize;
        if r == 0 || r > 261 {
            return None;
        }
        // Don't do bounds checks. K thx.
        // SAFETY: We already KNOW the bounds of the static slice, and we check
        // also beforehand, so the compiler shouldn't bounds check.
        let mut x = (r as usize) + 9;
        if unlikely(x + 24 >= 261) {
            return None;
        }
        unsafe {
            *b.get_unchecked_mut(r) = 0x62;
            *b.get_unchecked_mut(r + 1) = 0x75;
            *b.get_unchecked_mut(r + 2) = 0x67;
            *b.get_unchecked_mut(r + 3) = 0x74;
            *b.get_unchecked_mut(r + 4) = 0x72;
            *b.get_unchecked_mut(r + 5) = 0x61;
            *b.get_unchecked_mut(r + 6) = 0x63;
            *b.get_unchecked_mut(r + 7) = 0x6B;
            *b.get_unchecked_mut(r + 8) = 0x2D;
            x += uint(b.get_unchecked_mut(x..), pid());
        }
        if unlikely(x + 4 >= 261) {
            return None;
        }
        unsafe {
            *b.get_unchecked_mut(x) = 0x2E;
            *b.get_unchecked_mut(x + 1) = 0x6C;
            *b.get_unchecked_mut(x + 2) = 0x6F;
            *b.get_unchecked_mut(x + 3) = 0x67;
            *b.get_unchecked_mut(x + 4) = 0;
        }
        NonZeroUsize::new(unsafe {
            // 0x1      - FILE_SHARE_READ
            // 0x110114 - DELETE | SYNCHRONIZE | FILE_WRITE_ATTRIBUTES | FILE_APPEND_DATA |
            //             FILE_WRITE_EA, FILE_SHARE_READ | FILE_SHARE_WRITE |
            //             FILE_SHARE_DELETE
            // 0x2      - CREATE_ALWAYS
            CreateFileW(b.as_ptr(), 0x110114, 1, null(), 2, 0, 0)
        })
    }

    // Link and load modules to be able to debug issues during sensitive loading
    // areas. Also prevent deadlocks.
    //
    // This also happens in the Stdio lib so we can also use 'bugtrack' in
    // critical areas without deadlocks.
    //
    #[link(name = "kernel32")]
    unsafe extern "system" {
        unsafe fn CloseHandle(h: usize) -> u32;
        unsafe fn GetStdHandle(n: u32) -> isize;
        unsafe fn FlushFileBuffers(h: usize) -> u32;
        unsafe fn GetTempPathW(len: u32, buf: *mut u16) -> u32;
        unsafe fn WriteConsoleA(h: usize, s: *const u8, n: u32, w: *mut u32, r: u32) -> u32;
        unsafe fn WriteFile(h: usize, buf: *const u8, size: u32, written: *mut u32, overlap: *mut u8) -> u32;
        unsafe fn CreateFileW(name: *const u16, access: u32, share: u32, sa: *const u8, mode: u32, attrs: u32, template: usize) -> usize;
    }
}
#[cfg(not(target_family = "windows"))]
mod sys {
    extern crate alloc;
    extern crate core;
    extern crate std;

    use alloc::string::String;
    use core::fmt::{self, Result};
    use core::option::Option::{self, Some};
    use core::result::Result::Ok;
    use std::env::temp_dir;
    use std::fs::{File, OpenOptions};
    use std::io::{stderr, Stderr, Write};
    use std::process::id;

    use super::Bugs;

    pub struct Logger {
        f:   Option<File>,
        con: Stderr,
    }

    impl Logger {
        #[inline]
        pub fn new() -> Logger {
            let p = {
                let mut b = String::new();
                let _ = fmt::Write::write_fmt(&mut b, format_args!("bugtrack-{}.log", id()));
                temp_dir().join(&b)
            };
            let mut i = Logger {
                f:   OpenOptions::new().write(true).append(true).create(true).open(&p).ok(),
                con: stderr(),
            };
            if i.f.is_some() {
                Bugs::write_args(
                    &mut i,
                    format_args!(
                        "Bugtrack log init complete! Log file located at \"{}\"",
                        &p.as_os_str().to_string_lossy()
                    ),
                );
            }
            i
        }

        #[inline]
        pub fn flush(&mut self) {
            let _ = self.con.flush();
            if let Some(f) = self.f.as_mut() {
                let _ = f.flush();
            }
        }
        #[inline]
        pub fn write(&mut self, buf: &[u8]) {
            let _ = self.con.write(buf);
            if let Some(f) = self.f.as_mut() {
                let _ = f.write(buf);
            }
        }
    }

    impl fmt::Write for Logger {
        #[inline]
        fn write_str(&mut self, v: &str) -> Result {
            let _ = self.write(v.as_bytes());
            Ok(())
        }
    }
}
