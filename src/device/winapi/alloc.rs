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

// Heap Allocator!!
//
// This is similar to the stdlib Global allocator. The only difference is that
// there is more options in it, and we use the direct ProcessHeap instead of
// creating our own one.
//
// If the "pie" feature is used, this will NOT link kernel32.dll and will load
// the Rtl* heap functions from ntdll.dll via PEB loading, which if used with
// "nostd" and "nostart" (core and non-gnu) there will be NO imports in the
// resulting binary. (crazy shit right??!).
//
// To use this allocator as the default one, use this line in your 'main.rs' or
// whatever entry file you're using.
//
//   #[global_allocator]
//   static GLOBAL: winapi::HeapAllocator = winapi::HeapAllocator::new();
//

#![no_implicit_prelude]
#![cfg(target_family = "windows")]

extern crate core;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::clone::Clone;
use core::default::Default;
use core::ptr::NonNull;
use core::{cmp, ptr};

core::cfg_match! {
    cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm64ec", target_arch = "loongarch64", target_arch = "mips64", target_arch = "mips64r6", target_arch = "s390x", target_arch = "sparc64", target_arch = "riscv64")) => {
        const fn _align() -> usize {
            16
        }
    }
    cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "csky", target_arch = "mips", target_arch = "mips32r6", target_arch = "powerpc", target_arch = "powerpc64", target_arch = "sparc", target_arch = "riscv32")) => {
        const fn _align() -> usize {
            8
        }
    }
    _ => {
        const fn _align() -> usize {
            4
        }
    }
}

const MIN: usize = _align();

pub struct HeapAllocator;

impl HeapAllocator {
    #[inline]
    pub const fn new() -> HeapAllocator {
        HeapAllocator {}
    }
}

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
        #[cfg(feature = "heap_track")]
        {
            bugtrack!(
                "(HeapAllocator).alloc(): Allocation of memory size={}, align={}.",
                layout.size(),
                layout.align()
            );
        }
        alloc(layout, 0)
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[cfg(feature = "heap_track")]
        {
            bugtrack!(
                "(HeapAllocator).alloc(): Deallocation of memory size={}, align={} at address {:X}.",
                layout.size(),
                layout.align(),
                ptr as usize
            );
        }
        inner::heap_free(
            process_heap(),
            0,
            if layout.align() <= MIN {
                ptr as usize
            } else {
                ptr::read((ptr as *mut usize).sub(1))
            },
        );
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        alloc(layout, 0x8)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let h = process_heap();
        if layout.align() <= MIN {
            inner::heap_realloc(h, 0, ptr as usize, new_size) as *mut u8
        } else {
            let n = Layout::from_size_align_unchecked(new_size, layout.align());
            let a = self.alloc(n);
            if !a.is_null() {
                let s = cmp::min(layout.size(), new_size);
                ptr::copy_nonoverlapping(ptr, a, s);
                self.dealloc(ptr, layout);
            }
            a
        }
    }
}

// IF WE INLINE THIS FUNCTION THE PROGRAM CRASHES!!!
// Plz don't inline k thx.
#[inline(never)]
unsafe fn process_heap() -> usize {
    let mut h;
    // NOTE(dij): I'm not 100% sure if this works correctly. For the most
    //            part Windows or ARM has a very limited set of machines
    //            _currently_ supported for ARM and is focusing more on
    //            AARCH64.
    //            Also, why is ARM opcode so different than AARCH64?
    #[cfg(target_arch = "arm")]
    asm!(
        "push {{rll, lr}}
         mov    rll, sp
         mrc    p15, 0x0, r3, cr13, cr0, 0x2
         ldr     r0, [r3, #0x30]
         ldr     {}, [r0, #0x18]", out(reg) h
    );
    // NOTE(dij): This is some guesstimating on my part based on the other
    //            ASM samples I found. *shrug*
    #[cfg(target_arch = "aarch64")]
    asm!(
        "mov x8, x18
         ldr x8, [x8, #0x60]
         ldr {}, [x8, #0x30]", out(reg) h
    );
    #[cfg(target_arch = "x86")]
    asm!(
        "mov eax, FS:[0x18]
         mov eax, dword ptr [eax+0x30]
         mov {},  dword ptr [eax+0x18]", out(reg) h
    );
    #[cfg(target_arch = "x86_64")]
    asm!(
        "mov rax, qword ptr GS:[0x60]
         mov {},  qword ptr [rax+0x30]", out(reg) h
    );
    h
}
unsafe fn alloc(layout: Layout, flags: u32) -> *mut u8 {
    if layout.size() == 0 {
        return NonNull::slice_from_raw_parts(layout.dangling(), 0).as_ptr() as *mut u8;
    }
    let h = process_heap();
    if layout.align() <= MIN {
        return inner::heap_alloc(h, flags, layout.size()) as *mut u8;
    }
    let s = layout.align() + layout.size();
    let a = inner::heap_alloc(h, flags, s);
    if a == 0 {
        return ptr::null_mut();
    }
    let v = a + (layout.align() - (a & (layout.align() - 1)));
    ptr::write((v as *mut usize).sub(1), a);
    v as *mut u8
}

#[cfg(all(feature = "pie", feature = "heap_track"))]
compile_error!("Cannot use 'pie' and 'heap_track' at the same time!");

#[cfg(feature = "pie")]
mod inner {
    // Alloc free DLL loader!!
    //
    // This a quasi-copy of what's in loader.rs as we want this to be completely
    // independent of what's going on with the rest of the program. This one mostly
    // skips error checking lol and loads only the 3 functions needed for allocating
    // memory regions.

    extern crate core;

    use core::iter::Iterator;
    use core::option::Option::{None, Some};
    use core::sync::atomic::{AtomicBool, Ordering};

    use crate::device::winapi::{self, Function, Handle, ImageDosHeader, ImageExportDir, ImageOptionalHeader32, ImageOptionalHeader64};

    static LOADED: AtomicBool = AtomicBool::new(false);

    static HEAP_FREE: Function = Function::new();
    static HEAP_ALLOC: Function = Function::new();
    static HEAP_REALLOC: Function = Function::new();

    #[inline]
    pub(super) unsafe fn heap_free(heap: usize, flags: u32, addr: usize) -> u32 {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        winapi::syscall!(
            *HEAP_FREE,
            extern "stdcall" fn(usize, u32, usize) -> u32,
            heap,
            flags,
            addr
        )
    }
    #[inline]
    pub(super) unsafe fn heap_alloc(heap: usize, flags: u32, size: usize) -> usize {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        winapi::syscall!(
            *HEAP_ALLOC,
            extern "stdcall" fn(usize, u32, usize) -> usize,
            heap,
            flags,
            size
        )
    }
    #[inline]
    pub(super) unsafe fn heap_realloc(heap: usize, flags: u32, addr: usize, new_size: usize) -> usize {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        winapi::syscall!(
            *HEAP_REALLOC,
            extern "stdcall" fn(usize, u32, usize, usize) -> usize,
            heap,
            flags,
            addr,
            new_size
        )
    }

    #[inline]
    unsafe fn init() {
        let n = winapi::GetCurrentProcessPEB()
            .load_list()
            .iter()
            .find(|i| i.base_name.hash() == 0xA9ACADD3);
        match n {
            None => core::unreachable!(),
            Some(d) => load(d.dll_base),
        }
    }
    unsafe fn load(h: Handle) {
        // There's not really any error checking here as if something fails, we're
        // fucked anyway (since this is the allocator), so we'll just let rust
        // try to load a nil entry and C05. *shrug*
        let p = (&*(h.0 as *const ImageDosHeader)).pos as usize + 0x18;
        let i = match *((h + p) as *const u16) {
            0x20B => &(&*((h + p) as *const ImageOptionalHeader64)).directory[0],
            _ => &(&*((h + p) as *const ImageOptionalHeader32)).directory[0],
        };
        let e = &*((h + i.address as usize) as *const ImageExportDir);
        let (v, f, o) = (
            h + e.address_of_names as usize,
            h + e.address_of_functions as usize,
            h + e.address_of_name_ordinals as usize,
        );
        let mut c = 0;
        for x in 0..e.number_of_names {
            // We don't need to worry about forwarded functions here, as ntdll
            // shouldn't have any.
            let a = h + *((f + ((*((o + (x * 2) as usize) as *const u16)) as usize * 4) as usize) as *const u32) as usize;
            match Function::hash(h + *((v + (x * 4) as usize) as *const u32) as usize) {
                /* RtlFreeHeap */ 0xBC880A2D => {
                    HEAP_FREE.set(a);
                    c += 1;
                },
                /* RtlAllocateHeap */ 0x50AA445E => {
                    HEAP_ALLOC.set(a);
                    c += 1;
                },
                /* RtlReAllocateHeap */ 0xA51D1975 => {
                    HEAP_REALLOC.set(a);
                    c += 1;
                },
                _ => (),
            }
            if c >= 3 {
                // We only need 3 functions, so we'll bail once we got em all.
                break;
            }
        }
    }
}
#[cfg(not(feature = "pie"))]
mod inner {
    /*
    Here's the import entry for using kernel32.dll instead. The functions are exactly
    the same and take the same args, so they can be drop-in replacements.

    We're not using these as it's super sussy to import ntdll.dll directly (usually).
    Tools like ProcessHacker also don't like it and flag the binary as being packed
    when it's not (it just passes a heuristic test).

    You can uncomment the below block to use ntdll.dll instead, but it is similar
    to the PIE version.

    #[link(name = "ntdll")]
    extern "stdcall" {
        fn RtlFreeHeap(heap: usize, flags: u32, address: usize) -> u32;
        fn RtlAllocateHeap(heap: usize, flags: u32, size: usize) -> usize;
        fn RtlReAllocateHeap(heap: usize, flags: u32, address: usize, new_size: usize) -> usize;
    }
    */

    #[link(name = "kernel32")]
    extern "stdcall" {
        fn HeapFree(heap: usize, flags: u32, addr: usize) -> u32;
        fn HeapAlloc(heap: usize, flags: u32, size: usize) -> usize;
        fn HeapReAlloc(heap: usize, flags: u32, addr: usize, new_size: usize) -> usize;
    }

    #[inline]
    pub(super) unsafe fn heap_free(heap: usize, flags: u32, addr: usize) -> u32 {
        HeapFree(heap, flags, addr)
    }
    #[inline]
    pub(super) unsafe fn heap_alloc(heap: usize, flags: u32, size: usize) -> usize {
        HeapAlloc(heap, flags, size)
    }
    #[inline]
    pub(super) unsafe fn heap_realloc(heap: usize, flags: u32, addr: usize, new_size: usize) -> usize {
        HeapReAlloc(heap, flags, addr, new_size)
    }
}
