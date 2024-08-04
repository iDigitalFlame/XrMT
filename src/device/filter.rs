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
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};

use crate::data::str::Fiber;
use crate::data::{read_fiber_vec, write_fiber_vec, Readable, Reader, Writable, Writer};
use crate::io::{self, ErrorKind};
use crate::prelude::*;

pub enum FilterError {
    NoProcessFound,
    OsError(i32),
    FindError(String),
}

pub struct Filter<A: Allocator = Global> {
    pub exclude:  Vec<Fiber<A>, A>,
    pub include:  Vec<Fiber<A>, A>,
    pub pid:      u32,
    pub fallback: bool,
    pub session:  u8,
    pub elevated: u8,
}

pub trait Boolean {
    fn as_bool(&self) -> u8;
}
pub trait ToFilter: Into<Filter> {}
pub trait MaybeFilter<A: Allocator = Global> {
    fn into_filter(self) -> Option<Filter<A>>;
}

#[cfg(target_family = "windows")]
pub type FilterHandle = crate::device::winapi::OwnedHandle;
#[cfg(not(target_family = "windows"))]
pub type FilterHandle = usize;

pub type FilterResult<T> = Result<T, FilterError>;
pub type FilterFunc = Option<fn(u32, bool, &str, usize) -> bool>;

impl Filter {
    pub const TRUE: u8 = 2u8;
    pub const FALSE: u8 = 1u8;
    pub const EMPTY: u8 = 0u8;

    #[inline]
    pub const fn empty() -> Filter {
        Filter::with_fallback(false)
    }
    #[inline]
    pub const fn with_fallback(fallback: bool) -> Filter {
        Filter {
            pid: 0u32,
            exclude: Vec::new(),
            include: Vec::new(),
            session: Filter::EMPTY,
            elevated: Filter::EMPTY,
            fallback,
        }
    }

    #[inline]
    pub fn with_target(proc: impl AsRef<str>) -> Filter {
        let mut f = Filter::with_fallback(false);
        f.include.push(proc.as_ref().into());
        f
    }
    #[inline]
    pub fn with_include(include: Vec<impl AsRef<str>>) -> Filter {
        Filter {
            pid:      0u32,
            exclude:  Vec::new(),
            include:  Fiber::convert_vec(include),
            session:  Filter::EMPTY,
            elevated: Filter::EMPTY,
            fallback: false,
        }
    }
    #[inline]
    pub fn with_exclude(exclude: Vec<impl AsRef<str>>) -> Filter {
        Filter {
            pid:      0u32,
            exclude:  Fiber::convert_vec(exclude),
            include:  Vec::new(),
            session:  Filter::EMPTY,
            elevated: Filter::EMPTY,
            fallback: false,
        }
    }

    #[inline]
    pub fn from_reader(r: &mut impl Reader) -> io::Result<Option<Filter>> {
        Filter::from_reader_in(r, Global)
    }
}
impl<A: Allocator> Filter<A> {
    #[inline]
    pub fn clear(&mut self) {
        self.pid = 0u32;
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
    pub fn pid(&mut self, pid: u32) -> &mut Filter<A> {
        self.pid = pid;
        self
    }
    #[inline]
    pub fn fallback(&mut self, fallback: bool) -> &mut Filter<A> {
        self.fallback = fallback;
        self
    }
    #[inline]
    pub fn session(&mut self, session: impl Boolean) -> &mut Filter<A> {
        self.session = session.as_bool();
        self
    }
    #[inline]
    pub fn elevated(&mut self, elevated: impl Boolean) -> &mut Filter<A> {
        self.elevated = elevated.as_bool();
        self
    }
}
impl<A: Allocator + Clone> Filter<A> {
    #[inline]
    pub fn empty_in(alloc: A) -> Filter<A> {
        Filter::with_fallback_in(false, alloc)
    }
    #[inline]
    pub fn with_fallback_in(fallback: bool, alloc: A) -> Filter<A> {
        Filter {
            pid: 0u32,
            exclude: Vec::new_in(alloc.clone()),
            include: Vec::new_in(alloc),
            session: Filter::EMPTY,
            elevated: Filter::EMPTY,
            fallback,
        }
    }
    #[inline]
    pub fn with_target_in(proc: impl AsRef<str>, alloc: A) -> Filter<A> {
        let mut f = Filter::with_fallback_in(false, alloc.clone());
        f.include.push(proc.as_ref().into_alloc(alloc.clone()));
        f
    }
    #[inline]
    pub fn with_include_in<B: Allocator>(include: Vec<impl AsRef<str>, B>, alloc: A) -> Filter<A> {
        Filter {
            pid:      0u32,
            exclude:  Vec::new_in(alloc.clone()),
            include:  Fiber::convert_vec_in(include, alloc),
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        }
    }
    #[inline]
    pub fn with_exclude_in<B: Allocator>(exclude: Vec<impl AsRef<str>, B>, alloc: A) -> Filter<A> {
        Filter {
            pid:      0u32,
            exclude:  Fiber::convert_vec_in(exclude, alloc.clone()),
            include:  Vec::new_in(alloc),
            session:  Filter::EMPTY,
            fallback: false,
            elevated: Filter::EMPTY,
        }
    }

    #[inline]
    pub fn from_reader_in(r: &mut impl Reader, alloc: A) -> io::Result<Option<Filter<A>>> {
        if !r.read_bool()? {
            return Ok(None);
        }
        let mut f = Filter::empty_in(alloc);
        f.read_stream(r)?;
        Ok(Some(f))
    }

    #[inline]
    pub fn include(&mut self, proc: impl AsRef<str>) -> &mut Filter<A> {
        self.include
            .push(proc.as_ref().into_alloc(self.include.allocator().clone()));
        self
    }
    #[inline]
    pub fn exclude(&mut self, proc: impl AsRef<str>) -> &mut Filter<A> {
        self.exclude
            .push(proc.as_ref().into_alloc(self.exclude.allocator().clone()));
        self
    }
}

impl Boolean for i8 {
    #[inline]
    fn as_bool(&self) -> u8 {
        match self {
            0 => Filter::EMPTY,
            _ if *self < 0 => Filter::FALSE,
            _ => Filter::TRUE,
        }
    }
}
impl Boolean for u8 {
    #[inline]
    fn as_bool(&self) -> u8 {
        if *self > Filter::TRUE {
            Filter::TRUE
        } else {
            *self
        }
    }
}
impl Boolean for bool {
    #[inline]
    fn as_bool(&self) -> u8 {
        if *self {
            Filter::TRUE
        } else {
            Filter::FALSE
        }
    }
}
impl Boolean for Option<u8> {
    #[inline]
    fn as_bool(&self) -> u8 {
        self.map_or(Filter::EMPTY, |v| v.into())
    }
}
impl Boolean for Option<bool> {
    #[inline]
    fn as_bool(&self) -> u8 {
        self.map_or(Filter::EMPTY, |v| v.into())
    }
}

impl Default for Filter {
    #[inline]
    fn default() -> Filter {
        Filter::empty()
    }
}
impl<A: Allocator + Eq> Eq for Filter<A> {}
impl<A: Allocator> Writable for Filter<A> {
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        if self.is_empty() {
            return w.write_bool(false);
        }
        w.write_bool(true)?;
        w.write_u32(self.pid)?;
        w.write_bool(self.fallback)?;
        w.write_u8(self.session)?;
        w.write_u8(self.elevated)?;
        write_fiber_vec(w, &self.exclude)?;
        write_fiber_vec(w, &self.include)
    }
}
impl<A: Allocator + Clone> Clone for Filter<A> {
    #[inline]
    fn clone(&self) -> Filter<A> {
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
impl<A: Allocator + Clone> Readable for Filter<A> {
    #[inline]
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        if !r.read_bool()? {
            return Ok(());
        }
        r.read_into_u32(&mut self.pid)?;
        r.read_into_bool(&mut self.fallback)?;
        r.read_into_u8(&mut self.session)?;
        r.read_into_u8(&mut self.elevated)?;
        read_fiber_vec(r, &mut self.exclude)?;
        read_fiber_vec(r, &mut self.include)
    }
}
impl<A: Allocator + PartialEq> PartialEq for Filter<A> {
    #[inline]
    fn eq(&self, other: &Filter<A>) -> bool {
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
impl<A: Allocator + Clone> MaybeFilter<A> for Filter<A> {
    #[inline]
    fn into_filter(self) -> Option<Filter<A>> {
        Some(self)
    }
}
impl<A: Allocator + Clone> MaybeFilter<A> for &Filter<A> {
    #[inline]
    fn into_filter(self) -> Option<Filter<A>> {
        Some(self.clone())
    }
}
impl<A: Allocator + Clone> MaybeFilter<A> for &mut Filter<A> {
    #[inline]
    fn into_filter(self) -> Option<Filter<A>> {
        Some(self.clone())
    }
}

impl MaybeFilter for Option<Filter> {
    #[inline]
    fn into_filter(self) -> Option<Filter> {
        self
    }
}
impl<A: Allocator> Writable for Option<Filter<A>> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        match self {
            Some(f) => f.write_stream(w),
            None => w.write_bool(false),
        }
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
            FilterError::NoProcessFound => Display::fmt(&ErrorKind::NotFound, f),
            FilterError::OsError(c) => Display::fmt(&io::Error::from_raw_os_error(*c), f),
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
        let mut r = Filter::empty();
        r.include.push(v.into());
        r
    }
}
impl From<String> for Filter {
    #[inline]
    fn from(v: String) -> Filter {
        v.as_str().into()
    }
}
impl<A: Allocator + Clone> From<Fiber<A>> for Filter<A> {
    #[inline]
    fn from(v: Fiber<A>) -> Filter<A> {
        let mut r = Filter::empty_in(v.allocator().clone());
        r.include.push(v);
        r
    }
}
impl<A: Allocator + Clone> From<Vec<String, A>> for Filter<A> {
    #[inline]
    fn from(v: Vec<String, A>) -> Filter<A> {
        let a = v.allocator().clone();
        Filter::with_include_in(v, a)
    }
}
impl<A: Allocator + Clone> AllocFrom<Vec<String>, A> for Filter<A> {
    #[inline]
    fn from_alloc(v: Vec<String>, alloc: A) -> Filter<A> {
        Filter::with_include_in(v, alloc)
    }
}

impl From<FilterError> for io::Error {
    #[inline]
    fn from(v: FilterError) -> io::Error {
        match v {
            FilterError::NoProcessFound => ErrorKind::NotFound.into(),
            FilterError::OsError(c) => io::Error::from_raw_os_error(c),
            FilterError::FindError(m) => io::Error::new(ErrorKind::Other, m),
        }
    }
}

#[cfg(target_family = "windows")]
#[path = "filter/windows.rs"]
mod inner;
#[cfg(not(target_family = "windows"))]
#[path = "filter/unix.rs"]
mod inner;

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Formatter};

    use crate::prelude::*;
    use crate::process::filter::Filter;

    impl Debug for Filter {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("Filter")
                .field("exclude", &self.exclude)
                .field("include", &self.include)
                .field("pid", &self.pid)
                .field("fallback", &self.fallback)
                .field("session", &self.session)
                .field("elevated", &self.elevated)
                .finish()
        }
    }
}
