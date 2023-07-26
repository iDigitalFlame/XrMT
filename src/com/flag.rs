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
use core::ops::{Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, Sub, SubAssign};

use crate::data::{Readable, Reader, Writable, Writer};
use crate::util::stx::io;
use crate::util::stx::prelude::*;

pub struct Flag(pub u64);

impl Flag {
    pub const FRAG: Flag = Flag(0x1);
    pub const MULTI: Flag = Flag(0x2);
    pub const PROXY: Flag = Flag(0x4);
    pub const ERROR: Flag = Flag(0x8);
    pub const CHANNEL: Flag = Flag(0x10);
    pub const CHANNEL_END: Flag = Flag(0x20);
    pub const ONESHOT: Flag = Flag(0x40);
    pub const MULTI_DEVICE: Flag = Flag(0x80);
    pub const CRYPT: Flag = Flag(0x100);

    #[inline]
    pub const fn new() -> Flag {
        Flag(0)
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
impl Add for Flag {
    type Output = Flag;

    #[inline]
    fn add(self, rhs: Flag) -> Flag {
        Flag(self.0 | rhs.0)
    }
}
impl Sub for Flag {
    type Output = Flag;

    #[inline]
    fn sub(self, rhs: Flag) -> Flag {
        Flag(self.0 ^ rhs.0)
    }
}
impl Copy for Flag {}
impl Deref for Flag {
    type Target = u64;

    #[inline]
    fn deref(&self) -> &u64 {
        &self.0
    }
}
impl BitOr for Flag {
    type Output = Flag;

    #[inline]
    fn bitor(self, rhs: Flag) -> Flag {
        Flag(self.0 | rhs.0)
    }
}
impl Clone for Flag {
    #[inline]
    fn clone(&self) -> Flag {
        Flag(self.0)
    }
}
impl BitXor for Flag {
    type Output = Flag;

    #[inline]
    fn bitxor(self, rhs: Flag) -> Flag {
        Flag(self.0 ^ rhs.0)
    }
}
impl BitAnd for Flag {
    type Output = Flag;

    #[inline]
    fn bitand(self, rhs: Flag) -> Flag {
        Flag(self.0 & rhs.0)
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
impl AddAssign for Flag {
    #[inline]
    fn add_assign(&mut self, rhs: Flag) {
        self.0 |= rhs.0
    }
}
impl SubAssign for Flag {
    #[inline]
    fn sub_assign(&mut self, rhs: Flag) {
        self.0 ^= rhs.0
    }
}
impl PartialEq for Flag {
    #[inline]
    fn eq(&self, other: &Flag) -> bool {
        self.0 == other.0
    }
}
impl PartialOrd for Flag {
    #[inline]
    fn partial_cmp(&self, other: &Flag) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl BitOrAssign for Flag {
    #[inline]
    fn bitor_assign(&mut self, rhs: Flag) {
        self.0 |= rhs.0
    }
}
impl BitAndAssign for Flag {
    #[inline]
    fn bitand_assign(&mut self, rhs: Flag) {
        self.0 &= rhs.0
    }
}
impl BitXorAssign for Flag {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Flag) {
        self.0 ^= rhs.0
    }
}

#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use super::Flag;
    use crate::util::stx::prelude::*;
    use crate::util::{ToStr, ToStrHex};

    impl Debug for Flag {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for Flag {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let (mut b, mut n) = ([0u8; 27], 0);
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
                n += 1 + self.len().into_hex_buf(&mut b[n + 1..]);
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
