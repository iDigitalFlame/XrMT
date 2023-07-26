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
#![cfg(windows)]

extern crate core;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::{cmp, ptr};

pub use self::tracker::*;

const MIN: usize = if cfg!(any(
    target_arch = "s390x",
    target_arch = "x86_64",
    target_arch = "aarch64"
)) {
    16
} else {
    8
};

unsafe impl GlobalAlloc for HeapAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(feature = "heap_track")]
        {
            self.track(layout);
        }
        alloc(layout, 0)
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
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
    #[inline]
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
    // TODO(dij): Add ARM/ARM64 code here.
    #[cfg(target_arch = "x86")]
    asm!(
        "mov eax, FS:[0x18]
         mov eax, dword ptr [eax+0x30]
         mov {},  dword ptr [eax+0x18]",  out(reg) h
    );
    #[cfg(target_arch = "x86_64")]
    asm!(
        "mov rax, qword ptr GS:[0x60]
         mov {},  qword ptr [rax+0x30]", out(reg) h
    );
    h
}
#[inline]
unsafe fn alloc(layout: Layout, flags: u32) -> *mut u8 {
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

#[cfg(feature = "heap_track")]
mod tracker {
    extern crate core;

    use core::alloc::Layout;
    use core::clone::Clone;
    use core::default::Default;
    use core::sync::atomic::{AtomicUsize, Ordering};

    pub struct HeapAllocator(AtomicUsize);

    impl HeapAllocator {
        #[inline]
        pub const fn new() -> HeapAllocator {
            HeapAllocator(AtomicUsize::new(0))
        }

        #[inline]
        pub fn total(&self) -> usize {
            self.0.load(Ordering::Relaxed);
        }

        #[inline]
        pub(super) fn track(&self, v: Layout) {
            if v.align() <= super::MIN {
                self.0.fetch_add(v.size(), Ordering::Release);
            } else {
                self.0.fetch_add(v.align() + v.size(), Ordering::Release);
            }
        }
    }

    impl Clone for HeapAllocator {
        #[inline]
        fn clone(&self) -> HeapAllocator {
            HeapAllocator(AtomicUsize::new(self.0.load(Ordering::Relaxed)))
        }
    }
    impl Default for HeapAllocator {
        #[inline]
        fn default() -> HeapAllocator {
            HeapAllocator(AtomicUsize::new(0))
        }
    }
}
#[cfg(not(feature = "heap_track"))]
mod tracker {
    extern crate core;

    use core::clone::Clone;
    use core::default::Default;

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
}

#[cfg(feature = "pie")]
mod inner {
    // Alloc free DLL loader!!
    //
    // This a quasi-copy of what's in loader.rs as we want this to be completely
    // independent of what's going on with the rest of the program. This one mostly
    // skips error checking lol and loads only the 3 functions needed for allocating
    // memory regions.

    extern crate core;

    use core::arch::asm;
    use core::cell::UnsafeCell;
    use core::marker::Sync;
    use core::result::Result::Ok;
    use core::sync::atomic::{AtomicBool, Ordering};
    use core::{mem, slice};

    static LOADED: AtomicBool = AtomicBool::new(false);

    static HEAP_FREE: Function = Function::new();
    static HEAP_ALLOC: Function = Function::new();
    static HEAP_REALLOC: Function = Function::new();

    #[repr(C)]
    struct PEB {
        pad1: u32,
        pad2: [usize; 2],
        ldr:  *mut Loader,
    }
    #[repr(C)]
    struct Loader {
        pad1:        [u8; 8],
        pad2:        [usize; 3],
        module_list: LoaderEntry,
    }
    #[repr(C)]
    struct LoaderEntry {
        pad1:        usize,
        f_link:      *mut LoaderEntry,
        b_link:      *mut LoaderEntry,
        links:       usize,
        dll_base:    usize,
        entry_point: usize,
        image_size:  usize,
        full_name:   LoaderString,
        base_name:   LoaderString,
        flags:       usize,
    }
    #[repr(C)]
    struct LoaderString {
        length:     u16,
        max_length: u16,
        buffer:     *const u16,
    }
    #[repr(C)]
    struct ImageDosHeader {
        magic: u16,
        pad1:  [u8; 56],
        pos:   u32,
    }
    #[repr(C)]
    struct ImageExportDir {
        pad1:                     [u32; 3],
        name:                     u32,
        base:                     u32,
        number_of_functions:      u32,
        number_of_names:          u32,
        address_of_functions:     u32,
        address_of_names:         u32,
        address_of_name_ordinals: u32,
    }
    #[repr(C)]
    struct ImageDataDirectory {
        address: u32,
        size:    u32,
    }
    #[repr(C)]
    struct ImageOptionalHeader32 {
        pad1:                [u8; 92],
        number_of_rva_sizes: u32,
        directory:           [ImageDataDirectory; 16],
    }
    #[repr(C)]
    struct ImageOptionalHeader64 {
        pad1:                [u8; 108],
        number_of_rva_sizes: u32,
        directory:           [ImageDataDirectory; 16],
    }
    struct Function(UnsafeCell<usize>);

    unsafe impl Sync for Function {}

    impl Function {
        #[inline]
        const fn new() -> Function {
            Function(UnsafeCell::new(0))
        }

        #[inline]
        fn set(&self, a: usize) {
            unsafe { *self.0.get() = a }
        }
        #[inline]
        fn addr(&self) -> *const () {
            (unsafe { *self.0.get() }) as *const ()
        }
    }

    #[inline]
    pub(super) unsafe fn heap_free(heap: usize, flags: u32, addr: usize) -> u32 {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        mem::transmute::<*const (), extern "stdcall" fn(usize, u32, usize) -> u32>(HEAP_FREE.addr())(heap, flags, addr)
    }
    #[inline]
    pub(super) unsafe fn heap_alloc(heap: usize, flags: u32, size: usize) -> usize {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        mem::transmute::<*const (), extern "stdcall" fn(usize, u32, usize) -> usize>(HEAP_ALLOC.addr())(heap, flags, size)
    }
    #[inline]
    pub(super) unsafe fn heap_realloc(heap: usize, flags: u32, addr: usize, new_size: usize) -> usize {
        if LOADED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init() // Init before running.
        }
        mem::transmute::<*const (), extern "stdcall" fn(usize, u32, usize, usize) -> usize>(HEAP_REALLOC.addr())(heap, flags, addr, new_size)
    }

    #[inline]
    unsafe fn init() {
        // Let's inspect the PEB for ntdll, since we know it's there.
        let mut next = (*(*get_peb()).ldr).module_list.f_link;
        loop {
            // ntdll.dll
            if hash_str(&(*next).base_name) == 0xA9ACADD3 {
                // Load ntdll using our DLL loader.
                load((*next).dll_base);
                break;
            }
            if (*next).f_link.is_null() || (*(*next).f_link).dll_base == 0 {
                break;
            }
            next = (*next).f_link;
        }
    }
    unsafe fn load(h: usize) {
        // There's not really any error checking here as if something fails, we're
        // fucked anyway (since this is the allocator), so we'll just let rust
        // try to load a nil entry and C05. *shrug*
        let p = (*(h as *const ImageDosHeader)).pos as usize + 0x18;
        let i = match *((h + p) as *const u16) {
            0x20B => &(&*((h + p) as *const ImageOptionalHeader64)).directory[0],
            _ => &(&*((h + p) as *const ImageOptionalHeader32)).directory[0],
        };
        let e = (h + i.address as usize) as *const ImageExportDir;
        let (v, f, o) = (
            h + (*e).address_of_names as usize,
            h + (*e).address_of_functions as usize,
            h + (*e).address_of_name_ordinals as usize,
        );
        let mut c = 0;
        for x in 0..(*e).number_of_names {
            // We don't need to worry about forwarded functions here, as ntdll
            // shouldn't have any.
            let a = h + *((f + ((*((o + (x * 2) as usize) as *const u16)) as usize * 4) as usize) as *const u32) as usize;
            match hash(h + *((v + (x * 4) as usize) as *const u32) as usize) {
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
    #[inline]
    unsafe fn hash(a: usize) -> u32 {
        let b = slice::from_raw_parts(a as *const u8, 256);
        let mut h: u32 = 0x811C9DC5;
        for i in 0..256 {
            if b[i] == 0 {
                break;
            }
            h = h.wrapping_mul(0x1000193);
            h ^= b[i] as u32;
        }
        h
    }
    #[inline(never)]
    unsafe fn get_peb() -> *const PEB {
        let p: *mut PEB;
        // TODO(dij): Add ARM/ARM64 code here.
        #[cfg(target_arch = "x86")]
        asm!(
            "mov eax, FS:[0x18]
             mov {},  dword ptr [eax+0x30]", out(reg) p
        );
        #[cfg(target_arch = "x86_64")]
        asm!(
            "mov rax, qword ptr GS:[0x30]
             mov {},  qword ptr [rax+0x60]", out(reg) p
        );
        p
    }
    #[inline]
    unsafe fn hash_str(v: &LoaderString) -> u32 {
        let mut h: u32 = 0x811C9DC5;
        let s = slice::from_raw_parts(v.buffer, (v.length / 2) as usize);
        for i in s {
            h = h.wrapping_mul(0x1000193);
            h ^= match *i as u8 {
                b'A'..=b'Z' => *i + 0x20,
                _ => *i,
            } as u32;
        }
        h
    }
}
#[cfg(not(feature = "pie"))]
mod inner {
    /*
    Here's the import entry for using ntdll.dll instead. The functions are exactly
    the same and take the same args, so they can be drop-in replacements.

    We're not using these as it's super sussy to import ntdll.dll directly (usually).
    Tools like ProcessHacker also don't like it and flag the binary as being packed
    when it's not (it just passes a heuristic test).

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
