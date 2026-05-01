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

extern crate alloc;
extern crate core;

extern crate xrmt_io;

use alloc::boxed::Box;
use core::convert::{AsRef, From};
use core::marker::PhantomData;
use core::ops::Drop;
use core::ptr::{copy_nonoverlapping, null, null_mut};
use core::result::Result::{Err, Ok};

use xrmt_io::Write;

use crate::functions::{HeapAlloc, HeapCreate, HeapDestroy, HeapFree, HeapReAlloc};
use crate::structs::{Handle, Region};
use crate::{dbghelp, syscall, Win32Error, Win32Result};

const INITIAL_SIZE: usize = 2 << 15;

pub enum MinDumpOutput<'a> {
    Writer(&'a mut dyn Write),
    Handle(&'a Handle),
}

#[repr(C)]
struct MiniDumpData {
    heap:    Region,
    size:    usize,
    handle:  Handle,
    written: usize,
}
#[repr(C, packed)]
struct MiniDumpInput {
    pid:       u32,
    handle:    Handle,
    callback:  u32,
    io_handle: usize,
    io_offset: u64,
    io_buffer: usize,
    io_size:   u32,
}
#[repr(C)]
struct MiniDumpStatus {
    status: i32,
}
#[repr(C)]
struct MiniDumpCallback<'a> {
    func:  unsafe extern "system" fn(*mut Box<MiniDumpData>, *const MiniDumpInput, *mut MiniDumpStatus) -> u32,
    param: *mut Box<MiniDumpData>,
    p:     PhantomData<&'a MiniDumpData>,
}

impl MiniDumpData {
    #[inline]
    fn new() -> Win32Result<MiniDumpData> {
        // 0x1002 - MEM_COMMIT? | HEAP_GROWABLE
        let h = HeapCreate(0x1002, INITIAL_SIZE, INITIAL_SIZE)?;
        let b = HeapAlloc(h, INITIAL_SIZE, true).map_err(|e| {
            let _ = HeapDestroy(h); // Close Heap if the alloc fails.
            e
        })?;
        Ok(MiniDumpData {
            heap:    b,
            size:    INITIAL_SIZE,
            handle:  h,
            written: 0usize,
        })
    }

    #[inline]
    fn resize(&mut self, size: usize) -> Win32Result<()> {
        if size < self.size {
            Ok(())
        } else {
            let n = self.size + (size * 2);
            self.heap = HeapReAlloc(self.handle, 0, self.heap, n)?;
            self.size = n;
            Ok(())
        }
    }
    fn copy(&mut self, input: &MiniDumpInput) -> Win32Result<()> {
        self.resize((input.io_offset as usize) + (input.io_size as usize))?;
        unsafe {
            copy_nonoverlapping(
                input.io_buffer as *const u8,
                self.heap.as_mut_ptr().add(input.io_offset as usize),
                input.io_size as usize,
            )
        };
        self.written += input.io_size as usize;
        Ok(())
    }
    #[inline]
    fn finish(&mut self, w: &mut dyn Write) -> Win32Result<usize> {
        let _ = w.write_all(unsafe { self.heap.as_slice(self.written) })?;
        Ok(self.written)
    }
}
impl<'a> MiniDumpCallback<'a> {
    #[inline]
    fn new(v: &'a mut Box<MiniDumpData>) -> MiniDumpCallback<'a> {
        MiniDumpCallback {
            p:     PhantomData,
            func:  _dump_callback,
            param: v,
        }
    }
}

impl Drop for MiniDumpData {
    #[inline]
    fn drop(&mut self) {
        let _ = HeapFree(self.handle, self.heap.as_ptr());
        let _ = HeapDestroy(self.handle);
    }
}

impl<'a> From<&'a mut dyn Write> for MinDumpOutput<'a> {
    #[inline]
    fn from(v: &'a mut dyn Write) -> MinDumpOutput<'a> {
        MinDumpOutput::Writer(v)
    }
}
impl<'a, T: Write> From<&'a mut T> for MinDumpOutput<'a> {
    #[inline]
    fn from(v: &'a mut T) -> MinDumpOutput<'a> {
        MinDumpOutput::Writer(v)
    }
}
impl<'a, T: AsRef<Handle>> From<&'a T> for MinDumpOutput<'a> {
    #[inline]
    fn from(v: &'a T) -> MinDumpOutput<'a> {
        MinDumpOutput::Handle(v.as_ref())
    }
}

#[inline]
pub(crate) fn to_handle(h: &Handle, f: &Handle, pid: u32, flags: u32) -> Win32Result<usize> {
    let r = syscall!(
        dbghelp().MiniDumpWriteDump,
       (Handle, u32, Handle, u32, *const u8, *const u8, *mut u8) -> u32,
        *h,
        pid,
        *f,
        flags,
        null(),
        null(),
        null_mut()
    );
    if r == 0 {
        Err(Win32Error::last_error())
    } else {
        Ok(0)
    }
}
#[inline]
pub(crate) fn to_writer(h: &Handle, w: &mut dyn Write, pid: u32, flags: u32) -> Win32Result<usize> {
    let mut d = Box::new(MiniDumpData::new()?);
    let mut a = MiniDumpCallback::new(&mut d);
    let r = syscall!(
        dbghelp().MiniDumpWriteDump,
        (Handle, u32, Handle, u32, *const u8, *const u8, *mut MiniDumpCallback) -> u32,
        *h,
        pid,
        Handle::EMPTY,
        flags,
        null(),
        null(),
        &mut a
    );
    if r == 0 {
        Err(Win32Error::last_error())
    } else {
        d.finish(w)
    }
}

unsafe extern "system" fn _dump_callback(arg: *mut Box<MiniDumpData>, input: *const MiniDumpInput, out: *mut MiniDumpStatus) -> u32 {
    let (r, s) = match unsafe { &*input }.callback {
        0 | 1 | 2 | 3 | 4 => (1, 0x7F), // 0x7F includes most Module/Thread info.
        11 => (1, 1),
        12 => unsafe { arg.as_mut().map_or((0, 1), |d| d.copy(&*input).map_or((0, 1), |_| (1, 0))) },
        _ => (1, 0),
    };
    unsafe { (*out).status = s };
    r
}
