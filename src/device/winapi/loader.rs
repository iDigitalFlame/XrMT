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

use alloc::collections::BTreeMap;
use core::cell::UnsafeCell;
use core::intrinsics::unlikely;
use core::matches;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::slice::from_raw_parts;

use crate::device::winapi::{self, AnsiString, Handle, UnicodeString, WCharPtr, Win32Error, Win32Result};
use crate::prelude::*;
use crate::sync::{Lazy, Once};
use crate::{ignore_error, some_or_continue};

pub(super) mod advapi32;
pub(super) mod amsi;
pub(super) mod crypt32;
pub(super) mod dbghelp;
pub(super) mod gdi32;
pub(super) mod iphlpapi;
pub(super) mod kernel32;
mod mappings;
pub(super) mod netapi32;
pub(super) mod ntdll;
pub(super) mod psapi;
pub(super) mod user32;
pub(super) mod userenv;
pub(super) mod winhttp;
pub(super) mod winsock;
pub(super) mod wtsapi32;

pub(super) use mappings::*;

// All hashes are in the FNV format.
/*
def fnv(v):
    h = 2166136261
    for n in v:
        h *= 16777619
        h ^= ord(n)
        h = h&0xFFFFFFFF
    return "0x" + hex(h).upper()[2:]
*/

#[repr(C)]
pub(super) struct ImageNtHeader {
    pub(super) signature: u32,
    pub(super) file:      ImageFileHeader,
}
#[repr(C)]
pub(super) struct ImageDosHeader {
    pub(super) magic: u16,
    pad1:             [u8; 56],
    pub(super) pos:   u32,
}
#[repr(C)]
pub(super) struct ImageExportDir {
    pad1:                                [u32; 3],
    pub(super) name:                     u32,
    pub(super) base:                     u32,
    pub(super) number_of_functions:      u32,
    pub(super) number_of_names:          u32,
    pub(super) address_of_functions:     u32,
    pub(super) address_of_names:         u32,
    pub(super) address_of_name_ordinals: u32,
}
#[repr(C)]
pub(super) struct ImageFileHeader {
    pub(super) machine:         u16,
    pub(super) section_size:    u16,
    pad1:                       [u32; 3],
    pub(super) opt_header_size: u16,
    pub(super) characteristics: u16,
}
#[repr(C)]
pub(super) struct ImageDataDirectory {
    pub(super) address: u32,
    pub(super) size:    u32,
}
#[repr(C)]
pub(super) struct ImageOptionalHeader32 {
    pad1:                           [u8; 92],
    pub(super) number_of_rva_sizes: u32,
    pub(super) directory:           [ImageDataDirectory; 16],
}
#[repr(C)]
pub(super) struct ImageOptionalHeader64 {
    pad1:                           [u8; 108],
    pub(super) number_of_rva_sizes: u32,
    pub(super) directory:           [ImageDataDirectory; 16],
}
pub(crate) struct Function(UnsafeCell<usize>);

enum LoadType {
    Linked, // It's statically linked into the binary, don't increase load_count.
    // Don't try to free it either.
    Dynamic, // It was dynamically loaded by the host binary, increase the load_count.
    // Don't try to free it either.
    Imported, // This was loaded by us using LoadLibrary. Free this when done.
}

struct DLL<'a> {
    how:      LoadType,
    func:     BTreeMap<u32, &'a Function>,
    handle:   Handle,
    forwards: [Handle; 5],
}
struct Loader<'a, F = fn(&mut DLL<'a>)> {
    dll:  UnsafeCell<DLL<'a>>,
    once: Once,
    lazy: Lazy,
    func: UnsafeCell<ManuallyDrop<F>>,
}

impl Function {
    #[inline]
    pub(super) const fn new() -> Function {
        Function(UnsafeCell::new(0))
    }

    #[inline]
    pub fn address(&self) -> usize {
        unsafe { *self.0.get() }
    }
    #[inline]
    pub fn is_loaded(&self) -> bool {
        unsafe { *(self.0.get()) != 0 }
    }

    #[inline]
    pub(super) unsafe fn set(&self, a: usize) {
        *self.0.get() = a
    }

    #[inline]
    pub(super) unsafe fn hash(a: usize) -> u32 {
        let b = from_raw_parts(a as *const u8, 256);
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
}
impl<'a> DLL<'a> {
    #[inline]
    const fn new() -> DLL<'a> {
        DLL {
            how:      LoadType::Linked,
            func:     BTreeMap::new(),
            handle:   Handle(0),
            forwards: [Handle::INVALID; 5],
        }
    }

    #[inline]
    fn is_loaded(&self) -> bool {
        !self.handle.is_invalid()
    }
    fn free(&mut self) -> Win32Result<()> {
        if !self.func.is_empty() {
            self.func.clear()
        }
        if self.handle.is_invalid() {
            return Ok(());
        }
        for h in self.forwards.iter_mut() {
            if h.is_invalid() {
                continue;
            }
            bugtrack!("(DLL).free(): Freeing forwarded DLL {:X}", h);
            ignore_error!(winapi::FreeLibrary(*h));
            h.0 = 0;
        }
        match self.how {
            LoadType::Linked => {
                bugtrack!(
                    "(DLL).free(): Not freeing DLL {:X} ({}).",
                    self.handle,
                    self.how
                );
            }, // Don't free.
            LoadType::Dynamic => unsafe { self.unload() }, // Don't free, but decrement.
            LoadType::Imported => {
                bugtrack!("(DLL).free(): Executing 'FreeLibrary' on {:X}", self.handle);
                winapi::FreeLibrary(self.handle)?
            },
        }
        Ok(())
    }
    #[inline]
    fn proc(&mut self, f: &'a Function, h: u32) {
        // TODO(dij): Make a 'sys_proc' one for syscalls for funcmap.
        if f.is_loaded() {
            return;
        }
        self.func.insert(h, f);
    }
    #[inline]
    fn load(&mut self, h: Handle, t: LoadType) -> Win32Result<()> {
        if self.handle.is_invalid() {
            self.handle = h;
        }
        self.how = t;
        unsafe { self.load_functions() }
    }
    #[inline]
    fn load_if_name(&mut self, name: fn() -> &'a str) -> Win32Result<()> {
        // NOTE(dij): The name arg is a function so we can wait to see if the
        //            DLL is already loaded to lazily eval and load crypt if its
        //            being used.
        if self.handle.is_invalid() {
            unsafe { self.handle = load_dll_raw(name().as_bytes())? }
            self.how = LoadType::Imported; // Mark as manually imported
        }
        unsafe { self.load_functions() }
    }

    unsafe fn unload(&self) {
        let d = winapi::GetCurrentProcessPEB()
            .load_list()
            .iter()
            .find(|e| e.load_count > 0 && e.dll_base == self.handle && e.base_name.hash() != 0xA9ACADD3);
        if let Some(e) = d {
            bugtrack!(
                "(DLL).free(): Decrementing PEB load_count on {:X}",
                self.handle
            );
            e.load_count -= 1;
        }
    }
    unsafe fn load_functions(&mut self) -> Win32Result<()> {
        if self.func.is_empty() {
            return Ok(());
        }
        if self.handle.is_invalid() {
            return Err(Win32Error::InvalidHandle);
        }
        let d = &*(self.handle.0 as *const ImageDosHeader);
        if d.magic != 0x5A4D {
            return Err(Win32Error::InvalidHeader);
        }
        let n = &*((self.handle + d.pos as usize) as *const ImageNtHeader);
        match 0 {
            _ if n.signature != 0x00004550 => return Err(Win32Error::InvalidHeader),
            _ if n.file.characteristics & 0x2000 == 0 => return Err(Win32Error::InvalidDLL),
            _ => (),
        }
        match n.file.machine {
            0 | 0x14C | 0x1C4 | 0xAA64 | 0x8664 => (),
            _ => return Err(Win32Error::InvalidImage),
        }
        let p = d.pos as usize + 0x18;
        let i = match *((self.handle + p) as *const u16) {
            0x20B => &(&*((self.handle + p) as *const ImageOptionalHeader64)).directory[0],
            _ => &(&*((self.handle + p) as *const ImageOptionalHeader32)).directory[0],
        };
        if i.size == 0 || i.address == 0 {
            return Err(Win32Error::InvalidAddress);
        }
        let e = &*((self.handle + i.address as usize) as *const ImageExportDir);
        let (v, f, o) = (
            self.handle + e.address_of_names as usize,
            self.handle + e.address_of_functions as usize,
            self.handle + e.address_of_name_ordinals as usize,
        );
        let m = self.handle + i.address as usize + i.size as usize;
        for x in 0..(*e).number_of_names {
            let h = Function::hash(self.handle + *((v + (x * 4) as usize) as *const u32) as usize);
            let z = some_or_continue!(self.func.get(&h));
            let a = self.handle + *((f + ((*((o + (x * 2) as usize) as *const u16)) as usize * 4) as usize) as *const u32) as usize;
            if a < m && a > f {
                z.set(self.forward(a)?);
            } else {
                z.set(a);
            }
        }
        self.func.clear();
        Ok(())
    }
    unsafe fn forward(&mut self, a: usize) -> Win32Result<usize> {
        let buf = from_raw_parts(a as *const u8, 256);
        if buf[0] == 0 {
            return Err(Win32Error::InvalidForward);
        }
        let mut i = buf.iter();
        // NOTE(dij): Since one is after the other, this is ok! (It makes us not
        //            have to do checks on length :D).
        let n = i.position(|v| *v == b'.').ok_or(Win32Error::InvalidForward)?;
        let e = i.position(|v| *v == 0).ok_or(Win32Error::InvalidForward)?;
        if e <= n {
            // NOTE(dij): Realistically this will only happen if they mark the
            //            same position and this is mostly a sanity check.
            return Err(Win32Error::InvalidForward);
        }
        let mut d = [0u8; 260];
        d[0..n].copy_from_slice(&buf[0..n]);
        d[n + 4] = 0;
        d[n + 3] = b'l';
        d[n + 2] = b'l';
        d[n + 1] = b'd';
        d[n] = b'.';
        bugtrack!(
            "(DLL).forward(): Found Forwarded Function on {:X} pointing to '{}'.",
            self.handle,
            unsafe { core::str::from_utf8_unchecked(&d[0..n + 4]) }
        );
        let h = load_dll_raw(&d[0..n + 4])?;
        d[0..e].copy_from_slice(&buf[n + 1..e + n + 1]);
        bugtrack!(
            "(DLL).forward(): Loading Forwarded Function '{}'..",
            unsafe { core::str::from_utf8_unchecked(&d[0..e]) }
        );
        d[e + 1] = 0; // Add NULL.
        let x = match load_func_raw(h, &d[0..e + 1]) {
            Ok(x) => x,
            Err(e) => {
                winapi::close_handle(h);
                return Err(e);
            },
        };
        bugtrack!(
            "(DLL).forward(): Loaded forward DLL {:X} function '{}'.",
            x,
            unsafe { core::str::from_utf8_unchecked(&d[0..e]) }
        );
        let mut f = false;
        for i in self.forwards.iter_mut() {
            if !i.is_invalid() {
                continue;
            }
            (i.0, f) = (x, true);
            break;
        }
        if !f {
            bugtrack!(
                "(DLL).forward(): Cannot save forward Handle for {:X}, too many links, leaking Handle!",
                x
            );
        }
        Ok(x)
    }
}
impl<'a, F: FnOnce(&mut DLL<'a>)> Loader<'a, F> {
    #[inline]
    const fn new(func: F) -> Loader<'a, F> {
        Loader {
            dll:  UnsafeCell::new(DLL::new()),
            once: Once::new(),
            lazy: Lazy::new(),
            func: UnsafeCell::new(ManuallyDrop::new(func)),
        }
    }

    #[inline]
    pub(super) fn address(&self) -> usize {
        if self.lazy.is_new() {
            self.setup(false);
        }
        (unsafe { &*self.dll.get() }).handle.0
    }

    #[inline]
    fn init(&self) {
        let mut v = unsafe { &mut *self.func.get() };
        (unsafe { ManuallyDrop::take(&mut v) })(unsafe { &mut *self.dll.get() });
        unsafe { ManuallyDrop::drop(&mut v) };
    }
    fn setup(&self, quick: bool) {
        if self.lazy.is_ready() {
            return;
        }
        if quick {
            // Quick load some DLL's. Don't use a Mutex.
            if self.lazy.is_new() {
                self.lazy.load(|| self.init());
            }
        } else {
            if unlikely(!ntdll::NtCreateMutant.is_loaded() && !ntdll::DLL.lazy.is_init()) {
                // NOTE(dij): Base should be loaded by the time this is called,
                //            but we're gonna run a sanity check here.
                mappings::init();
            }
            if self.lazy.is_new() {
                if ntdll::NtCreateMutant.is_loaded() {
                    self.once.call_once(|| self.init());
                } else {
                    self.lazy.load(|| self.init());
                }
            }
        }
        self.lazy.force()
    }
    #[inline]
    fn is_freeable(&self) -> bool {
        self.lazy.is_ready() && matches!(unsafe { &mut *self.dll.get() }.how, LoadType::Imported)
    }
    #[inline]
    fn free(&self) -> Win32Result<()> {
        if self.lazy.is_ready() {
            unsafe { &mut *self.dll.get() }.free()
        } else {
            Ok(())
        }
    }
    #[inline]
    fn load_if_name(&self, name: fn() -> &'a str) {
        // NOTE(dij): The name arg is a function so we can wait to see if the
        //            DLL is already loaded to lazily eval and load crypt if its
        //            being used.
        if self.lazy.is_ready() {
            return;
        }
        self.setup(false); // Loading by name is never quick.
        let d = unsafe { &mut *self.dll.get() };
        if d.func.len() > 0 && !d.is_loaded() {
            // Not unlikely, as this could fail.
            unwrap(d.load_if_name(name))
        }
    }
    #[inline]
    fn load_by_handle(&self, quick: bool, h: Handle, t: LoadType) {
        if self.lazy.is_ready() {
            return;
        }
        // Init functions
        self.setup(quick);
        let d = unsafe { &mut *self.dll.get() };
        if !d.is_loaded() {
            // Unlikely to fail as it's been loaded, failure would be a missing
            // function, which is my fault.
            unwrap_unlikely(d.load(h, t))
        }
    }
    #[inline]
    fn load(&self, quick: bool, func: impl FnOnce(&mut DLL<'a>) -> Win32Result<()>) {
        if self.lazy.is_ready() {
            return;
        }
        self.setup(quick);
        let d = unsafe { &mut *self.dll.get() };
        if !d.is_loaded() {
            // Not unlikely, as this could fail.
            unwrap(func(unsafe { &mut *self.dll.get() }))
        }
    }
}

impl Copy for LoadType {}
impl Clone for LoadType {
    #[inline]
    fn clone(&self) -> LoadType {
        *self
    }
}

impl Deref for Function {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        let a = unsafe { &*self.0.get() };
        if *a == 0 {
            // TODO(dij): Address this gracefully?
            bugtrack!("DLL FUNCTION LOAD ATTEMPTED ON A NON-LOADED FUNCTION!!");
            abort!()
        }
        a
    }
}

unsafe impl Sync for Function {}
unsafe impl<F: Send> Sync for Loader<'_, F> {}

#[inline]
pub(super) fn unload_dlls() -> Win32Result<()> {
    amsi::DLL.free()?;
    wtsapi32::DLL.free()?;
    gdi32::DLL.free()?;
    dbghelp::DLL.free()?;
    psapi::DLL.free()?;
    crypt32::DLL.free()?;
    winhttp::DLL.free()?;
    netapi32::DLL.free()?;
    iphlpapi::DLL.free()?;
    userenv::DLL.free()?;
    if winsock::DLL.is_freeable() {
        // NOTE(dij): Only call WSACleanup if we loaded the DLL ourselves and
        //            did NOT pull it from the PEB, since that means that someone
        //            else may have loaded it and we shouldn't shutdown WSA ourselves.
        winsock::wsa_cleanup()
    }
    winsock::DLL.free()?;
    user32::DLL.free()?;
    advapi32::DLL.free()?;
    kernel32::KERNEL32.free()?;
    kernel32::KERNELBASE.free()
}

#[inline]
unsafe fn load_dll_raw(name: &[u8]) -> Win32Result<Handle> {
    if name.len() >= 256 {
        return Err(Win32Error::InvalidFilename);
    }
    bugtrack!(
        "Loader::load_dll_raw(): Loading DLL '{}' via LdlLoadLibrary..",
        unsafe { core::str::from_utf8_unchecked(&name) }
    );
    let mut n = [0u16; 256]; // Avoid allocations and use an array.
    for (i, v) in name.iter().enumerate() {
        if *v == 0 {
            break;
        }
        n[i] = *v as u16;
    }
    let v = UnicodeString {
        buffer:     WCharPtr::new(n.as_ptr()),
        length:     name.len() as u16 * 2,
        max_length: name.len() as u16 * 2,
    };
    winapi::ldl_load_library(&v)
}
#[inline]
unsafe fn load_func_raw(dll: Handle, name: &[u8]) -> Win32Result<usize> {
    let a = AnsiString::new(name);
    winapi::ldl_load_address(dll, 0, &a)
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Display, Formatter};

    use super::LoadType;
    use crate::prelude::*;

    impl Display for LoadType {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            match self {
                LoadType::Linked => f.write_str("Statically Linked to Binary"),
                LoadType::Dynamic => f.write_str("Dynamically Loaded from PEB"),
                LoadType::Imported => f.write_str("Loaded using LoadLibrary"),
            }
        }
    }
}
