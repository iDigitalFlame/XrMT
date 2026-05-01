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

use core::alloc::{GlobalAlloc, Layout};
use core::clone::Clone;
use core::cmp::Ord;
use core::default::Default;
use core::marker::Copy;
use core::ptr::{copy_nonoverlapping, null_mut, read, write, NonNull};

use crate::functions::GetProcessHeap;

#[cfg(all(
    not(target_arch = "arm"),
    not(target_arch = "x86"),
    not(target_arch = "s390x"),
    not(target_arch = "x86_64"),
    not(target_arch = "aarch64"),
    not(target_arch = "arm64ec"),
))]
const MIN: usize = 4; // Other Min Allocation.
#[cfg(any(target_arch = "arm", target_arch = "x86"))]
const MIN: usize = 8; // 32bit Min Allocation.
#[cfg(any(
    target_arch = "s390x",
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "arm64ec"
))]
const MIN: usize = 16; // 64bit Min Allocation.

/// Heap Allocator!!
///
/// This is similar to the stdlib Global allocator. The only difference is that
/// there is more options in it, and we use the direct ProcessHeap instead of
/// creating our own one.
///
/// If the "pie" feature is used, this will NOT link kernel32.dll and will load
/// the Rtl* heap functions from ntdll.dll via PEB loading, which if used with
/// "nostd" and "nostart" (core and non-gnu) there will be NO imports in the
/// resulting binary. (crazy shit right??!).
///
/// To use this allocator as the default one, use this line in your 'main.rs' or
/// whatever entry file you're using.
///
/// ```
/// #[global_allocator]
/// static GLOBAL: xrmt_winapi::HeapAllocator = xrmt_winapi::HeapAllocator::new();
/// ```
pub struct HeapAllocator;

impl HeapAllocator {
    #[inline]
    pub const fn new() -> HeapAllocator {
        HeapAllocator {}
    }
}

impl Copy for HeapAllocator {}
impl Clone for HeapAllocator {
    #[inline]
    fn clone(&self) -> HeapAllocator {
        HeapAllocator {}
    }
}
impl Default for HeapAllocator {
    #[inline]
    fn default() -> HeapAllocator {
        HeapAllocator {}
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        alloc(layout, 0)
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        loader::heap_free(
            *GetProcessHeap(),
            0,
            if layout.align() <= MIN {
                ptr as usize
            } else {
                unsafe { read((ptr as *mut usize).sub(1)) }
            },
        );
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        alloc(layout, 0x8)
    }
    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MIN {
            return loader::heap_realloc(*GetProcessHeap(), 0, ptr as usize, new_size) as *mut u8;
        }
        unsafe {
            let v = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
            if !v.is_null() {
                copy_nonoverlapping(ptr, v, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            v
        }
    }
}

fn alloc(layout: Layout, f: u32) -> *mut u8 {
    if layout.size() == 0 {
        return NonNull::slice_from_raw_parts(layout.dangling(), 0).as_ptr() as *mut u8;
    }
    if layout.align() <= MIN {
        return loader::heap_alloc(*GetProcessHeap(), f, layout.size()) as *mut u8;
    }
    let v = loader::heap_alloc(*GetProcessHeap(), f, layout.align() + layout.size());
    if v == 0 {
        return null_mut();
    }
    let r = v + (layout.align() - (v & layout.align() - 1));
    unsafe { write((r as *mut usize).sub(1), v) };
    r as *mut u8
}

/// Alloc free DLL loader!!
#[cfg(feature = "pie")]
mod loader {
    extern crate core;

    use core::cell::UnsafeCell;
    use core::iter::Iterator;
    use core::marker::Sync;
    use core::ops::Deref;
    use core::result::Result::{Err, Ok};
    use core::sync::atomic::{AtomicBool, Ordering};

    use crate::functions::GetCurrentProcessPEB;
    use crate::loader_error;
    use crate::structs::ImageExportsIter;

    static LOADED: AtomicBool = AtomicBool::new(false);

    static HEAP_FREE: Function = Function::new();
    static HEAP_ALLOC: Function = Function::new();
    static HEAP_REALLOC: Function = Function::new();

    struct Function(UnsafeCell<usize>);

    impl Function {
        #[inline]
        const fn new() -> Function {
            Function(UnsafeCell::new(0usize))
        }

        #[inline]
        fn set(&self, a: usize) {
            unsafe { *self.0.get() = a }
        }
    }

    impl Deref for Function {
        type Target = usize;

        #[inline]
        fn deref(&self) -> &usize {
            unsafe { &*self.0.get() }
        }
    }

    unsafe impl Sync for Function {}

    #[inline]
    pub fn heap_free(h: usize, f: u32, a: usize) -> u32 {
        check();
        crate::syscall!(
            HEAP_FREE,
            (usize, u32, usize) -> u32,
            h,
            f,
            a
        )
    }
    #[inline]
    pub fn heap_alloc(h: usize, f: u32, n: usize) -> usize {
        check();
        crate::syscall!(
            HEAP_ALLOC,
            (usize, u32, usize) -> usize,
            h,
            f,
            n
        )
    }
    #[inline]
    pub fn heap_realloc(h: usize, f: u32, a: usize, n: usize) -> usize {
        check();
        crate::syscall!(
            HEAP_REALLOC,
           (usize, u32, usize, usize) -> usize,
            h,
            f,
            a,
            n
        )
    }

    #[inline]
    fn init() {
        // SAFETY: "ntdll.dll" is ALWAYS loaded in every process.
        let h = unsafe {
            GetCurrentProcessPEB()
                .modules()
                .find(|i| i.name_base.hash_fnv32() == 0xA9ACADD3)
                .unwrap_unchecked()
                .dll_base
        };
        let i = match ImageExportsIter::new(h) {
            Ok(v) => v,
            Err(_) => loader_error(),
        };
        let mut c = 0u8;
        for e in i {
            if c >= 3 {
                // We only need 3 functions, so we'll bail once we got em all.
                break;
            }
            match e.hash() {
                /* RtlFreeHeap */ 0xBC880A2D => {
                    HEAP_FREE.set(e.address);
                    c += 1;
                },
                /* RtlAllocateHeap */ 0x50AA445E => {
                    HEAP_ALLOC.set(e.address);
                    c += 1;
                },
                /* RtlReAllocateHeap */ 0xA51D1975 => {
                    HEAP_REALLOC.set(e.address);
                    c += 1;
                },
                _ => (),
            }
        }
    }
    #[inline]
    fn check() {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init();
        }
    }
}
/// Here's the import entry for using kernel32.dll instead. The functions
/// are exactly the same and take the same args, so they can be drop-in
/// replacements.
///
/// We're not using these as it's super sussy to import ntdll.dll directly
/// (usually). Tools like ProcessHacker also don't like it and flag the
/// binary as being packed when it's not (it just passes a heuristic
/// test).
///
/// You can use the below block to get the exports from ntdll.dll instead, but
/// it is similar to the PIE version.
///
/// ```
/// #[link(name = "ntdll")]
/// unsafe extern "system" {
///     unsafe fn RtlFreeHeap(heap: usize, flags: u32, address: usize) -> u32;
///     unsafe fn RtlAllocateHeap(heap: usize, flags: u32, size: usize) -> usize;
///     unsafe fn RtlReAllocateHeap(heap: usize, flags: u32, address: usize, new_size: usize) -> usize;
/// }
/// ```
#[cfg(not(feature = "pie"))]
mod loader {
    #[inline]
    pub fn heap_free(h: usize, f: u32, a: usize) -> u32 {
        unsafe { HeapFree(h, f, a) }
    }
    #[inline]
    pub fn heap_alloc(h: usize, f: u32, n: usize) -> usize {
        unsafe { HeapAlloc(h, f, n) }
    }
    #[inline]
    pub fn heap_realloc(h: usize, f: u32, a: usize, n: usize) -> usize {
        unsafe { HeapReAlloc(h, f, a, n) }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        unsafe fn HeapFree(heap: usize, flags: u32, addr: usize) -> u32;
        unsafe fn HeapAlloc(heap: usize, flags: u32, size: usize) -> usize;
        unsafe fn HeapReAlloc(heap: usize, flags: u32, addr: usize, new_size: usize) -> usize;
    }
}
