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

use core::cmp::Ordering;

use crate::data::{Readable, Reader, Writable, Writer};
use crate::prelude::*;
use crate::{io, number_like};

number_like!(Flag, u64);

pub struct Flag(pub u64);

impl Flag {
    pub const FRAG: Flag = Flag(0x1u64);
    pub const MULTI: Flag = Flag(0x2u64);
    pub const PROXY: Flag = Flag(0x4u64);
    pub const ERROR: Flag = Flag(0x8u64);
    pub const CHANNEL: Flag = Flag(0x10u64);
    pub const CHANNEL_END: Flag = Flag(0x20u64);
    pub const ONESHOT: Flag = Flag(0x40u64);
    pub const MULTI_DEVICE: Flag = Flag(0x80u64); // NOTE(dij): Should we infer MULTI?
    pub const CRYPT: Flag = Flag(0x100u64);

    #[inline]
    pub const fn new() -> Flag {
        Flag(0u64)
    }

    #[inline]
    pub fn from_be_bytes(v: [u8; 8]) -> Flag {
        Flag(u64::from_be_bytes(v))
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = ((self.0 as u16) as u64) ^ Flag::FRAG.0
    }
    #[inline]
    pub fn len(&self) -> u16 {
        (self.0 >> 48) as u16
    }
    #[inline]
    pub fn group(&self) -> u16 {
        (self.0 >> 16) as u16
    }
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn position(&self) -> u16 {
        (self.0 >> 32) as u16
    }
    #[inline]
    pub fn set(&mut self, flag: Flag) {
        self.0 |= flag.0
    }
    #[inline]
    pub fn unset(&mut self, flag: Flag) {
        self.0 ^= flag.0
    }
    #[inline]
    pub fn set_len(&mut self, len: u16) {
        self.0 = (len as u64) << 48 | (self.position() as u64) << 32 | (self.0 as u32) as u64 | Flag::FRAG.0
    }
    #[inline]
    pub fn to_be_bytes(&self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
    #[inline]
    pub fn has(&self, other: Flag) -> bool {
        self.0 & other.0 > 0
    }
    #[inline]
    pub fn set_group(&mut self, group: u16) {
        self.0 = ((self.0 >> 32) << 32) | (group as u64) << 16 | (self.0 as u16) as u64 | Flag::FRAG.0
    }
    #[inline]
    pub fn set_position(&mut self, pos: u16) {
        self.0 = ((self.len() as u64) << 48) | (pos as u64) << 32 | (self.0 as u32) as u64 | Flag::FRAG.0
    }
}

impl Eq for Flag {}
impl Ord for Flag {
    #[inline]
    fn cmp(&self, other: &Flag) -> Ordering {
        let (m, o) = (self.position(), other.position());
        if m == o {
            Ordering::Equal
        } else if m > 0 {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}
impl Default for Flag {
    #[inline]
    fn default() -> Flag {
        Flag::new()
    }
}
impl Writable for Flag {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_u64(self.0)
    }
}
impl Readable for Flag {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        r.read_into_u64(&mut self.0)
    }
}
impl PartialOrd for Flag {
    #[inline]
    fn partial_cmp(&self, other: &Flag) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use crate::com::Flag;
    use crate::prelude::*;
    use crate::util::{ToStr, ToStrHex};

    impl Debug for Flag {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Flag").field(&self.0).finish()
        }
    }
    impl Display for Flag {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let (mut b, mut n) = ([0u8; 27], 0usize);
            if self.has(Flag::FRAG) {
                b[n] = b'F';
                n += 1;
            }
            if self.has(Flag::MULTI) {
                b[n] = b'M';
                n += 1;
            }
            if self.has(Flag::PROXY) {
                b[n] = b'P';
                n += 1;
            }
            if self.has(Flag::ERROR) {
                b[n] = b'E';
                n += 1;
            }
            if self.has(Flag::CHANNEL) {
                b[n] = b'C';
                n += 1;
            }
            if self.has(Flag::CHANNEL_END) {
                b[n] = b'K';
                n += 1;
            }
            if self.has(Flag::ONESHOT) {
                b[n] = b'O';
                n += 1;
            }
            if self.has(Flag::MULTI_DEVICE) {
                b[n] = b'X';
                n += 1;
            }
            if self.has(Flag::CRYPT) {
                b[n] = b'Z';
                n += 1;
            }
            if n == 0 {
                b[0] = b'V';
                n += 1 + self.0.into_hex_buf(&mut b[n + 1..]);
            }
            if self.has(Flag::MULTI) && self.len() > 0 {
                b[n] = b'[';
                n += 1 + self.len().into_buf(&mut b[n + 1..]);
                b[n] = b']';
                n += 1;
            } else if self.has(Flag::FRAG) && !self.has(Flag::MULTI) {
                if self.len() == 0 {
                    b[n] = b'[';
                    n += 1 + self.group().into_hex_buf(&mut b[n + 1..]);
                    b[n] = b']';
                    n += 1;
                } else {
                    b[n] = b'[';
                    n += 1 + self.group().into_hex_buf(&mut b[n + 1..]);
                    b[n] = b':';
                    n += 1 + (self.position() + 1).into_buf(&mut b[n + 1..]);
                    b[n] = b'/';
                    n += 1 + self.len().into_buf(&mut b[n + 1..]);
                    b[n] = b']';
                    n += 1;
                }
            }
            f.write_str(unsafe { core::str::from_utf8_unchecked(&b[0..n]) })
        }
    }
    impl UpperHex for Flag {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
    impl LowerHex for Flag {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.0, f)
        }
    }
}
