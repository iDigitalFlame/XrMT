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

use crate::io::{self, Write};
use crate::prelude::*;

pub enum Transform<'a, A: Allocator = Global> {
    None,
    Base64(u8),
    DNS(Vec<&'a str, A>),
    Custom(Box<dyn CustomTransform>),
}

pub trait CustomTransform {
    fn read(&self, input: &[u8], output: &mut dyn Write) -> io::Result<()>;
    fn write(&self, input: &[u8], output: &mut dyn Write) -> io::Result<()>;
}

impl<A: Allocator> Transform<'_, A> {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Transform::None)
    }
    pub fn read(&self, input: &[u8], output: &mut impl Write) -> io::Result<()> {
        match self {
            Transform::Custom(c) => c.read(input, output),
            _ => output.write_all(input),
        }
    }
    pub fn write(&self, input: &[u8], output: &mut impl Write) -> io::Result<()> {
        match self {
            Transform::Custom(c) => c.write(input, output),
            _ => output.write_all(input),
        }
    }
}
