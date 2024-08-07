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

use core::alloc::Allocator;

use crate::data::crypto::hmac;
use crate::data::rand::system_rand;
use crate::data::{read_full, write_full, Readable, Reader, Writable, Writer};
use crate::ignore_error;
use crate::io::{self, Read, Write};
use crate::prelude::*;
use crate::util::crypt;

pub struct ID([u8; 32]);

impl ID {
    pub const SIZE: u8 = 32u8;
    pub const MACHINE_SIZE: u8 = 28u8;

    #[inline]
    pub const fn new() -> ID {
        ID([0u8; 32])
    }

    #[inline]
    pub fn hash(&self) -> u32 {
        let mut h = 0x811C9DC5u32;
        for i in self.0 {
            h = h.wrapping_mul(0x1000193);
            h ^= i as u32;
        }
        h
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
    #[inline]
    pub fn write(&self, w: &mut impl Write) -> io::Result<()> {
        write_full(w, &self.0)
    }
    #[inline]
    pub fn read(&mut self, r: &mut impl Read) -> io::Result<()> {
        read_full(r, &mut self.0)
    }
}

impl Eq for ID {}
impl Copy for ID {}
impl Clone for ID {
    #[inline]
    fn clone(&self) -> ID {
        ID(self.0)
    }
}
impl Default for ID {
    #[inline]
    fn default() -> ID {
        ID::new()
    }
}
impl Writable for ID {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        write_full(w, &self.0)
    }
}
impl Readable for ID {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        read_full(r, &mut self.0)
    }
}
impl PartialEq for ID {
    #[inline]
    fn eq(&self, other: &ID) -> bool {
        self.0 == other.0
    }
}
impl<A: Allocator> From<Option<Vec<u8, A>>> for ID {
    fn from(v: Option<Vec<u8, A>>) -> ID {
        let mut i = match v {
            Some(x) => ID(hmac(&x, crypt::get_or(0, "framework-v7").as_bytes())),
            None => {
                let mut i = ID::new();
                ignore_error!(system_rand(&mut i.0));
                i
            },
        };
        ignore_error!(system_rand(&mut i.0[ID::MACHINE_SIZE as usize + 1..]));
        if i.0[0] == 0 {
            i.0[0] = 1;
        }
        if i.0[ID::MACHINE_SIZE as usize] == 0 {
            i.0[ID::MACHINE_SIZE as usize] = 1;
        }
        i
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, LowerHex, UpperHex};

    use crate::device::ID;
    use crate::prelude::*;
    use crate::util::HEXTABLE;

    impl ID {
        #[inline]
        pub fn full(&self) -> String {
            self.string(0, ID::SIZE)
        }
        #[inline]
        pub fn signature(&self) -> String {
            self.string(0, ID::MACHINE_SIZE)
        }

        #[inline]
        fn string(&self, start: u8, end: u8) -> String {
            let mut b = String::with_capacity(((end - start) * 2) as usize);
            let v = unsafe { b.as_mut_vec() };
            for i in start..end {
                if self.0[i as usize] < 16 {
                    v.push(b'0');
                    v.push(HEXTABLE[(self.0[i as usize] as usize) & 0x0F]);
                } else {
                    v.push(HEXTABLE[(self.0[i as usize] as usize) >> 4]);
                    v.push(HEXTABLE[(self.0[i as usize] as usize) & 0x0F]);
                }
            }
            b
        }
    }

    impl Debug for ID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for ID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            if self.0[ID::MACHINE_SIZE as usize] == 0 {
                f.write_str(&self.string(0, ID::MACHINE_SIZE))
            } else {
                f.write_str(&self.string(ID::MACHINE_SIZE, ID::SIZE))
            }
        }
    }
    impl UpperHex for ID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.hash(), f)
        }
    }
    impl LowerHex for ID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            LowerHex::fmt(&self.hash(), f)
        }
    }
}
