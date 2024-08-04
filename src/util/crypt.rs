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

pub use self::inner::unload;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub(crate) use self::inner::get_or;

#[cfg(feature = "crypt")]
mod inner {
    use core::cell::UnsafeCell;
    use core::ptr;
    use core::str::from_utf8_unchecked;
    use core::sync::atomic::{AtomicBool, Ordering};

    use crate::prelude::*;

    static MAPPER: Mapper = Mapper::new();

    struct Mapper<'a> {
        map:     UnsafeCell<[&'a str; 255]>,
        ready:   AtomicBool,
        backing: UnsafeCell<Vec<u8>>,
    }

    impl<'a> Mapper<'a> {
        #[inline]
        const fn new() -> Mapper<'a> {
            Mapper {
                map:     UnsafeCell::new([unsafe { from_utf8_unchecked(&[]) }; 255]),
                ready:   AtomicBool::new(false),
                backing: UnsafeCell::new(Vec::new()),
            }
        }

        fn init(&self) {
            let src = core::include_bytes!(core::env!("CRYPT_DB"));
            if src.len() < 65 {
                // Invalid or no key, bail.
                abort();
            }
            // Copy non-key material to our backing buffer.
            unsafe {
                (*self.backing.get()).reserve_exact(src.len() - 64);
                (*self.backing.get()).extend_from_slice(&src[64..]);
            }
            // [0 :64] - Key (64b)
            // [64: N] - Entries
            //           Each broken up by zero.
            let b = unsafe { (*self.backing.get()).as_mut_slice() };
            // Pull mutable backing buffer and decode from src key.
            for i in 0..b.len() {
                b[i] = b[i] ^ src[i % 64];
            }
            let (mut s, mut n) = (0, 0);
            let e = unsafe { &mut *self.map.get() };
            // Extract the strings as pointers and push them into the map.
            for i in 0..b.len() {
                if b[i] != 0 {
                    continue;
                }
                if i - s > 0 {
                    e[n] = unsafe { from_utf8_unchecked(&b[s..i]) }
                }
                (s, n) = (i + 1, n + 1);
            }
            bugtrack!(
                "util::crypt::Mapper.init(): Crypt loader loaded {n} entries from {} bytes.",
                src.len()
            )
        }
        #[inline]
        fn check(&self) {
            if self
                .ready
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.init()
            }
        }
        #[inline]
        fn destroy(&self) {
            unsafe {
                *self.map.get() = [from_utf8_unchecked(&[]); 255];
                ptr::drop_in_place(&mut *self.backing.get())
            }
        }
        #[inline]
        fn get(&self, index: u8) -> &'a str {
            self.check();
            // SAFETY: Fast check, no bounds checking.
            unsafe { (*self.map.get()).get_unchecked(index as usize) }
        }
    }

    unsafe impl Sync for Mapper<'_> {}
    unsafe impl Send for Mapper<'_> {}

    #[inline]
    pub fn init() {
        MAPPER.check()
    }
    #[inline]
    pub fn unload() {
        if !MAPPER.ready.load(Ordering::Relaxed) {
            return;
        }
        MAPPER.destroy();
    }

    #[inline(always)]
    pub(crate) fn get_or(index: u8, _v: &str) -> &str {
        MAPPER.get(index)
    }
}
#[cfg(not(feature = "crypt"))]
mod inner {
    #[inline(always)]
    pub fn unload() {}

    #[inline(always)]
    pub(crate) fn get_or(_index: u8, v: &str) -> &str {
        v
    }
}
