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

pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "rust"]
mod inner {
    extern crate alloc;
    extern crate core;

    use core::mem::size_of;

    const LO: usize = repeat_byte(0x01u8);
    const HI: usize = repeat_byte(0x80u8);
    const PTR_SIZE: usize = size_of::<usize>();
    const BUF_SIZE: usize = if cfg!(target_os = "espidf") { 0x200 } else { 0x2000 };

    use alloc::string::String;
    use core::convert::From;
    use core::marker::Sized;
    use core::mem::MaybeUninit;
    use core::result::Result::{Err, Ok};

    mod borrow;
    mod error;
    mod read;
    mod seek;
    mod write;

    pub use self::borrow::*;
    pub use self::error::*;
    pub use self::read::*;
    pub use self::seek::*;
    pub use self::write::*;
    #[doc(inline)]
    pub use crate::device::winapi::{Stderr, StderrLock, Stdin, StdinLock, Stdout, StdoutLock};

    pub mod prelude {
        pub use crate::io::{BufRead, Read, Seek, Write};
    }

    #[inline]
    pub fn stdin() -> Stdin {
        Stdin::get()
    }
    #[inline]
    pub fn stdout() -> Stdout {
        Stdout::get()
    }
    #[inline]
    pub fn stderr() -> Stderr {
        Stderr::get()
    }
    #[inline]
    pub fn read_to_string(mut reader: impl Read) -> Result<String> {
        let mut b = String::new();
        reader.read_to_string(&mut b)?;
        Ok(b)
    }
    pub fn copy<R: Read + ?Sized, W: Write + ?Sized>(reader: &mut R, writer: &mut W) -> Result<u64> {
        let t: &mut [_] = &mut [MaybeUninit::uninit(); BUF_SIZE];
        let mut buf: BorrowedBuf<'_> = BorrowedBuf::from(t);
        let mut n = 0;
        loop {
            match reader.read_buf(buf.unfilled()) {
                Ok(()) => (),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            if buf.filled().is_empty() {
                break;
            }
            n += buf.filled().len() as u64;
            writer.write_all(buf.filled())?;
            buf.clear();
        }
        Ok(n)
    }

    #[inline]
    const fn repeat_byte(b: u8) -> usize {
        if cfg!(target_pointer_width = "16") {
            (b as usize) << 8 | b as usize
        } else {
            (b as usize) * (usize::MAX / 0xFF)
        }
    }
    #[inline]
    const fn zero_byte(x: usize) -> bool {
        x.wrapping_sub(LO) & !x & HI != 0
    }
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
mod inner {
    extern crate std;
    pub use std::io::*;
}
