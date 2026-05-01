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
#![cfg(not(feature = "std"))]

extern crate core;

use core::convert::From;
use core::hint::unreachable_unchecked;
use core::marker::Sized;
use core::mem::MaybeUninit;
use core::result::Result::{Err, Ok};

use crate::{BorrowedBuf, ErrorKind, IoResult, Read, Write, BASE_BUF_SIZE};

trait BufReadSpec {
    fn size(&self) -> usize;
    fn copy_to(&mut self, d: &mut (impl ?Sized + Write)) -> IoResult<u64>;
}
trait BuffWriteSpec: Write {
    fn size(&self) -> usize;
    fn copy_from(&mut self, s: &mut (impl ?Sized + Read)) -> IoResult<u64>;
}

impl BufReadSpec for &[u8] {
    #[inline]
    fn size(&self) -> usize {
        usize::MAX
    }
    #[inline]
    fn copy_to(&mut self, d: &mut (impl ?Sized + Write)) -> IoResult<u64> {
        let n = self.len();
        d.write_all(self)?;
        // Will be in len bounds
        unsafe { *self = self.get_unchecked(n..) };
        Ok(n as u64)
    }
}
impl<T: ?Sized> BufReadSpec for T
where Self: Read
{
    #[inline]
    default fn size(&self) -> usize {
        0
    }
    default fn copy_to(&mut self, _d: &mut (impl ?Sized + Write)) -> IoResult<u64> {
        unsafe { unreachable_unchecked() }
    }
}

impl<T: ?Sized + Write> BuffWriteSpec for T {
    #[inline]
    default fn size(&self) -> usize {
        0
    }
    #[inline]
    default fn copy_from(&mut self, s: &mut (impl ?Sized + Read)) -> IoResult<u64> {
        default_copy(s, self)
    }
}

/// Copies the entire contents of a reader into a writer.
///
/// This function will continuously read data from `reader` and then
/// write it into `writer` in a streaming fashion until `reader`
/// returns EOF.
///
/// On success, the total number of bytes that were copied from
/// `reader` to `writer` is returned.
///
/// If you want to copy the contents of one file to another and you’re
/// working with filesystem paths, see the `fs::copy` function.
///
/// # Errors
///
/// This function will return an error immediately if any call to [`read`] or
/// [`write`] returns an error. All instances of [`ErrorKind::Interrupted`] are
/// handled by this function and the underlying operation is retried.
///
/// [`read`]: Read::read
/// [`write`]: Write::write
/// [`ErrorKind::Interrupted`]: crate::io::ErrorKind::Interrupted
///
/// # Examples
///
/// ```
/// use xrmt_stx::io::{self, IoResult};
///
/// fn main() -> IoResult<()> {
///     let mut reader: &[u8] = b"hello";
///     let mut writer: Vec<u8> = vec![];
///
///     io::copy(&mut reader, &mut writer)?;
///
///     assert_eq!(&b"hello"[..], &writer[..]);
///     Ok(())
/// }
/// ```
///
/// # Platform-specific behavior
///
/// On Linux (including Android), this function uses `copy_file_range(2)`,
/// `sendfile(2)` or `splice(2)` syscalls to move data directly between file
/// descriptors if possible.
///
/// Note that platform-specific behavior [may change in the future][changes].
///
/// [changes]: crate#platform-specific-behavior
#[inline]
pub fn copy(r: &mut (impl ?Sized + Read), w: &mut (impl ?Sized + Write)) -> IoResult<u64> {
    let (x, y) = (BufReadSpec::size(r), BuffWriteSpec::size(w));
    if x >= BASE_BUF_SIZE && x >= y {
        BufReadSpec::copy_to(r, w)
    } else {
        BuffWriteSpec::copy_from(w, r)
    }
}

fn default_copy(r: &mut (impl ?Sized + Read), w: &mut (impl ?Sized + Write)) -> IoResult<u64> {
    let v = &mut [MaybeUninit::uninit(); BASE_BUF_SIZE];
    let (mut b, mut n) = (BorrowedBuf::from(v.as_mut_slice()), 0u64);
    loop {
        match r.read_buf(b.unfilled()) {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
            Ok(()) => (),
        };
        if b.filled().is_empty() {
            break;
        }
        n += b.filled().len() as u64;
        w.write_all(b.filled())?;
        b.clear();
    }
    Ok(n)
}

#[cfg(feature = "alloc")]
mod alloc {
    extern crate alloc;
    extern crate core;

    use alloc::collections::vec_deque::VecDeque;
    use alloc::vec::Vec;
    use core::alloc::Allocator;
    use core::cmp::max;
    use core::convert::From;
    use core::marker::Sized;
    use core::result::Result::{Err, Ok};

    use crate::io::copy::{default_copy, BufReadSpec, BuffWriteSpec};
    use crate::{BorrowedBuf, BufReader, BufWriter, ErrorKind, IoResult, Read, Write, BASE_BUF_SIZE};

    impl<T: ?Sized> BufReadSpec for BufReader<T>
    where Self: Read
    {
        #[inline]
        fn size(&self) -> usize {
            self.capacity()
        }
        fn copy_to(&mut self, d: &mut (impl ?Sized + Write)) -> IoResult<u64> {
            let mut n = 0u64;
            loop {
                match self.read(&mut []) {
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => break Err(e),
                    Ok(_) => (),
                }
                let b = self.buffer();
                if self.buffer().len() == 0 {
                    break Ok(n);
                }
                d.write_all(b)?;
                n += b.len() as u64;
                self.discard();
            }
        }
    }
    impl<A: Allocator> BufReadSpec for VecDeque<u8, A> {
        #[inline]
        fn size(&self) -> usize {
            usize::MAX
        }
        #[inline]
        fn copy_to(&mut self, d: &mut (impl ?Sized + Write)) -> IoResult<u64> {
            let n = self.len();
            let (f, b) = self.as_slices();
            d.write_all(f)?;
            d.write_all(b)?;
            self.clear();
            Ok(n as u64)
        }
    }

    impl BuffWriteSpec for Vec<u8> {
        #[inline]
        fn size(&self) -> usize {
            max(BASE_BUF_SIZE, self.capacity() - self.len())
        }
        #[inline]
        fn copy_from(&mut self, s: &mut (impl ?Sized + Read)) -> IoResult<u64> {
            Ok(s.read_to_end(self)? as u64)
        }
    }
    impl<T: ?Sized + Write> BuffWriteSpec for BufWriter<T> {
        #[inline]
        fn size(&self) -> usize {
            self.capacity()
        }
        fn copy_from(&mut self, s: &mut (impl ?Sized + Read)) -> IoResult<u64> {
            if self.capacity() < BASE_BUF_SIZE {
                return default_copy(s, self);
            }
            let (mut n, mut v) = (0u64, 0usize);
            loop {
                let r = self.buffer_mut();
                let mut b = BorrowedBuf::from(r.spare_capacity_mut());
                unsafe { b.set_init(v) };
                if b.capacity() < BASE_BUF_SIZE {
                    self.flush_buf()?;
                    v = 0;
                    continue;
                }
                let mut c = b.unfilled();
                match s.read_buf(c.reborrow()) {
                    Ok(_) => {
                        let i = c.written();
                        if i == 0 {
                            break Ok(n);
                        }
                        (n, v) = (n + i as u64, b.init_len() - i);
                        unsafe { r.set_len(r.len() + i) };
                    },
                    Err(e) if e.kind() == ErrorKind::Interrupted => (),
                    Err(e) => break Err(e),
                }
            }
        }
    }
}
