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

use crate::util::stx::io::{self, ErrorKind, Read, Write};
use crate::util::stx::prelude::*;

pub mod blob;
mod chunk;
pub mod crypto;
pub mod time;

pub use self::chunk::Chunk;

pub trait Writable {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()>;
}
pub trait Readable {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()>;
}
pub trait Reader: Read {
    #[inline]
    fn read_f32(&mut self) -> io::Result<f32> {
        let mut b: [u8; 4] = [0; 4];
        if self.read(&mut b)? != 4 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(f32::from_be_bytes(b))
    }
    #[inline]
    fn read_f64(&mut self) -> io::Result<f64> {
        let mut b: [u8; 8] = [0; 8];
        if self.read(&mut b)? != 8 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(f64::from_be_bytes(b))
    }

    #[inline]
    fn read_bool(&mut self) -> io::Result<bool> {
        Ok(self.read_i8()? == 1)
    }

    #[inline]
    fn read_i8(&mut self) -> io::Result<i8> {
        Ok(self.read_u8()? as i8)
    }
    #[inline]
    fn read_i16(&mut self) -> io::Result<i16> {
        Ok(self.read_u16()? as i16)
    }
    #[inline]
    fn read_i32(&mut self) -> io::Result<i32> {
        Ok(self.read_u32()? as i32)
    }
    #[inline]
    fn read_i64(&mut self) -> io::Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    #[inline]
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut b: [u8; 1] = [0; 1];
        if self.read(&mut b)? != 1 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(b[0])
    }
    #[inline]
    fn read_u16(&mut self) -> io::Result<u16> {
        let mut b: [u8; 2] = [0; 2];
        if self.read(&mut b)? != 2 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(u16::from_be_bytes(b))
    }
    #[inline]
    fn read_u32(&mut self) -> io::Result<u32> {
        let mut b: [u8; 4] = [0; 4];
        if self.read(&mut b)? != 4 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(u32::from_be_bytes(b))
    }
    #[inline]
    fn read_u64(&mut self) -> io::Result<u64> {
        let mut b: [u8; 8] = [0; 8];
        if self.read(&mut b)? != 8 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(u64::from_be_bytes(b))
    }

    #[inline]
    fn read_str(&mut self) -> io::Result<Option<String>> {
        Ok(self.read_vec()?.map(|v| String::from_utf8_lossy(&v).to_string()))
    }
    fn read_vec(&mut self) -> io::Result<Option<Vec<u8>>> {
        let n;
        match self.read_u8() {
            Ok(v) => n = v,
            Err(e) => return Err(e),
        };
        let c = match n {
            0 => return Ok(None),
            1 | 2 => self.read_u8()? as isize,
            3 | 4 => self.read_u16()? as isize,
            5 | 6 => self.read_u32()? as isize,
            7 | 8 => self.read_u64()? as isize,
            _ => return Err(ErrorKind::InvalidData.into()),
        };
        if c <= 0 || c >= isize::MAX {
            return Err(ErrorKind::FileTooLarge.into());
        }
        let mut b = vec![0u8; c as usize];
        self.read_exact(&mut b)?;
        Ok(Some(b))
    }

    #[inline]
    fn read_into_bool(&mut self, v: &mut bool) -> io::Result<()> {
        *v = self.read_bool()?;
        Ok(())
    }

    #[inline]
    fn read_into_f32(&mut self, v: &mut f32) -> io::Result<()> {
        *v = self.read_f32()?;
        Ok(())
    }
    #[inline]
    fn read_into_f64(&mut self, v: &mut f64) -> io::Result<()> {
        *v = self.read_f64()?;
        Ok(())
    }

    #[inline]
    fn read_into_i8(&mut self, v: &mut i8) -> io::Result<()> {
        *v = self.read_i8()?;
        Ok(())
    }
    #[inline]
    fn read_into_i16(&mut self, v: &mut i16) -> io::Result<()> {
        *v = self.read_i16()?;
        Ok(())
    }
    #[inline]
    fn read_into_i32(&mut self, v: &mut i32) -> io::Result<()> {
        *v = self.read_i32()?;
        Ok(())
    }
    #[inline]
    fn read_into_i64(&mut self, v: &mut i64) -> io::Result<()> {
        *v = self.read_i64()?;
        Ok(())
    }

    #[inline]
    fn read_into_u8(&mut self, v: &mut u8) -> io::Result<()> {
        *v = self.read_u8()?;
        Ok(())
    }
    #[inline]
    fn read_into_u16(&mut self, v: &mut u16) -> io::Result<()> {
        *v = self.read_u16()?;
        Ok(())
    }
    #[inline]
    fn read_into_u32(&mut self, v: &mut u32) -> io::Result<()> {
        *v = self.read_u32()?;
        Ok(())
    }
    #[inline]
    fn read_into_u64(&mut self, v: &mut u64) -> io::Result<()> {
        *v = self.read_u64()?;
        Ok(())
    }

    #[inline]
    fn read_into_str(&mut self, v: &mut String) -> io::Result<usize> {
        unsafe { self.read_into_vec(v.as_mut_vec()) }
    }
    fn read_into_vec(&mut self, v: &mut Vec<u8>) -> io::Result<usize> {
        v.clear();
        let c = match self.read_u8()? {
            0 => return Ok(0),
            1 | 2 => self.read_u8()? as isize,
            3 | 4 => self.read_u16()? as isize,
            5 | 6 => self.read_u32()? as isize,
            7 | 8 => self.read_u64()? as isize,
            _ => return Err(ErrorKind::InvalidData.into()),
        };
        if c <= 0 || c >= isize::MAX {
            return Err(ErrorKind::FileTooLarge.into());
        }
        v.resize(c as usize, 0);
        self.read_exact(v)?;
        Ok(c as usize)
    }
}
pub trait Writer: Write {
    #[inline]
    fn write_f32(&mut self, v: f32) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 4 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }
    #[inline]
    fn write_f64(&mut self, v: f64) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 8 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }

    #[inline]
    fn write_bool(&mut self, v: bool) -> io::Result<()> {
        self.write_u8(if v { 1 } else { 0 })
    }

    #[inline]
    fn write_i8(&mut self, v: i8) -> io::Result<()> {
        self.write_u8(v as u8)
    }
    #[inline]
    fn write_i16(&mut self, v: i16) -> io::Result<()> {
        self.write_u16(v as u16)
    }
    #[inline]
    fn write_i32(&mut self, v: i32) -> io::Result<()> {
        self.write_u32(v as u32)
    }
    #[inline]
    fn write_i64(&mut self, v: i64) -> io::Result<()> {
        self.write_u64(v as u64)
    }

    #[inline]
    fn write_u8(&mut self, v: u8) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 1 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }
    #[inline]
    fn write_u16(&mut self, v: u16) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 2 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }
    #[inline]
    fn write_u32(&mut self, v: u32) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 4 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }
    #[inline]
    fn write_u64(&mut self, v: u64) -> io::Result<()> {
        if self.write(&v.to_be_bytes())? != 8 {
            return Err(ErrorKind::WriteZero.into());
        }
        Ok(())
    }

    #[inline]
    fn write_str(&mut self, v: &str) -> io::Result<()> {
        self.write_bytes(v.as_bytes())
    }
    fn write_bytes(&mut self, v: &[u8]) -> io::Result<()> {
        let n = v.len();
        match n {
            0x0 => return self.write_u8(0),
            0x1..=0xFF => {
                self.write_u8(1)?;
                self.write_u8(n as u8)?
            },
            0x100..=0xFFFF => {
                self.write_u8(3)?;
                self.write_u16(n as u16)?
            },
            0x10000..=0xFFFFFFFF => {
                self.write_u8(5)?;
                self.write_u32(n as u32)?
            },
            _ => {
                self.write_u8(7)?;
                self.write_u64(n as u64)?
            },
        }
        self.write_all(v)
    }
    #[inline]
    fn write_vec(&mut self, v: &Vec<u8>) -> io::Result<()> {
        self.write_bytes(v.as_slice())
    }
}

#[inline]
pub fn write_full(w: &mut impl Write, buf: &[u8]) -> io::Result<()> {
    w.write_all(buf)
}
#[inline]
pub fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    r.read_exact(buf)
}
pub fn write_str_vec(w: &mut impl Writer, s: &Vec<String>) -> io::Result<()> {
    let n = s.len();
    match n {
        0x0 => return w.write_u8(0),
        0x1..=0xFF => {
            w.write_u8(1)?;
            w.write_u8(n as u8)?
        },
        0x100..=0xFFFF => {
            w.write_u8(3)?;
            w.write_u16(n as u16)?
        },
        0x10000..=0xFFFFFFFF => {
            w.write_u8(5)?;
            w.write_u32(n as u32)?
        },
        _ => {
            w.write_u8(7)?;
            w.write_u64(n as u64)?
        },
    }
    for v in s {
        w.write_str(&v)?
    }
    Ok(())
}
pub fn read_str_vec(r: &mut impl Reader, s: &mut Vec<String>) -> io::Result<()> {
    s.clear();
    let c = match r.read_u8()? {
        0 => return Ok(()),
        1 | 2 => r.read_u8()? as isize,
        3 | 4 => r.read_u16()? as isize,
        5 | 6 => r.read_u32()? as isize,
        7 | 8 => r.read_u64()? as isize,
        _ => return Err(ErrorKind::InvalidData.into()),
    };
    if c <= 0 || c >= isize::MAX {
        return Err(ErrorKind::FileTooLarge.into());
    }
    s.reserve_exact(c as usize);
    for _ in 0..c {
        s.push(r.read_str()?.unwrap_or_default())
    }
    Ok(())
}
