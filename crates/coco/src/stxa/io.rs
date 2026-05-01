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

extern crate alloc;
extern crate core;

extern crate xrmt_stx;

use alloc::string::String;
use core::convert::From;
use core::marker::Sized;
use core::mem::MaybeUninit;
use core::result::Result::{Err, Ok};

pub use xrmt_stx::io::*;

mod read;
mod write;

pub use self::read::*;
pub use self::write::*;

const BASE_BUF_SIZE: usize = if cfg!(target_os = "espidf") { 0x200 } else { 0x2000 };

pub trait AsyncSeek {
    async fn async_seek(&mut self, pos: SeekFrom) -> IoResult<u64>;

    #[inline]
    async fn async_rewind(&mut self) -> IoResult<()> {
        self.async_seek(SeekFrom::Start(0)).await?;
        Ok(())
    }
    #[inline]
    async fn async_stream_len(&mut self) -> IoResult<u64> {
        let o = self.async_stream_position().await?;
        let n = self.async_seek(SeekFrom::End(0)).await?;
        if o != n {
            self.async_seek(SeekFrom::Start(o)).await?;
        }
        Ok(n)
    }
    #[inline]
    async fn async_stream_position(&mut self) -> IoResult<u64> {
        self.async_seek(SeekFrom::Current(0)).await
    }
    #[inline]
    async fn async_seek_relative(&mut self, offset: i64) -> IoResult<()> {
        self.async_seek(SeekFrom::Current(offset)).await?;
        Ok(())
    }
}

#[inline]
pub async fn async_read_to_string<T: AsyncRead>(mut r: T) -> IoResult<String> {
    let mut b = String::new();
    r.async_read_to_string(&mut b).await?;
    Ok(b)
}
#[inline]
pub async fn async_copy(r: &mut (impl ?Sized + AsyncRead), w: &mut (impl ?Sized + AsyncWrite)) -> IoResult<u64> {
    let v = &mut [MaybeUninit::uninit(); BASE_BUF_SIZE];
    let (mut b, mut n) = (BorrowedBuf::from(v.as_mut_slice()), 0u64);
    loop {
        match r.async_read_buf(b.unfilled()).await {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
            Ok(()) => (),
        };
        if b.filled().is_empty() {
            break;
        }
        n += b.filled().len() as u64;
        w.async_write_all(b.filled()).await?;
        b.clear();
    }
    Ok(n)
}
