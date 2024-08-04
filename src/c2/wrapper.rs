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

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::matches;

use crate::io::{self, Read, Write};
use crate::prelude::*;

pub enum Wrapper<'a, A: Allocator = Global> {
    None,
    Hex,
    Base64,
    Zlib,
    Gzip,
    XOR(&'a [u8]),
    CBK(u8, u8, u8, u8, u8),
    AES(&'a [u8], &'a [u8]),
    Custom(Box<dyn CustomWrapper>),
    Multiple(Vec<Wrapper<'a, A>, A>),
}

pub trait CustomWrapper {
    fn wrap<'a>(&self, input: &'a mut dyn Write) -> io::Result<&mut (dyn Write + 'a)>;
    fn unwrap<'a>(&self, input: &'a mut dyn Read) -> io::Result<&mut (dyn Read + 'a)>;
}

impl<'a, A: Allocator> Wrapper<'_, A> {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Wrapper::None)
    }
    pub fn wrap(&self, input: &'a mut impl Write) -> io::Result<&mut (dyn Write + 'a)> {
        match self {
            Wrapper::Custom(c) => c.wrap(input),
            _ => Ok(input),
        }
    }
    pub fn unwrap(&self, input: &'a mut impl Read) -> io::Result<&mut (dyn Read + 'a)> {
        match self {
            Wrapper::Custom(c) => c.unwrap(input),
            _ => Ok(input),
        }
    }
}
