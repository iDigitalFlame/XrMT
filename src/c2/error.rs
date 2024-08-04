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

use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::hash::Hasher;
use core::{fmt, result};

use crate::com::{Flag, Packet};
use crate::data::time::Time;
use crate::data::{Fnv64, Writer};
use crate::prelude::*;
use crate::util::ToStrHex;
use crate::{ignore_error, io};

const MASK_R: u32 = 0x80000000u32;
const MASK_K: u32 = 0x40000000u32;

#[derive(Debug)] // TODO(dij):
#[repr(u32)]
// Rust CoreError format.
//
// 32bits
//
// RKxxxxxx|xxxxxxxx|xxxxxxxx|xxxxxxxx
//
//   0 - 29: Error Code Value.
//       30: (K) Bit denotes that this code is the hash of an ErrorKind.
//       31: (R) Bit denotes that this code is a raw error code from the OS.
//
//   R and K both cannot be 1 at the same time, but both can be zero.
pub enum CoreError {
    Closing             = 0x01u32,
    InvalidTask         = 0x02u32,
    InvalidInput        = 0x03u32,
    Disconnected        = 0x04u32,
    UnsupportedOs       = 0x05u32,
    TooManyPackets      = 0x06u32,
    InvalidPacketFrag   = 0x07u32,
    InvalidPacketCount  = 0x08u32,
    InvalidPacketDevice = 0x09u32,
    KillDate(Time)      = 0x0Au32,
    KeysRejected(u32)   = 0x0Bu32,
    InvalidResponse(u8) = 0x0Cu32,

    Io(u32),
    Os(u32),
    Other(String),
}
#[derive(Debug)]
pub enum BufferError {
    Full(Packet),
    Error(CoreError),
}

pub type CoreResult<T> = result::Result<T, CoreError>;
pub type BufferResult<T> = result::Result<T, BufferError>;

impl CoreError {
    #[inline]
    pub fn new(v: impl Display) -> CoreError {
        CoreError::Other(v.to_string())
    }
    #[inline]
    pub fn from_code(v: u32) -> CoreError {
        CoreError::Os(v)
    }

    pub fn write(&self, n: &mut Packet) {
        n.clear();
        n.flags.set(Flag::ERROR);
        let v = match self {
            CoreError::Other(v) => {
                ignore_error!(n.write_str(v));
                return;
            },
            _ => self.as_u32(),
        };
        let mut b = [0u8; 0xF];
        let r = v.into_hex_buf(&mut b);
        // Set signature as Rust-specific error so we know how to translate it.
        if r > 3 {
            (b[r - 3], b[r - 2], b[r - 1]) = (b'R', b'0', b'x');
        }
        ignore_error!(n.write_bytes(&b[r - 3..]));
    }

    #[inline]
    fn as_u32(&self) -> u32 {
        match self {
            CoreError::Closing => 0x01,
            CoreError::InvalidTask => 0x02,
            CoreError::InvalidInput => 0x03,
            CoreError::Disconnected => 0x04,
            CoreError::UnsupportedOs => 0x05,
            CoreError::TooManyPackets => 0x06,
            CoreError::InvalidPacketFrag => 0x07,
            CoreError::InvalidPacketCount => 0x08,
            CoreError::InvalidPacketDevice => 0x09,
            CoreError::KillDate(_) => 0x0A,
            CoreError::KeysRejected(_) => 0x0B,
            CoreError::InvalidResponse(_) => 0x0C,
            CoreError::Io(h) => *h as u32,
            CoreError::Os(h) => (*h) | MASK_R,
            _ => 0,
        }
    }
}
impl BufferError {
    #[inline]
    pub fn unpack(self) -> CoreResult<Packet> {
        match self {
            BufferError::Full(v) => Ok(v),
            BufferError::Error(e) => Err(e),
        }
    }
}

/*
impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {

    }
}*/

impl Error for CoreError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for CoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let _ = f;
        todo!()
    }
}
impl From<io::Error> for CoreError {
    #[inline]
    fn from(v: io::Error) -> CoreError {
        match v.raw_os_error() {
            Some(e) => CoreError::Os(e as u32),
            None => v.kind().into(),
        }
    }
}
impl From<io::ErrorKind> for CoreError {
    #[inline]
    fn from(v: io::ErrorKind) -> CoreError {
        let mut h = Fnv64::new();
        h.write_u32(v as u32);
        CoreError::Io(h.finish() as u32 | MASK_K)
    }
}

impl Display for BufferError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let _ = f;
        todo!()
    }
}
impl From<CoreError> for BufferError {
    #[inline]
    fn from(v: CoreError) -> BufferError {
        BufferError::Error(v)
    }
}
