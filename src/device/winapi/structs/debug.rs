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

use core::marker::PhantomData;
use core::ptr;

use crate::device::winapi::{self, Handle, Region, Win32Result};
use crate::fs::File;
use crate::ignore_error;
use crate::io::Write;
use crate::prelude::*;

const INITIAL_SIZE: usize = 2 << 16;

pub enum MinDumpOutput<'a> {
    Writer(&'a mut dyn Write),
    File(&'a mut File),
    Handle(Handle),
}

#[repr(C)]
pub(crate) struct MiniDumpData {
    heap:    Region,
    size:    usize,
    handle:  Handle,
    written: usize,
}
#[repr(C, packed)]
pub(crate) struct MiniDumpInput {
    pid:       u32,
    handle:    Handle,
    callback:  u32,
    io_handle: usize,
    io_offset: u64,
    io_buffer: usize,
    io_size:   u32,
}
#[repr(C)]
pub(crate) struct MiniDumpStatus {
    status: i32,
}
#[repr(C)]
pub(crate) struct MiniDumpCallback<'a> {
    func:  unsafe extern "stdcall" fn(*mut Box<MiniDumpData>, *const MiniDumpInput, *mut MiniDumpStatus) -> u32,
    param: *mut Box<MiniDumpData>,
    p:     PhantomData<&'a Box<MiniDumpData>>,
}

impl MiniDumpData {
    #[inline]
    pub fn new() -> Win32Result<MiniDumpData> {
        // 0x1002 - MEM_COMMIT? | HEAP_GROWABLE
        let h = winapi::HeapCreate(0x1002, INITIAL_SIZE, INITIAL_SIZE)?;
        let b = winapi::HeapAlloc(h, INITIAL_SIZE, true).or_else(|e| {
            ignore_error!(winapi::HeapDestroy(h));
            Err(e)
        })?;
        Ok(MiniDumpData {
            heap:    b,
            size:    INITIAL_SIZE,
            handle:  h,
            written: 0usize,
        })
    }

    #[inline]
    pub fn finish(&mut self, w: &mut dyn Write) -> Win32Result<usize> {
        unsafe { w.write_all(self.heap.as_slice(self.written)) }?;
        Ok(self.written)
    }

    fn resize(&mut self, size: usize) -> Win32Result<()> {
        if size < self.size {
            Ok(())
        } else {
            let n = (self.size + size) * 2;
            self.heap = winapi::HeapReAlloc(self.handle, 0, self.heap, n)?;
            self.size = n;
            Ok(())
        }
    }
    fn copy(&mut self, input: &MiniDumpInput) -> Win32Result<()> {
        self.resize(input.io_offset as usize + input.io_size as usize)?;
        unsafe {
            ptr::copy_nonoverlapping(
                input.io_buffer as *const u8,
                self.heap.as_mut_ptr().add(input.io_offset as usize),
                input.io_size as usize,
            )
        };
        self.written += input.io_size as usize;
        Ok(())
    }
}
impl MiniDumpCallback<'_> {
    #[inline]
    pub fn new(data: &mut Box<MiniDumpData>) -> MiniDumpCallback {
        MiniDumpCallback {
            p:     PhantomData,
            func:  _dump_callback,
            param: data,
        }
    }
}

impl Drop for MiniDumpData {
    #[inline]
    fn drop(&mut self) {
        ignore_error!(winapi::HeapFree(self.handle, self.heap.as_ptr()));
        ignore_error!(winapi::HeapDestroy(self.handle));
    }
}

unsafe extern "stdcall" fn _dump_callback(arg: *mut Box<MiniDumpData>, input: *const MiniDumpInput, out: *mut MiniDumpStatus) -> u32 {
    let (r, s) = match (&*input).callback {
        0 | 1 | 2 | 3 | 4 => (1, 0x7F), // 0x7F includes most Module/Thread info.
        11 => (1, 1),
        12 => arg.as_mut().map_or_else(
            || (0, 1),
            |d| d.copy(&*input).map_or_else(|_| (0, 1), |_| (1, 0)),
        ),
        _ => (1, 0),
    };
    (*out).status = s;
    r
}
