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
#![feature(allocator_api, likely_unlikely)]
#![cfg_attr(target_family = "windows", no_std)]
#![cfg_attr(feature = "std", feature(raw_os_error_ty, io_error_more))]

#[cfg(target_family = "windows")]
pub use self::winapi::*;

pub mod env;
pub mod structs;
pub(self) mod utils;

#[cfg(target_family = "windows")]
#[macro_export]
macro_rules! dll {
    (_ $_:ident, $name:ident, $called:ident, $($i:ident),+) => {
        pub(super) static $called: $crate::loader::DLL<$name> = $crate::loader::DLL::new($name::new());

        impl $name {
            #[inline]
            const fn new() -> $name {
                $name {
                    $(
                        $i: $crate::loader::Function::new(),
                    )+
                }
            }

            pub fn verify(&self) {
                $(
                    if !self.$i.is_loaded() {
                        xrmt_bugtrack::bugtrack!("function {} not loaded!", core::stringify!($i));
                    }
                )+
            }
        }

        impl $crate::loader::Loader for $name {
            #[inline]
            fn set_func(&mut self, h: u32, e: $crate::structs::ImageExport<'_>) -> $crate::Win32Result<()> {
                match h {
                    $(
                        xrmt_winapi_fnv::fnv!($i) => self.$i.set(e.address()?),
                    )+
                    _ => (),
                }
                core::result::Result::Ok(())
            }
        }
    };
    ($name:ident, $called:ident, $func:ident, $($i:ident),+) => {
        pub struct $name {
            $(
                pub $i: $crate::loader::Function,
            )+
        }

        dll!(_ internal, $name, $called, $($i),+);

        #[inline]
        pub fn $func<'a>() -> &'a $crate::loader::Core<$name> {
            $called.load()
        }
    };
    ($name:ident, $called:ident, $func:ident, || $str:expr, $($i:ident),+) => {
        pub struct $name {
            $(
                pub $i: $crate::loader::Function,
            )+
        }

        dll!(_ internal, $name, $called, $($i),+);

        #[inline]
        pub fn $func<'a>() -> &'a $crate::loader::Core<$name> {
            $called.load_if_name(|| $str)
        }
    };
    (C $name:ident, $called:ident, $func:ident, || $str:expr, $($i:ident),+) => {
        #[repr(C)]
        pub struct $name {
            $(
                pub $i: $crate::loader::Function,
            )+
        }

        dll!(_ internal, $name, $called, $($i),+);

        #[inline]
        pub fn $func<'a>() -> &'a $crate::loader::Core<$name> {
            $called.load_if_name(|| $str)
        }
    };
}
#[cfg(target_family = "windows")]
#[macro_export]
macro_rules! syscall {
    ($addr:expr,fn($($args:ty),*) -> $ret:ty) => {
        unsafe { core::mem::transmute::<*const (), unsafe extern "system" fn($($args,)*) -> $ret>(*$addr as _) }
    };
    ($addr:expr,($($args:ty),*) -> $ret:ty, $($x:expr),*) => {
        unsafe { core::mem::transmute::<*const (), unsafe extern "system" fn($($args,)*) -> $ret>(*$addr as _)($($x,)*) }
    };
}

#[cfg(target_family = "windows")]
#[path = "."]
mod winapi {
    extern crate core;

    extern crate xrmt_bugtrack;

    pub mod alloc;
    pub(super) mod errors;
    pub mod functions;
    pub mod info;
    pub(super) mod loader;
    pub(super) mod path;
    pub mod registry;
    pub mod stdio;

    pub const INFINITE: u64 = u64::MAX;

    pub const CURRENT_THREAD: crate::structs::Handle = crate::structs::Handle::new(-2isize as usize);
    pub const CURRENT_PROCESS: crate::structs::Handle = crate::structs::Handle::new(-1isize as usize);

    pub(super) const PTR_SIZE: usize = core::mem::size_of::<usize>();

    pub use self::errors::*;
    pub use self::loader::{advapi32, amsi, crypt32, dbghelp, dnsapi, gdi32, iphlpapi, kernel32, kernel32_or_base, kernelbase, ntdll, user32, winhttp, winsock, wtsapi32};
    pub use self::path::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub(super) use self::loader::{load_dll, load_dll_hash};

    #[inline]
    pub fn unload_libraries() {
        loader::unload_dlls();
    }
    #[inline]
    pub fn unload_or_exit(c: u32) -> ! {
        // TODO(dij): Find a way to detect if we "own" the exe so can unload and
        //            kill our thread if we're the last.
        loader::unload_dlls();
        functions::exit_process(c)
    }

    #[cold]
    #[inline(never)]
    pub(super) fn loader_error() -> ! {
        xrmt_bugtrack::bugtrack!("LOADER ERROR OCCURRED, CANNOT CONTINUE!");
        #[cfg(debug_assertions)]
        {
            core::panic!("LOADER ERROR")
        }
        #[cfg(not(debug_assertions))]
        {
            unload_or_exit(90)
        }
    }
}
