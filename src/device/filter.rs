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
use core::fmt::{self, Debug, Display, Formatter};

use crate::data::{self, Readable, Reader, Writable, Writer};
use crate::util::stx::io::{self, ErrorKind};
use crate::util::stx::prelude::*;

pub enum FilterError {
    NoProcessFound,
    FindError(String),
}

#[cfg_attr(not(feature = "implant"), derive(Debug))]
pub struct Filter {
    pub exclude:  Vec<String>,
    pub include:  Vec<String>,
    pub pid:      u32,
    pub fallback: bool,
    pub session:  u8,
    pub elevated: u8,
}

pub trait Boolean {
    fn as_boolean(&self) -> u8;
}
pub trait MaybeFilter {
    fn into_filter(self) -> Option<Filter>;
}
pub trait ToFilter: Into<Filter> {}

pub type FilterFunc = Option<fn(u32, bool, &str, usize) -> bool>;

impl Filter {
    pub const TRUE: u8 = 2;
    pub const FALSE: u8 = 1;
    pub const EMPTY: u8 = 0;

    #[inline]
    pub const fn empty() -> Filter {
        Filter {
            pid:      0,
            exclude:  Vec::new(),
            include:  Vec::new(),
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        }
    }
    #[inline]
    pub const fn with_fallback(fallback: bool) -> Filter {
        Filter {
            fallback,
            pid: 0,
            exclude: Vec::new(),
            include: Vec::new(),
            session: Filter::EMPTY,
            elevated: Filter::EMPTY,
        }
    }

    #[inline]
    pub fn with_target(proc: impl AsRef<str>) -> Filter {
        let mut f = Filter {
            pid:      0,
            exclude:  Vec::new(),
            include:  Vec::new(),
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        };
        f.include.push(proc.as_ref().to_string());
        f
    }
    #[inline]
    pub fn with_include(include: Vec<String>) -> Filter {
        include.into()
    }
    #[inline]
    pub fn with_exclude(exclude: Vec<String>) -> Filter {
        Filter {
            pid:      0,
            exclude:  exclude.clone(),
            include:  Vec::new(),
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.pid = 0;
        self.session = Filter::EMPTY;
        self.elevated = Filter::EMPTY;
        self.include.clear();
        self.exclude.clear();
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pid == 0 && self.session == Filter::EMPTY && self.elevated == Filter::EMPTY && self.include.is_empty() && self.exclude.is_empty()
    }
    #[inline]
    pub fn pid(&mut self, pid: u32) -> &mut Filter {
        self.pid = pid;
        self
    }
    #[inline]
    pub fn fallback(&mut self, fallback: bool) -> &mut Filter {
        self.fallback = fallback;
        self
    }
    #[inline]
    pub fn include(&mut self, proc: impl AsRef<str>) -> &mut Filter {
        self.include.push(proc.as_ref().to_string());
        self
    }
    #[inline]
    pub fn exclude(&mut self, proc: impl AsRef<str>) -> &mut Filter {
        self.exclude.push(proc.as_ref().to_string());
        self
    }
    #[inline]
    pub fn session(&mut self, session: impl Boolean) -> &mut Filter {
        self.session = session.as_boolean();
        self
    }
    #[inline]
    pub fn elevated(&mut self, elevated: impl Boolean) -> &mut Filter {
        self.elevated = elevated.as_boolean();
        self
    }
}

impl Boolean for i8 {
    #[inline]
    fn as_boolean(&self) -> u8 {
        match self {
            0 => Filter::EMPTY,
            _ if *self < 0 => Filter::FALSE,
            _ => Filter::TRUE,
        }
    }
}
impl Boolean for u8 {
    #[inline]
    fn as_boolean(&self) -> u8 {
        if *self > Filter::TRUE {
            Filter::TRUE
        } else {
            *self
        }
    }
}
impl Boolean for bool {
    #[inline]
    fn as_boolean(&self) -> u8 {
        if *self {
            Filter::TRUE
        } else {
            Filter::FALSE
        }
    }
}
impl Boolean for Option<u8> {
    #[inline]
    fn as_boolean(&self) -> u8 {
        self.map_or(Filter::EMPTY, |v| {
            if v > Filter::TRUE {
                Filter::TRUE
            } else {
                v
            }
        })
    }
}
impl Boolean for Option<bool> {
    #[inline]
    fn as_boolean(&self) -> u8 {
        self.map_or(
            Filter::EMPTY,
            |v| if v { Filter::TRUE } else { Filter::FALSE },
        )
    }
}

impl Eq for Filter {}
impl Clone for Filter {
    #[inline]
    fn clone(&self) -> Filter {
        Filter {
            pid:      self.pid,
            session:  self.session,
            exclude:  self.exclude.clone(),
            include:  self.include.clone(),
            fallback: self.fallback,
            elevated: self.elevated,
        }
    }
}
impl Default for Filter {
    #[inline]
    fn default() -> Filter {
        Filter::empty()
    }
}
impl Writable for Filter {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        if self.is_empty() {
            return w.write_bool(false);
        }
        w.write_bool(true)?;
        w.write_u32(self.pid)?;
        w.write_bool(self.fallback)?;
        w.write_u8(self.session)?;
        w.write_u8(self.elevated)?;
        data::write_str_vec(w, &self.exclude)?;
        data::write_str_vec(w, &self.include)
    }
}
impl Readable for Filter {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        if !r.read_bool()? {
            return Ok(());
        }
        r.read_into_u32(&mut self.pid)?;
        r.read_into_bool(&mut self.fallback)?;
        r.read_into_u8(&mut self.session)?;
        r.read_into_u8(&mut self.elevated)?;
        data::read_str_vec(r, &mut self.exclude)?;
        data::read_str_vec(r, &mut self.include)
    }
}
impl PartialEq for Filter {
    #[inline]
    fn eq(&self, other: &Filter) -> bool {
        self.fallback == other.fallback && self.pid == other.pid && self.session == other.session && self.elevated == other.elevated && self.exclude == other.exclude && self.include == other.include
    }
}

impl MaybeFilter for &str {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        Some(self.into())
    }
}
impl MaybeFilter for String {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        Some(self.into())
    }
}
impl MaybeFilter for &Filter {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        Some(self.clone())
    }
}
impl MaybeFilter for &mut Filter {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        Some(self.clone())
    }
}

impl Writable for Option<Filter> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        match self {
            Some(f) => f.write_stream(w),
            None => w.write_bool(false),
        }
    }
}
impl Readable for Option<Filter> {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        if !r.read_bool()? {
            *self = None;
            return Ok(());
        }
        let mut f = Filter::empty();
        f.read_stream(r)?;
        *self = Some(f);
        Ok(())
    }
}
impl MaybeFilter for Option<Filter> {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        self
    }
}

impl Error for FilterError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for FilterError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for FilterError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FilterError::FindError(v) => f.write_str(v),
            FilterError::NoProcessFound => f.write_str(if cfg!(feature = "implant") {
                "0x404"
            } else {
                "no process found"
            }),
        }
    }
}

impl<T: ToFilter> MaybeFilter for T {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        Some(self.into())
    }
}

impl From<&str> for Filter {
    #[inline]
    fn from(v: &str) -> Filter {
        vec![v.to_string()].into()
    }
}
impl From<String> for Filter {
    #[inline]
    fn from(v: String) -> Filter {
        vec![v].into()
    }
}
impl From<Vec<String>> for Filter {
    #[inline]
    fn from(v: Vec<String>) -> Filter {
        Filter {
            pid:      0,
            exclude:  Vec::new(),
            include:  v,
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        }
    }
}

impl From<FilterError> for io::Error {
    #[inline]
    fn from(v: FilterError) -> io::Error {
        match v {
            FilterError::NoProcessFound => ErrorKind::NotFound.into(),
            FilterError::FindError(m) => io::Error::new(io::ErrorKind::Other, m),
        }
    }
}

#[cfg(unix)]
#[path = "filter/unix.rs"]
mod inner;
#[cfg(windows)]
#[path = "filter/windows.rs"]
mod inner;
