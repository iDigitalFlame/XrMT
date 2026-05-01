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
#![no_std]
#![allow(incomplete_features, internal_features)]
#![feature(
    alloc_layout_extra,
    allocator_api,
    ascii_char,
    bstr,
    core_intrinsics,
    extend_one,
    likely_unlikely,
    panic_internals,
    ptr_alignment_type,
    ptr_internals,
    sized_type_properties,
    slice_index_methods,
    specialization,
    try_reserve_kind,
    unchecked_shifts,
    vec_into_raw_parts
)]
#![cfg_attr(any(not(windows), feature = "std"), feature(can_vector, seek_stream_len))]

extern crate alloc;
extern crate core;

extern crate xrmt_io;

use alloc::alloc::Global;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::clone::Clone;
use core::convert::{AsRef, From};
use core::hint::unlikely;
use core::marker::{Copy, Sized};
use core::ops::{BitOrAssign, Deref, DerefMut};
use core::option::Option::{self, None, Some};
use core::ptr::copy_nonoverlapping;
use core::result::Result::{Err, Ok};
use core::slice::from_mut;
use core::time::Duration;

use xrmt_io::{ErrorKind, IoError, IoResult, Read, Write};

use crate::text::{utf8_to_lossy_owned, utf8_to_lossy_rewrite};

pub mod base64;
mod blob;
mod chunk;
mod fiber;
pub mod text;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::chunk::Chunk;
pub use self::blob::{BasicBuffer, Blob, Buffer, Slice};
pub use self::fiber::*;

pub trait Writable {
    fn write_stream(&self, w: &mut impl Writer) -> IoResult<()>;
}
pub trait Readable {
    fn read_stream(&mut self, r: &mut impl Reader) -> IoResult<()>;
}
pub trait Reader: Read {
    #[inline]
    fn read_bool(&mut self) -> IoResult<bool> {
        Ok(self.read_i8()? == 1)
    }

    #[inline]
    fn read_f32(&mut self) -> IoResult<f32> {
        let mut b: [u8; 4] = [0; 4];
        if self.read(&mut b)? != 4 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(f32::from_be_bytes(b))
    }
    #[inline]
    fn read_f64(&mut self) -> IoResult<f64> {
        let mut b: [u8; 8] = [0; 8];
        if self.read(&mut b)? != 8 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(f64::from_be_bytes(b))
    }

    #[inline]
    fn read_i8(&mut self) -> IoResult<i8> {
        Ok(self.read_u8()? as i8)
    }
    #[inline]
    fn read_i16(&mut self) -> IoResult<i16> {
        Ok(self.read_u16()? as i16)
    }
    #[inline]
    fn read_i32(&mut self) -> IoResult<i32> {
        Ok(self.read_u32()? as i32)
    }
    #[inline]
    fn read_i64(&mut self) -> IoResult<i64> {
        Ok(self.read_u64()? as i64)
    }

    #[inline]
    fn read_u8(&mut self) -> IoResult<u8> {
        let mut b = 0u8;
        if self.read(from_mut(&mut b))? != 1 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(b)
    }
    #[inline]
    fn read_u16(&mut self) -> IoResult<u16> {
        let mut b = [0u8; 2];
        if self.read(&mut b)? != 2 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(u16::from_be_bytes(b))
    }
    #[inline]
    fn read_u32(&mut self) -> IoResult<u32> {
        let mut b = [0u8; 4];
        if self.read(&mut b)? != 4 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(u32::from_be_bytes(b))
    }
    #[inline]
    fn read_u64(&mut self) -> IoResult<u64> {
        let mut b = [0u8; 8];
        if self.read(&mut b)? != 8 {
            return Err(IoError::from(ErrorKind::UnexpectedEof));
        }
        Ok(u64::from_be_bytes(b))
    }

    fn read_leb128(&mut self) -> IoResult<u64> {
        let (mut n, mut b) = (0u64, 0u8);
        for i in 0..10 {
            if self.read(from_mut(&mut b))? != 1 {
                return Err(IoError::from(ErrorKind::UnexpectedEof));
            }
            n |= unsafe { ((b & 0x7F) as u64).unchecked_shl((i * 7) as u32) };
            if b & 0x80 == 0 {
                break;
            }
        }
        Ok(n)
    }

    #[inline]
    fn read_duration(&mut self) -> IoResult<Duration> {
        Ok(Duration::from_nanos(self.read_u64()?))
    }

    #[inline]
    fn read_vec(&mut self) -> IoResult<Vec<u8>> {
        self.read_vec_in(Global)
    }
    #[inline]
    fn read_maybe_vec(&mut self) -> IoResult<Option<Vec<u8>>> {
        self.read_maybe_vec_in(Global)
    }
    fn read_vec_in<A: Allocator>(&mut self, alloc: A) -> IoResult<Vec<u8, A>> {
        let n = match read_size(self)? {
            Some(v) => v,
            None => return Ok(Vec::new_in(alloc)),
        };
        let mut b = Vec::with_capacity_in(n as usize, alloc);
        unsafe { b.set_len(n as usize) };
        self.read_exact(&mut b)?;
        Ok(b)
    }
    fn read_maybe_vec_in<A: Allocator>(&mut self, alloc: A) -> IoResult<Option<Vec<u8, A>>> {
        let n = match read_size(self)? {
            Some(v) => v,
            None => return Ok(None),
        };
        let mut b = Vec::with_capacity_in(n as usize, alloc);
        unsafe { b.set_len(n as usize) };
        self.read_exact(&mut b)?;
        Ok(Some(b))
    }

    #[inline]
    fn read_str(&mut self) -> IoResult<String> {
        self.read_vec_in(Global).map(utf8_to_lossy_owned)
    }
    #[inline]
    fn read_maybe_str(&mut self) -> IoResult<Option<String>> {
        Ok(self.read_maybe_vec_in(Global)?.map(utf8_to_lossy_owned))
    }

    #[inline]
    fn read_fiber(&mut self) -> IoResult<Fiber> {
        self.read_fiber_in(Global)
    }
    #[inline]
    fn read_maybe_fiber(&mut self) -> IoResult<Option<Fiber>> {
        self.read_maybe_fiber_in(Global)
    }
    #[inline]
    fn read_fiber_in<A: Allocator>(&mut self, alloc: A) -> IoResult<Fiber<A>> {
        self.read_vec_in(alloc).map(Fiber::from)
    }
    #[inline]
    fn read_maybe_fiber_in<A: Allocator>(&mut self, alloc: A) -> IoResult<Option<Fiber<A>>> {
        Ok(self.read_maybe_vec_in(alloc)?.map(Fiber::from))
    }

    #[inline]
    fn read_into_bool(&mut self, v: &mut bool) -> IoResult<()> {
        *v = self.read_bool()?;
        Ok(())
    }

    #[inline]
    fn read_into_f32(&mut self, v: &mut f32) -> IoResult<()> {
        *v = self.read_f32()?;
        Ok(())
    }
    #[inline]
    fn read_into_f64(&mut self, v: &mut f64) -> IoResult<()> {
        *v = self.read_f64()?;
        Ok(())
    }

    #[inline]
    fn read_into_i8(&mut self, v: &mut i8) -> IoResult<()> {
        *v = self.read_i8()?;
        Ok(())
    }
    #[inline]
    fn read_into_i16(&mut self, v: &mut i16) -> IoResult<()> {
        *v = self.read_i16()?;
        Ok(())
    }
    #[inline]
    fn read_into_i32(&mut self, v: &mut i32) -> IoResult<()> {
        *v = self.read_i32()?;
        Ok(())
    }
    #[inline]
    fn read_into_i64(&mut self, v: &mut i64) -> IoResult<()> {
        *v = self.read_i64()?;
        Ok(())
    }

    #[inline]
    fn read_into_u8(&mut self, v: &mut u8) -> IoResult<()> {
        *v = self.read_u8()?;
        Ok(())
    }
    #[inline]
    fn read_into_u16(&mut self, v: &mut u16) -> IoResult<()> {
        *v = self.read_u16()?;
        Ok(())
    }
    #[inline]
    fn read_into_u32(&mut self, v: &mut u32) -> IoResult<()> {
        *v = self.read_u32()?;
        Ok(())
    }
    #[inline]
    fn read_into_u64(&mut self, v: &mut u64) -> IoResult<()> {
        *v = self.read_u64()?;
        Ok(())
    }

    #[inline]
    fn read_into_leb128(&mut self, v: &mut u64) -> IoResult<()> {
        *v = self.read_leb128()?;
        Ok(())
    }

    #[inline]
    fn read_into_duration(&mut self, v: &mut Duration) -> IoResult<()> {
        *v = self.read_duration()?;
        Ok(())
    }

    #[inline]
    fn read_into_vec<A: Allocator>(&mut self, v: &mut Vec<u8, A>) -> IoResult<()> {
        v.clear();
        let n = match read_size(self)? {
            Some(v) => v,
            None => return Ok(()),
        };
        v.resize(n as usize, 0);
        self.read_exact(v)?;
        Ok(())
    }

    #[inline]
    fn read_into_str_lossy(&mut self, v: &mut String) -> IoResult<()> {
        let b = unsafe { v.as_mut_vec() };
        self.read_into_vec(b)?;
        utf8_to_lossy_rewrite(b);
        Ok(())
    }
    #[inline]
    fn read_into_fiber_lossy<A: Allocator>(&mut self, v: &mut Fiber<A>) -> IoResult<()> {
        let b = unsafe { v.as_mut_vec() };
        self.read_into_vec(b)?;
        utf8_to_lossy_rewrite(b);
        Ok(())
    }

    #[inline]
    unsafe fn read_into_str(&mut self, v: &mut String) -> IoResult<()> {
        self.read_into_vec(unsafe { v.as_mut_vec() })
    }
    #[inline]
    unsafe fn read_into_fiber<A: Allocator>(&mut self, v: &mut Fiber<A>) -> IoResult<()> {
        self.read_into_vec(unsafe { v.as_mut_vec() })
    }
}
pub trait Writer: Write {
    #[inline]
    fn write_bool(&mut self, v: bool) -> IoResult<()> {
        self.write_u8(if v { 1 } else { 0 })
    }

    #[inline]
    fn write_f32(&mut self, v: f32) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 4 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }
    #[inline]
    fn write_f64(&mut self, v: f64) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 8 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }

    #[inline]
    fn write_i8(&mut self, v: i8) -> IoResult<()> {
        self.write_u8(v as u8)
    }
    #[inline]
    fn write_i16(&mut self, v: i16) -> IoResult<()> {
        self.write_u16(v as u16)
    }
    #[inline]
    fn write_i32(&mut self, v: i32) -> IoResult<()> {
        self.write_u32(v as u32)
    }
    #[inline]
    fn write_i64(&mut self, v: i64) -> IoResult<()> {
        self.write_u64(v as u64)
    }

    #[inline]
    fn write_u8(&mut self, v: u8) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 1 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }
    #[inline]
    fn write_u16(&mut self, v: u16) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 2 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }
    #[inline]
    fn write_u32(&mut self, v: u32) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 4 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }
    #[inline]
    fn write_u64(&mut self, v: u64) -> IoResult<()> {
        if self.write(&v.to_be_bytes())? != 8 {
            return Err(IoError::from(ErrorKind::WriteZero));
        }
        Ok(())
    }

    fn write_leb128(&mut self, v: u64) -> IoResult<()> {
        let (mut n, mut b, mut i) = (v, [0u8; 10], 0usize);
        for _ in 0..10 {
            // BCE as we never go above 10
            unsafe { *b.get_unchecked_mut(i) = (n as u8) & 0x7F };
            n = unsafe { n.unchecked_shr(7) };
            if n == 0 {
                i += 1;
                break;
            }
            unsafe { b.get_unchecked_mut(i).bitor_assign(0x80) };
            i += 1;
        }
        self.write_all(unsafe { b.get_unchecked(0..i) })
    }

    #[inline]
    fn write_duration(&mut self, v: Duration) -> IoResult<()> {
        self.write_u64(v.as_nanos() as u64)
    }

    #[inline]
    fn write_str(&mut self, v: impl AsRef<str>) -> IoResult<()> {
        self.write_bytes(v.as_ref().as_bytes())
    }
    fn write_bytes(&mut self, v: impl AsRef<[u8]>) -> IoResult<()> {
        let b = v.as_ref();
        let _ = write_size(self, b.len())?;
        self.write_all(b)
    }

    #[inline]
    fn write_fiber<A: Allocator>(&mut self, v: &Fiber<A>) -> IoResult<()> {
        self.write_bytes(v.as_bytes())
    }

    #[inline]
    fn write_maybe_str<T: AsRef<str>>(&mut self, v: Option<T>) -> IoResult<()> {
        self.write_bytes(v.as_ref().map_or([0u8; 0].as_slice(), |i| i.as_ref().as_bytes()))
    }
    #[inline]
    fn write_maybe_bytes<T: AsRef<[u8]>>(&mut self, v: Option<T>) -> IoResult<()> {
        self.write_bytes(v.as_ref().map_or([0u8; 0].as_slice(), |i| i.as_ref()))
    }
}
pub trait AllocInto<T, A: Allocator = Global>: Sized {
    fn into_alloc(self, alloc: A) -> T;
}
pub trait AllocFrom<T, A: Allocator = Global>: Sized {
    fn from_alloc(value: T, alloc: A) -> Self;
}
pub trait VecLike<T>: Deref<Target = [T]> + DerefMut<Target = [T]> {
    fn push(&mut self, v: T);
    fn capacity(&self) -> usize;
    fn shrink_to_fit(&mut self);
    fn reserve(&mut self, len: usize);
    fn truncate(&mut self, len: usize);
    fn resize(&mut self, len: usize, v: T);
    fn extend_from_slice(&mut self, other: &[T]);
}

impl<T: Copy, A: Allocator> VecLike<T> for Vec<T, A> {
    #[inline]
    fn push(&mut self, v: T) {
        self.push(v)
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.capacity()
    }
    #[inline]
    fn shrink_to_fit(&mut self) {
        self.shrink_to_fit()
    }
    #[inline]
    fn reserve(&mut self, len: usize) {
        self.reserve(len)
    }
    #[inline]
    fn truncate(&mut self, len: usize) {
        self.truncate(len)
    }
    #[inline]
    fn resize(&mut self, len: usize, v: T) {
        self.resize(len, v);
    }
    #[inline]
    fn extend_from_slice(&mut self, other: &[T]) {
        self.extend_from_slice(other)
    }
}

#[inline]
pub const fn read_u16(src: &[u8]) -> u16 {
    u16::from_be_bytes(if unlikely(src.len() < 2) {
        let mut b = [0u8, 0u8];
        unsafe { copy_nonoverlapping(src.as_ptr(), b.as_mut_ptr(), src.len()) };
        b
    } else {
        unsafe { *(src.as_ptr() as *const u8).cast::<[u8; 2]>() }
    })
}
#[inline]
pub const fn read_u32(src: &[u8]) -> u32 {
    u32::from_be_bytes(if unlikely(src.len() < 4) {
        let mut b = [0u8; 4];
        unsafe { copy_nonoverlapping(src.as_ptr(), b.as_mut_ptr(), src.len()) };
        b
    } else {
        unsafe { *(src.as_ptr() as *const u8).cast::<[u8; 4]>() }
    })
}
#[inline]
pub const fn read_u64(src: &[u8]) -> u64 {
    u64::from_be_bytes(if unlikely(src.len() < 8) {
        let mut b = [0u8; 8];
        unsafe { copy_nonoverlapping(src.as_ptr(), b.as_mut_ptr(), src.len()) };
        b
    } else {
        unsafe { *(src.as_ptr() as *const u8).cast::<[u8; 8]>() }
    })
}

#[inline]
pub fn write_full(w: &mut impl Write, b: &[u8]) -> IoResult<()> {
    w.write_all(b)
}
#[inline]
pub fn read_full(r: &mut impl Read, b: &mut [u8]) -> IoResult<()> {
    r.read_exact(b)
}
pub fn write_str_vec<A: Allocator>(w: &mut impl Writer, v: &Vec<String, A>) -> IoResult<()> {
    let _ = write_size(w, v.len())?;
    for i in v.iter() {
        w.write_str(&i)?
    }
    Ok(())
}
pub fn read_str_vec<A: Allocator>(r: &mut impl Reader, v: &mut Vec<String, A>) -> IoResult<()> {
    v.clear();
    let n = match read_size(r)? {
        Some(v) => v,
        None => return Ok(()),
    };
    v.reserve_exact(n as usize);
    for _ in 0..n {
        v.push(r.read_str()?)
    }
    Ok(())
}
pub fn read_fiber_vec<A: Allocator + Clone>(r: &mut impl Reader, v: &mut Vec<Fiber<A>, A>) -> IoResult<()> {
    v.clear();
    let n = match read_size(r)? {
        Some(v) => v,
        None => return Ok(()),
    };
    v.reserve_exact(n as usize);
    let a = v.allocator().clone();
    for _ in 0..n {
        v.push(r.read_fiber_in(a.clone())?);
    }
    Ok(())
}
pub fn write_fiber_vec<A: Allocator, B: Allocator>(w: &mut impl Writer, v: &Vec<Fiber<B>, A>) -> IoResult<()> {
    let _ = write_size(w, v.len());
    for i in v.iter() {
        w.write_str(&i)?
    }
    Ok(())
}
pub fn read_fiber_vec_in<A: Allocator, B: Allocator + Clone>(r: &mut impl Reader, v: &mut Vec<Fiber<B>, A>, alloc: B) -> IoResult<()> {
    v.clear();
    let n = match read_size(r)? {
        Some(v) => v,
        None => return Ok(()),
    };
    v.reserve_exact(n as usize);
    let a = alloc;
    for _ in 0..n {
        v.push(r.read_fiber_in(a.clone())?);
    }
    Ok(())
}

#[cfg(feature = "leb_sizes")]
#[inline]
fn write_size(w: &mut (impl ?Sized + Writer), n: usize) -> IoResult<()> {
    w.write_leb128(n as u64)
}
#[cfg(not(feature = "leb_sizes"))]
#[inline]
fn write_size(w: &mut (impl ?Sized + Writer), n: usize) -> IoResult<()> {
    match n {
        0x0 => w.write_u8(0),
        0x1..=0xFF => {
            w.write_u8(1)?;
            w.write_u8(n as u8)
        },
        0x100..=0xFFFF => {
            w.write_u8(3)?;
            w.write_u16(n as u16)
        },
        0x10000..=0xFFFFFFFF => {
            w.write_u8(5)?;
            w.write_u32(n as u32)
        },
        _ => {
            w.write_u8(7)?;
            w.write_u64(n as u64)
        },
    }
}
#[cfg(feature = "leb_sizes")]
#[inline]
fn read_size(r: &mut (impl ?Sized + Reader)) -> IoResult<Option<usize>> {
    let n = r.read_leb128()? as usize;
    if n == 0 {
        return Ok(None);
    }
    #[cfg(feature = "limit_signed")]
    {
        // Limit all sized buffers to isize::MAX
        // if this feature is disabled, then buffers may extend to usize::MAX
        if n > isize::MAX as usize {
            return Err(IoError::from(ErrorKind::FileTooLarge));
        }
    }
    Ok(Some(n))
}
#[cfg(not(feature = "leb_sizes"))]
#[inline]
fn read_size(r: &mut (impl ?Sized + Reader)) -> IoResult<Option<usize>> {
    let n = match r.read_u8()? {
        0 => return Ok(None),
        1 | 2 => r.read_u8()? as usize,
        3 | 4 => r.read_u16()? as usize,
        5 | 6 => r.read_u32()? as usize,
        7 | 8 => r.read_u64()? as usize,
        _ => return Err(IoError::from(ErrorKind::InvalidData)),
    };
    #[cfg(feature = "limit_signed")]
    {
        // Limit all sized buffers to isize::MAX
        // if this feature is disabled, then buffers may extend to usize::MAX
        if n > isize::MAX as usize {
            return Err(IoError::from(ErrorKind::FileTooLarge));
        }
    }
    Ok(Some(n))
}
