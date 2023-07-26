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

use alloc::boxed::Box;

use crate::data::blob::Blob;
use crate::util::stx::io::{self, Read, Write};
use crate::util::stx::prelude::*;

pub enum Wrapper {
    None,
    Hex,
    Base64,
    Zlib(u8),
    Gzip(u8),
    XOR(Blob<u8, 64>),
    CBK(u8, u8, u8, u8, u8),
    AES(([u8; 32], [u8; 16])),
    Custom(Box<dyn CustomWrapper>),
}

pub trait CustomWrapper {
    fn wrap<'a>(&mut self, input: &'a mut dyn Write) -> io::Result<&mut (dyn Write + 'a)>;
    fn unwrap<'a>(&mut self, input: &'a mut dyn Read) -> io::Result<&mut (dyn Read + 'a)>;
}

impl Wrapper {
    #[inline]
    pub fn is_none(&self) -> bool {
        match self {
            Wrapper::None => true,
            _ => false,
        }
    }
    pub fn wrap<'a>(&mut self, input: &'a mut impl Write) -> io::Result<&mut (dyn Write + 'a)> {
        match self {
            Wrapper::Custom(c) => c.wrap(input),
            _ => Ok(input),
        }
    }
    pub fn unwrap<'a>(&mut self, input: &'a mut impl Read) -> io::Result<&mut (dyn Read + 'a)> {
        match self {
            Wrapper::Custom(c) => c.unwrap(input),
            _ => Ok(input),
        }
    }
}
