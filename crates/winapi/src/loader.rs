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

extern crate xrmt_bugtrack;

use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::PartialEq;
use core::convert::{From, Into};
use core::hint::unlikely;
use core::iter::{IntoIterator, Iterator};
use core::marker::{Copy, Sync};
use core::matches;
use core::mem::swap;
use core::ops::{Deref, FnOnce};
use core::option::Option::{self, None, Some};
use core::ptr::drop_in_place;
use core::result::Result::{Err, Ok};
use core::sync::atomic::{AtomicBool, Ordering};

use xrmt_bugtrack::bugtrack;

use crate::functions::{wsa_cleanup, FreeLibrary, GetCurrentProcessPEB, LdlLoadLibrary};
use crate::structs::{ForwardList, ImageExport, ImageExportsIter, Lazy, LoaderEntry, NonZeroHandle, UnicodeString, WCharLike};
use crate::{loader_error, Win32Error, Win32Result};

static LOAD_COMPLETE: AtomicBool = AtomicBool::new(false);

mod advapi32;
mod crypt32;
mod gdi32;
mod kernel32;
mod ntdll;
mod simple;
mod user32;
mod winsock;
mod wtsapi32;

pub use self::advapi32::advapi32;
pub use self::crypt32::crypt32;
pub use self::gdi32::gdi32;
pub use self::kernel32::{kernel32, kernel32_or_base, kernel32_or_base_address, kernelbase};
pub use self::ntdll::ntdll;
pub use self::simple::{amsi, dbghelp, dnsapi, iphlpapi, winhttp};
pub use self::user32::user32;
pub use self::winsock::winsock;
pub use self::wtsapi32::wtsapi32;

/// Reason the DLL was loaded.
pub enum Reason {
    /// It's statically linked into the binary, don't increase load_count.
    /// Don't try to free it either.
    Linked,
    /// It was dynamically loaded by the host binary, increase the load_count.
    /// Don't try to free it either.
    Dynamic,
    /// This was loaded by us using LoadLibrary. Free this when done.
    Imported,
}

#[repr(transparent)]
pub struct Function(usize);
pub struct DLL<T: Loader> {
    core:  UnsafeCell<Core<T>>,
    mutex: Lazy,
}
pub struct Core<T: Loader> {
    v:       T,
    how:     Reason,
    handle:  NonZeroHandle,
    forward: ForwardList,
}

pub trait Loader {
    fn set_func(&mut self, h: u32, a: ImageExport<'_>) -> Win32Result<()>;
}

impl Function {
    #[inline]
    pub const fn new() -> Function {
        Function(0usize)
    }

    #[inline]
    pub fn is_loaded(&self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub fn set(&mut self, a: usize) {
        self.0 = a
    }
}
impl<T: Loader> DLL<T> {
    #[inline]
    pub const fn new(v: T) -> DLL<T> {
        DLL {
            core:  UnsafeCell::new(Core {
                v,
                how: Reason::Linked,
                handle: NonZeroHandle::invalid(),
                forward: ForwardList::new(),
            }),
            mutex: Lazy::new(),
        }
    }

    #[inline]
    pub fn unload(&self) {
        if !self.mutex.is_ready() {
            return;
        }
        unsafe { &mut *self.core.get() }.unload();
    }
    #[inline]
    pub fn is_loaded(&self) -> bool {
        self.mutex.is_ready()
    }
    #[inline]
    pub fn address_if_loaded(&self) -> Option<NonZeroHandle> {
        if self.mutex.is_ready() {
            Some(unsafe { &*self.core.get() }.handle)
        } else {
            None
        }
    }
    #[inline]
    pub fn load<'a>(&'a self) -> &'a Core<T> {
        // This indicates a DLL that should ONLY load by Handle and should fail
        // if a name load was attempted. This mostly applies to ntdll.dll
        self.load_if_name(|| loader_error())
    }
    #[inline]
    pub fn load_if_name<'a>(&'a self, name: impl FnOnce() -> &'a str) -> &'a Core<T> {
        self.mutex.load(|| self.sync(name));
        unsafe { &*self.core.get() }
    }

    #[inline]
    fn update(&self, v: (NonZeroHandle, Reason)) {
        // Loading here is fine as the Mutex is locked during this
        // time and this most likely is called during 'dll_init'.
        if unlikely(unsafe { &mut *self.core.get() }.update(v.1, v.0).is_err()) {
            loader_error();
        }
        self.mutex.force(); // Clear Mutex to signal completion of loading.
    }
    #[inline]
    fn sync<'a>(&self, name: impl FnOnce() -> &'a str) {
        if LOAD_COMPLETE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            init(); // Do first load
        }
        // This will be true if the DLL was already loaded.
        if self.mutex.is_ready() {
            return;
        }
        if unlikely(unsafe { &mut *self.core.get() }.update_with_name(name()).is_err()) {
            loader_error();
        }
        self.mutex.force(); // Clear Mutex to signal completion of loading.
    }
}
impl<T: Loader> Core<T> {
    #[inline]
    pub fn address(&self) -> NonZeroHandle {
        self.handle
    }

    fn unload(&mut self) {
        if unlikely(self.handle.is_invalid()) {
            return;
        }
        bugtrack!(
            "(Core).unload(): Unloading DLL 0x{:X} ({}).",
            self.handle,
            self.how
        );
        match self.how {
            Reason::Linked => {},
            Reason::Imported => {
                let _ = FreeLibrary(*self.handle);
            },
            Reason::Dynamic => self.unload_handle(),
        }
        // Drop forwards
        unsafe { drop_in_place(&mut self.forward) };
    }
    #[inline]
    fn unload_handle(&self) {
        for i in GetCurrentProcessPEB().modules() {
            if self.handle.eq(&i.dll_base) {
                i.reference_decrease();
                return;
            }
        }
        bugtrack!(
            "(Core).unload(): Dynamically imported DLL 0x{:X} was not found in the PEB entry list! Was it already unloaded?",
            self.handle
        )
    }
    #[inline]
    fn setup(&mut self) -> Win32Result<()> {
        // Loading here is fine as the Mutex is locked during this time.
        if unlikely(self.handle.is_invalid()) {
            return Err(Win32Error::InvalidHandle);
        }
        bugtrack!(
            "(Core).setup(): Loading functions for DLL at 0x{:X}.",
            self.handle
        );
        let mut v = ImageExportsIter::new(self.handle.get())?;
        for i in &mut v {
            self.v.set_func(i.hash(), i)?;
        }
        // Take forwards and swap them to prevent them from being dropped.
        swap(v.forwards(), &mut self.forward);
        Ok(())
    }
    #[inline]
    fn update_with_name(&mut self, v: &str) -> Win32Result<()> {
        bugtrack!("(Core).update_with_name(): Loading DLL by '{v}' by name..");
        (self.how, self.handle) = (Reason::Imported, load_dll(v.as_bytes())?);
        bugtrack!(
            "(Core).update_with_name(): DLL '{v}' loaded at 0x{:X}.",
            self.handle
        );
        self.setup()
    }
    #[inline]
    fn update(&mut self, r: Reason, h: NonZeroHandle) -> Win32Result<()> {
        (self.how, self.handle) = (r, h);
        self.setup()
    }
}

impl Copy for Reason {}
impl Clone for Reason {
    #[inline]
    fn clone(&self) -> Reason {
        *self
    }
}

impl Copy for Function {}
impl Clone for Function {
    #[inline]
    fn clone(&self) -> Function {
        Function(self.0)
    }
}
impl Deref for Function {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        if unlikely(self.0 == 0) {
            bugtrack!("DLL FUNCTION LOAD ATTEMPTED ON A NON-LOADED FUNCTION!!");
            loader_error()
        }
        &self.0
    }
}

impl<T: Loader> Deref for Core<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.v
    }
}

unsafe impl<T: Loader> Sync for DLL<T> {}

#[inline]
pub(crate) fn unload_dlls() {
    if unlikely(!LOAD_COMPLETE.load(Ordering::Acquire)) {
        // RARE: Only load ntdll to exit cleanly.
        bugtrack!("unload_dlls(): No DLLs loaded, loading ntdll to cleanly exit..");
        init_only_ntdll();
        return;
    }
    bugtrack!("unload_dlls(): Unloading all loaded DLLs..");
    // Unload in opposite order of loading.
    simple::AMSI.unload();
    wtsapi32::WTSAPI32.unload();
    gdi32::GDI32.unload();
    simple::DBGHELP.unload();
    crypt32::CRYPT32.unload();
    simple::WINHTTP.unload();
    simple::IPHLPAPI.unload();
    // Before unloading WinSock, if it's been loaded, clean it up first.
    if winsock::WINSOCK.is_loaded() && matches!(winsock().how, Reason::Imported) {
        wsa_cleanup();
    }
    winsock::WINSOCK.unload();
    user32::USER32.unload();
    advapi32::ADVAPI32.unload();
    kernel32::KERNEL32.unload();
    kernel32::KERNELBASE.unload();
    // ntdll is NEVER unloaded.
    bugtrack!("unload_dlls(): Unload complete!");
}
#[inline]
pub(crate) fn load_dll_hash(v: u32) -> Option<NonZeroHandle> {
    // Quickpath to load any DLLs we already loaded.
    match v {
        // ntdll.dll
        0xA9ACADD3 => return ntdll::NTDLL.address_if_loaded(),
        // kernelbase.dll
        0x55AA707B => return kernel32::KERNELBASE.address_if_loaded(),
        // kernel32.dll
        0xD741ACCF => return kernel32::KERNEL32.address_if_loaded(),
        // advapi32.dll
        0x316180CD => return advapi32::ADVAPI32.address_if_loaded(),
        // user32.dll
        0xB1CC909D => return user32::USER32.address_if_loaded(),
        // ws2_32.dll
        0x6CFD82A3 => return winsock::WINSOCK.address_if_loaded(),
        // iphlpapi.dll
        0x86DA0E28 => return simple::IPHLPAPI.address_if_loaded(),
        // winhttp.dll
        0x198007FF => return simple::WINHTTP.address_if_loaded(),
        // crypt32.dll
        0xC38FCEAE => return crypt32::CRYPT32.address_if_loaded(),
        // dbhhelp.dll
        0xB674F88D => return simple::DBGHELP.address_if_loaded(),
        // gpi32.dll
        0x6C411560 => return gdi32::GDI32.address_if_loaded(),
        // wtsapi32.dll
        0xEABBD160 => return wtsapi32::WTSAPI32.address_if_loaded(),
        // asmi.dll
        0x9D8C6359 => return simple::AMSI.address_if_loaded(),
        _ => (),
    }
    // Search PEB to see if it's contained there?
    for i in GetCurrentProcessPEB().modules() {
        if i.name_base.hash_fnv32() == v {
            return Some(NonZeroHandle::new(i.dll_base));
        }
    }
    None
}
#[inline]
pub(crate) fn load_dll(name: &[u8]) -> Win32Result<NonZeroHandle> {
    if name.len() > 255 {
        return Err(Win32Error::InvalidName);
    }
    bugtrack!(
        "load_dll(): Loading DLL '{}' via LdlLoadLibrary..",
        unsafe { core::str::from_utf8_unchecked(&name) }
    );
    let mut n = [0u16; 256]; // Avoid allocations and use an array.
    for (i, v) in name.iter().enumerate() {
        if *v == 0 {
            break;
        }
        // This is lazy encoding to u16, but all the DLL names we use won't
        // contain non-ASCII characters. Most system DLLs won't either. So this is ok!
        unsafe { *n.get_unchecked_mut(i) = *v as u16 }
    }
    // The name length should be the same, so indexing this way is ok.
    let b = WCharLike::from(unsafe { n.get_unchecked(0..name.len()) });
    let u = UnicodeString::new(&b);
    // We don't use the macro here as it has a debug assert for checking if
    // WChars created properly end with NULL. Since this may not happen, we
    // work around it by doing it manually.
    unsafe { LdlLoadLibrary(&u) }
}

fn init() {
    bugtrack!("init(): PEB DLL loading started!");
    // There CANNOT be any other instructions that call syscalls in here
    // besides the 'Ldr' syscalls.
    //
    // If any other calls are made, this WILL cause a CRASH!!!
    let mut e = [(NonZeroHandle::invalid(), Reason::Linked); 0xD];
    for i in GetCurrentProcessPEB().modules() {
        bugtrack!(
            "init(): Found PEB Module '{}' (0x{:X}).",
            i.name_full,
            i.dll_base
        );
        // Match DLL name by its lower FNV32 hash.
        // Why does the compiler BCE this? It's fucking static.
        unsafe {
            match i.name_base.hash_fnv32() {
                // ntdll.dll - We don't increase the load count for ntdll as it will
                //             never be unloaded.
                0xA9ACADD3 => *e.get_unchecked_mut(0x0) = (i.dll_base.into(), Reason::Linked),
                // kernelbase.dll
                0x55AA707B => *e.get_unchecked_mut(0x1) = init_dll(i),
                // kernel32.dll
                0xD741ACCF => *e.get_unchecked_mut(0x2) = init_dll(i),
                // advapi32.dll
                0x316180CD => *e.get_unchecked_mut(0x3) = init_dll(i),
                // user32.dll
                0xB1CC909D => *e.get_unchecked_mut(0x4) = init_dll(i),
                // ws2_32.dll
                0x6CFD82A3 => *e.get_unchecked_mut(0x5) = init_dll(i),
                // iphlpapi.dll
                0x86DA0E28 => *e.get_unchecked_mut(0x6) = init_dll(i),
                // winhttp.dll
                0x198007FF => *e.get_unchecked_mut(0x7) = init_dll(i),
                // crypt32.dll
                0xC38FCEAE => *e.get_unchecked_mut(0x8) = init_dll(i),
                // dbhhelp.dll
                0xB674F88D => *e.get_unchecked_mut(0x9) = init_dll(i),
                // gpi32.dll
                0x6C411560 => *e.get_unchecked_mut(0xA) = init_dll(i),
                // wtsapi32.dll
                0xEABBD160 => *e.get_unchecked_mut(0xB) = init_dll(i),
                // asmi.dll
                0x9D8C6359 => *e.get_unchecked_mut(0xC) = init_dll(i),
                _ => (),
            }
        }
    }
    // Load DLLs in a specific order so we can resolve references easily.
    // Luckly this will omit the bounds checks.
    for (i, v) in e.into_iter().enumerate() {
        if v.0.is_invalid() {
            continue;
        }
        match i {
            0x0 => ntdll::NTDLL.update(v),
            0x1 => kernel32::KERNELBASE.update(v),
            0x2 => kernel32::KERNEL32.update(v),
            0x3 => advapi32::ADVAPI32.update(v),
            0x4 => user32::USER32.update(v),
            0x5 => winsock::WINSOCK.update(v),
            0x6 => simple::IPHLPAPI.update(v),
            0x7 => simple::WINHTTP.update(v),
            0x8 => crypt32::CRYPT32.update(v),
            0x9 => simple::DBGHELP.update(v),
            0xA => gdi32::GDI32.update(v),
            0xB => wtsapi32::WTSAPI32.update(v),
            0xC => simple::AMSI.update(v),
            _ => (),
        }
    }
    bugtrack!("init(): PEB Loading complete!")
}
#[inline]
fn init_only_ntdll() {
    for i in GetCurrentProcessPEB().modules() {
        match i.name_base.hash_fnv32() {
            // ntdll.dll
            0xA9ACADD3 => {
                ntdll::NTDLL.update((i.dll_base.into(), Reason::Linked));
                return;
            },
            _ => (),
        }
    }
}
#[inline]
fn init_dll(e: &mut LoaderEntry) -> (NonZeroHandle, Reason) {
    // Load and increase load count so it won't get unloaded if the parent
    // process frees it.
    //
    // Indictate and skip staticly linked DLLs.
    if e.is_static() {
        bugtrack!(
            "init_dll(): DLL at 0x{:X} is statically linked.",
            e.dll_base
        );
        (e.dll_base.into(), Reason::Linked)
    } else {
        bugtrack!(
            "init_dll(): Increasing reference count for dynamically loaded DLL at 0x{:X}.",
            e.dll_base
        );
        e.reference_increase();
        (e.dll_base.into(), Reason::Dynamic)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, LowerHex, Result, UpperHex};
    use core::write;

    use crate::loader::{Function, Reason};

    impl Debug for Reason {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Reason {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Reason::Linked => f.write_str("Statically Linked"),
                Reason::Dynamic => f.write_str("Loaded from PEB"),
                Reason::Imported => f.write_str("Loaded using LdlLoadLibrary"),
            }
        }
    }

    impl Debug for Function {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "FUNC(0x{:X})", self.0)
        }
    }
    impl Display for Function {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "Function 0x{:X}", self.0)
        }
    }
    impl LowerHex for Function {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "{:x}", self.0)
        }
    }
    impl UpperHex for Function {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "{:X}", self.0)
        }
    }
}
