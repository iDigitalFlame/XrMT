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

extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::marker::{Copy, Sized};
use core::result::Result::Ok;

use crate::io;

pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64>;

    #[inline]
    fn rewind(&mut self) -> io::Result<()> {
        self.seek(SeekFrom::Start(0))?;
        Ok(())
    }
    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        let o = self.stream_position()?;
        let n = self.seek(SeekFrom::End(0))?;
        if o != n {
            self.seek(SeekFrom::Start(o))?;
        }
        Ok(n)
    }
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

impl Eq for SeekFrom {}
impl Copy for SeekFrom {}
impl Clone for SeekFrom {
    #[inline]
    fn clone(&self) -> SeekFrom {
        *self
    }
}
impl PartialEq for SeekFrom {
    #[inline]
    fn eq(&self, other: &SeekFrom) -> bool {
        match (self, other) {
            (SeekFrom::Start(x), SeekFrom::Start(y)) => x == y,
            (SeekFrom::End(x), SeekFrom::End(y)) => x == y,
            (SeekFrom::Current(x), SeekFrom::Current(y)) => x == y,
            _ => false,
        }
    }
}

impl<S: Seek + ?Sized> Seek for &mut S {
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        (**self).stream_position()
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }
}
impl<S: Seek + ?Sized> Seek for Box<S> {
    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        (**self).stream_position()
    }
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }
}
