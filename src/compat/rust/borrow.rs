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

//
// Module assistance with help from the Rust Team std/io code!
//

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

use core::convert::From;
use core::mem::{transmute, MaybeUninit};
use core::result::Result::Ok;
use core::{cmp, ptr};

use crate::io::{self, Write};
use crate::prelude::take;

pub struct BorrowedBuf<'a> {
    buf:    &'a mut [MaybeUninit<u8>],
    filled: usize,
    init:   usize,
}
pub struct BorrowedCursor<'a> {
    buf:   &'a mut BorrowedBuf<'a>,
    start: usize,
}

impl<'a> BorrowedBuf<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.filled
    }
    #[inline]
    pub fn filled(&self) -> &[u8] {
        unsafe { MaybeUninit::slice_assume_init_ref(&self.buf[0..self.filled]) }
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
    #[inline]
    pub fn init_len(&self) -> usize {
        self.init
    }
    #[inline]
    pub fn clear(&mut self) -> &mut BorrowedBuf<'a> {
        self.filled = 0;
        self
    }
    #[inline]
    pub fn unfilled<'b>(&'b mut self) -> BorrowedCursor<'b> {
        BorrowedCursor {
            start: self.filled,
            buf:   unsafe { transmute::<&'b mut BorrowedBuf<'a>, &'b mut BorrowedBuf<'b>>(self) },
        }
    }

    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut BorrowedBuf<'a> {
        self.init = cmp::max(self.init, n);
        self
    }
}
impl<'a> BorrowedCursor<'a> {
    #[inline]
    pub fn written(&self) -> usize {
        self.buf.filled - self.start
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.capacity() - self.buf.filled
    }
    #[inline]
    pub fn init_ref(&self) -> &[u8] {
        unsafe { MaybeUninit::slice_assume_init_ref(&self.buf.buf[self.buf.filled..self.buf.init]) }
    }
    #[inline]
    pub fn append(&mut self, buf: &[u8]) {
        unsafe {
            MaybeUninit::copy_from_slice(&mut self.as_mut()[..buf.len()], buf);
            self.set_init(buf.len());
        }
        self.buf.filled += buf.len();
    }
    #[inline]
    pub fn init_mut(&mut self) -> &mut [u8] {
        unsafe { MaybeUninit::slice_assume_init_mut(&mut self.buf.buf[self.buf.filled..self.buf.init]) }
    }
    #[inline]
    pub fn uninit_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.buf.buf[self.buf.init..]
    }
    #[inline]
    pub fn reborrow<'b>(&'b mut self) -> BorrowedCursor<'b> {
        BorrowedCursor {
            buf:   unsafe { transmute::<&'b mut BorrowedBuf<'a>, &'b mut BorrowedBuf<'b>>(self.buf) },
            start: self.start,
        }
    }
    #[inline]
    pub fn ensure_init(&mut self) -> &mut BorrowedCursor<'a> {
        let u = self.uninit_mut();
        unsafe { ptr::write_bytes(u.as_mut_ptr(), 0, u.len()) };
        self.buf.init = self.buf.capacity();
        self
    }

    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.buf.buf[self.buf.filled..]
    }
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) -> &mut BorrowedCursor<'a> {
        self.buf.filled += n;
        self.buf.init = cmp::max(self.buf.init, self.buf.filled);
        self
    }
    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut BorrowedCursor<'a> {
        self.buf.init = cmp::max(self.buf.init, self.buf.filled + n);
        self
    }
}

impl<'a> Write for BorrowedCursor<'a> {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.append(buf);
        Ok(buf.len())
    }
}

impl<'a> From<&'a mut [u8]> for BorrowedBuf<'a> {
    #[inline]
    fn from(v: &'a mut [u8]) -> BorrowedBuf<'a> {
        let n = v.len();
        BorrowedBuf {
            buf:    take(unsafe { (v as *mut [u8]).as_uninit_slice_mut() }),
            init:   n,
            filled: 0usize,
        }
    }
}
impl<'a> From<&'a mut [MaybeUninit<u8>]> for BorrowedBuf<'a> {
    #[inline]
    fn from(v: &'a mut [MaybeUninit<u8>]) -> BorrowedBuf<'a> {
        BorrowedBuf {
            buf:    v,
            filled: 0usize,
            init:   0usize,
        }
    }
}
