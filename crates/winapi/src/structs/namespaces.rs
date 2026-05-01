// Copyright (C) 2023 - 2025 iDigitalFlame
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

// Thanks to @phra's PEzor project for the insights into the Api struct details.
// https://github.com/phra/PEzor/blob/master/ApiSetMap.c
// https://github.com/phra/PEzor/blob/master/ApiSetMap.h
//

#![no_implicit_prelude]
#![cfg(target_family = "windows")]

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::alloc::{Allocator, Global};
use alloc::string::String;
use core::convert::{From, Into};
use core::iter::{ExactSizeIterator, FusedIterator, Iterator};
use core::ops::FnOnce;
use core::option::Option::{self, None, Some};
use core::slice::from_raw_parts;

use xrmt_data::text::{utf16_match, utf16_to_fiber_in, utf16_to_string};
use xrmt_data::{Blob, Fiber, Slice};

use crate::info::is_min_windows_7;
use crate::structs::{Chars, StringLike, WCharLike, WCharSlice, WChars, PEB};
use crate::utils::copy;

pub enum ApiMap<'a> {
    V2(&'a ApiSetV2),
    V4(&'a ApiSetV4),
    V6(&'a ApiSetV6),
}
pub enum ApiEntry<'a> {
    V2(ApiEntryV2<'a>),
    V4(ApiEntryV4<'a>),
    V6(ApiEntryV6<'a>),
}
pub enum ApiEntryValue<'a> {
    V2(ApiEntryValueV2<'a>),
    V4(ApiEntryValueV4<'a>),
    V6(ApiEntryValueV6<'a>),
}

#[repr(C)]
pub struct ApiSet {
    pub version: u32,
    pub count:   u32,
}
pub struct ApiEntryIter<'a> {
    pos: u32,
    map: &'a ApiMap<'a>,
}
pub struct ApiEntryValueIter<'a> {
    pos:   u32,
    entry: &'a ApiEntry<'a>,
}

// ========================
// API Namespace v2
// Windows 7-8
// ========================
//
// ApiSetV2
// \-> ApiSetEntryV2
//  \-> ApiSetEntryValuesV2
//   \-> ApiSetEntryValueV2
//
pub struct ApiEntryV2<'a> {
    pub entry: &'a ApiSetEntryV2,
    pub map:   *const ApiSetV2,
}
pub struct ApiEntryValueV2<'a> {
    pub entry: &'a ApiSetEntryValueV2,
    pub map:   *const ApiSetV2,
}

#[repr(C)]
pub struct ApiSetV2 {
    pub version: u32,
    pub count:   u32,
    pub array:   [ApiSetEntryV2; 1],
}
#[repr(C)]
pub struct ApiSetEntryV2 {
    pub name_offset: u32,
    pub name_size:   u32,
    pub data_offset: u32,
}
#[repr(C)]
pub struct ApiSetEntryValueV2 {
    pub name_offset:  u32,
    pub name_size:    u32,
    pub value_offset: u32,
    pub value_size:   u32,
}
#[repr(C)]
pub struct ApiSetEntryValuesV2 {
    pub count: u32,
    pub array: [ApiSetEntryValueV2; 1],
}

// ========================
// API Namespace v4
// Windows 8.1-10
// ========================
//
// ApiSetV4
// \-> ApiSetEntryV4
//  \-> ApiSetEntryValuesV4
//   \-> ApiSetEntryValueV4
//
pub struct ApiEntryV4<'a> {
    pub entry: &'a ApiSetEntryV4,
    pub map:   *const ApiSetV4,
}
pub struct ApiEntryValueV4<'a> {
    pub entry: &'a ApiSetEntryValueV4,
    pub map:   *const ApiSetV4,
}

#[repr(C)]
pub struct ApiSetV4 {
    pub version: u32,
    pub size:    u32,
    pub flags:   u32,
    pub count:   u32,
    pub array:   [ApiSetEntryV4; 1],
}
#[repr(C)]
pub struct ApiSetEntryV4 {
    pub flags:        u32,
    pub name_offset:  u32,
    pub name_size:    u32,
    pub alias_offset: u32,
    pub alias_length: u32,
    pub data_offset:  u32,
}
#[repr(C)]
pub struct ApiSetEntryValueV4 {
    pub flags:        u32,
    pub name_offset:  u32,
    pub name_size:    u32,
    pub value_offset: u32,
    pub value_size:   u32,
}
#[repr(C)]
pub struct ApiSetEntryValuesV4 {
    pub flags: u32,
    pub count: u32,
    pub array: [ApiSetEntryValueV4; 1],
}

// ========================
// API Namespace v6
// Windows 10+
// ========================
//
// ApiSetV6
// \-> ApiSetEntryV6
//  \-> ApiSetEntryValueV6
//
pub struct ApiEntryV6<'a> {
    pub entry: &'a ApiSetEntryV6,
    pub map:   *const ApiSetV6,
}
pub struct ApiEntryValueV6<'a> {
    pub entry: &'a ApiSetEntryValueV6,
    pub map:   *const ApiSetV6,
}

#[repr(C)]
pub struct ApiSetV6 {
    pub version:     u32,
    pub size:        u32,
    pub flags:       u32,
    pub count:       u32,
    pub offset_data: u32,
    pub offset_hash: u32,
    pub multiplier:  u32,
    pub array:       [ApiSetEntryV6; 1],
}
#[repr(C)]
pub struct ApiSetEntryV6 {
    pub flags:       u32,
    pub name_offset: u32,
    pub size:        u32,
    pub name_size:   u32,
    pub data_offset: u32,
    pub count:       u32,
}
#[repr(C)]
pub struct ApiSetEntryValueV6 {
    pub flags:        u32,
    pub name_offset:  u32,
    pub name_size:    u32,
    pub value_offset: u32,
    pub value_size:   u32,
}

pub trait ApiName<'a> {
    fn name(&self) -> WCharSlice<'a>;

    #[inline]
    fn name_len(&self) -> usize {
        self.name().len()
    }
    #[inline]
    fn name_as_u8(&self) -> Chars {
        self.name_as_u8_in(Global)
    }
    #[inline]
    fn name_as_u16(&self) -> WChars {
        self.name_as_u16_in(Global)
    }
    #[inline]
    fn name_as_str(&self) -> String {
        utf16_to_string(&self.name())
    }
    #[inline]
    fn name_as_fiber(&self) -> Fiber {
        self.name_as_fiber_in(Global)
    }
    #[inline]
    fn name_as_slice(&self, b: &mut [u16]) -> usize {
        copy(&self.name(), b)
    }
    #[inline]
    fn name_as_u8_slice<const N: usize>(&self) -> Slice<u8, N> {
        Slice::from_utf16(&self.name())
    }
    #[inline]
    fn name_as_u8_in<A: Allocator>(&self, alloc: A) -> Chars<A> {
        Blob::from_utf16_in(&self.name(), alloc)
    }
    #[inline]
    fn name_as_u16_slice<const N: usize>(&self) -> Slice<u16, N> {
        Slice::from(self.name().as_slice())
    }
    #[inline]
    fn name_as_u16_in<A: Allocator>(&self, alloc: A) -> WChars<A> {
        Blob::with_values_in(&self.name(), alloc)
    }
    #[inline]
    fn name_as_fiber_in<A: Allocator>(&self, alloc: A) -> Fiber<A> {
        utf16_to_fiber_in(&self.name(), alloc)
    }
}
pub trait ApiValue<'a> {
    fn value(&self) -> WCharSlice<'a>;

    #[inline]
    fn value_len(&self) -> usize {
        self.value().len()
    }
    #[inline]
    fn value_as_u8(&self) -> Chars {
        self.value_as_u8_in(Global)
    }
    #[inline]
    fn value_as_u16(&self) -> WChars {
        self.value_as_u16_in(Global)
    }
    #[inline]
    fn value_as_str(&self) -> String {
        utf16_to_string(&self.value())
    }
    #[inline]
    fn value_as_fiber(&self) -> Fiber {
        self.value_as_fiber_in(Global)
    }
    #[inline]
    fn value_as_slice(&self, b: &mut [u16]) -> usize {
        copy(&self.value(), b)
    }
    #[inline]
    fn value_as_u8_slice<const N: usize>(&self) -> Slice<u8, N> {
        Slice::from_utf16(&self.value())
    }
    #[inline]
    fn value_as_u8_in<A: Allocator>(&self, alloc: A) -> Chars<A> {
        Blob::from_utf16_in(&self.value(), alloc)
    }
    #[inline]
    fn value_as_u16_slice<const N: usize>(&self) -> Slice<u16, N> {
        Slice::from(self.value().as_slice())
    }
    #[inline]
    fn value_as_u16_in<A: Allocator>(&self, alloc: A) -> WChars<A> {
        Blob::with_values_in(&self.value(), alloc)
    }
    #[inline]
    fn value_as_fiber_in<A: Allocator>(&self, alloc: A) -> Fiber<A> {
        utf16_to_fiber_in(&self.value(), alloc)
    }
}

impl<'a> PEB<'a> {
    #[inline]
    pub fn api_map(&self) -> Option<ApiMap<'a>> {
        if !is_min_windows_7() || self.api_map.is_null() {
            return None;
        }
        Some(match unsafe { (&*self.api_map).version } {
            2 => ApiMap::V2(unsafe { &*(self.api_map as *const ApiSetV2) }),
            4 => ApiMap::V4(unsafe { &*(self.api_map as *const ApiSetV4) }),
            6 => ApiMap::V6(unsafe { &*(self.api_map as *const ApiSetV6) }),
            _ => return None,
        })
    }
}
impl<'a> ApiMap<'a> {
    #[inline]
    pub fn iter(&'a self) -> ApiEntryIter<'a> {
        ApiEntryIter { pos: 0u32, map: self }
    }
    #[inline]
    pub fn find(&'a self, name: impl Into<WCharLike<'a>>) -> Option<String> {
        self.find_func(name, |v| utf16_to_string(&v.value()))
    }
    #[inline]
    pub fn find_as_u8(&self, name: impl Into<WCharLike<'a>>) -> Option<Chars> {
        self.find_as_u8_in(name, Global)
    }
    #[inline]
    pub fn find_as_u16(&self, name: impl Into<WCharLike<'a>>) -> Option<WChars> {
        self.find_as_u16_in(name, Global)
    }
    #[inline]
    pub fn find_as_str(&self, name: impl Into<WCharLike<'a>>) -> Option<String> {
        self.find_func(name, |v| v.value_as_str())
    }
    #[inline]
    pub fn find_as_fiber(&self, name: impl Into<WCharLike<'a>>) -> Option<Fiber> {
        self.find_as_fiber_in(name, Global)
    }
    #[inline]
    pub fn find_as_slice(&self, name: impl Into<WCharLike<'a>>, b: &mut [u16]) -> Option<usize> {
        self.find_func(name, |v| v.value_as_slice(b))
    }
    #[inline]
    pub fn find_as_u8_slice<const N: usize>(&self, name: impl Into<WCharLike<'a>>) -> Option<Slice<u8, N>> {
        self.find_func(name, |v| v.value_as_u8_slice())
    }
    #[inline]
    pub fn find_as_u8_in<A: Allocator>(&self, name: impl Into<WCharLike<'a>>, alloc: A) -> Option<Chars<A>> {
        self.find_func(name, |v| v.value_as_u8_in(alloc))
    }
    #[inline]
    pub fn find_as_u16_slice<const N: usize>(&self, name: impl Into<WCharLike<'a>>) -> Option<Slice<u16, N>> {
        self.find_func(name, |v| v.value_as_u16_slice())
    }
    #[inline]
    pub fn find_as_u16_in<A: Allocator>(&self, name: impl Into<WCharLike<'a>>, alloc: A) -> Option<WChars<A>> {
        self.find_func(name, |v| v.value_as_u16_in(alloc))
    }
    #[inline]
    pub fn find_as_fiber_in<A: Allocator>(&self, name: impl Into<WCharLike<'a>>, alloc: A) -> Option<Fiber<A>> {
        self.find_func(name, |v| v.value_as_fiber_in(alloc))
    }
    #[inline]
    pub fn find_func<T>(&self, name: impl Into<WCharLike<'a>>, f: impl FnOnce(ApiEntryValue) -> T) -> Option<T> {
        let k = name.into();
        for e in self.iter() {
            if !utf16_match(&e.name(), &k, false) {
                continue;
            }
            for s in e.iter() {
                if s.is_default() || utf16_match(&s.name(), &k, false) {
                    return Some(f(s));
                }
            }
            return Some(f(e.value_default()?));
        }
        None
    }
}
impl<'a> ApiEntry<'a> {
    #[inline]
    pub fn flags(&self) -> u32 {
        match self {
            ApiEntry::V2(_) => 0,
            ApiEntry::V4(e) => e.entry.flags,
            ApiEntry::V6(e) => e.entry.flags,
        }
    }
    #[inline]
    pub fn map(&self) -> ApiMap<'a> {
        match self {
            ApiEntry::V2(e) => unsafe { ApiMap::V2(&*e.map) },
            ApiEntry::V4(e) => unsafe { ApiMap::V4(&*e.map) },
            ApiEntry::V6(e) => unsafe { ApiMap::V6(&*e.map) },
        }
    }
    #[inline]
    pub fn iter(&'a self) -> ApiEntryValueIter<'a> {
        ApiEntryValueIter { pos: 0u32, entry: self }
    }
    #[inline]
    pub fn value_default(&self) -> Option<ApiEntryValue<'a>> {
        match self {
            ApiEntry::V2(e) => {
                let a = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValuesV2) };
                if a.count < 1 {
                    return None;
                }
                let x = unsafe { &*(a as *const ApiSetEntryValuesV2 as *const ApiSetEntryValueV2) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V2(ApiEntryValueV2 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
            ApiEntry::V4(e) => {
                let a = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValuesV4) };
                if a.count < 1 {
                    return None;
                }
                let x = unsafe { &*(a as *const ApiSetEntryValuesV4 as *const ApiSetEntryValueV4) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V4(ApiEntryValueV4 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
            ApiEntry::V6(e) => {
                if e.entry.count < 1 {
                    return None;
                }
                let x = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValueV6) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V6(ApiEntryValueV6 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
        }
    }
}
impl<'a> ApiEntryValue<'a> {
    #[inline]
    pub fn flags(&self) -> u32 {
        match self {
            ApiEntryValue::V2(_) => 0,
            ApiEntryValue::V4(e) => e.entry.flags,
            ApiEntryValue::V6(e) => e.entry.flags,
        }
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            ApiEntryValue::V2(e) => e.entry.value_size == 0,
            ApiEntryValue::V4(e) => e.entry.value_size == 0,
            ApiEntryValue::V6(e) => e.entry.value_size == 0,
        }
    }
    #[inline]
    pub fn map(&self) -> ApiMap<'a> {
        match self {
            ApiEntryValue::V2(e) => unsafe { ApiMap::V2(&*e.map) },
            ApiEntryValue::V4(e) => unsafe { ApiMap::V4(&*e.map) },
            ApiEntryValue::V6(e) => unsafe { ApiMap::V6(&*e.map) },
        }
    }
    #[inline]
    pub fn is_default(&self) -> bool {
        match self {
            ApiEntryValue::V2(_) => false,
            ApiEntryValue::V4(_) => false,
            ApiEntryValue::V6(e) => e.entry.name_size == 0,
        }
    }
}

impl<'a> Iterator for ApiEntryIter<'a> {
    type Item = ApiEntry<'a>;

    fn next(&mut self) -> Option<ApiEntry<'a>> {
        let v = match self.map {
            ApiMap::V2(m) => {
                if self.pos >= m.count {
                    return None;
                }
                let e = unsafe { &*(m.array.as_ptr().add(self.pos as usize)) };
                Some(ApiEntry::V2(ApiEntryV2 { entry: e, map: *m }))
            },
            ApiMap::V4(m) => {
                if self.pos >= m.count {
                    return None;
                }
                let e = unsafe { &*(m.array.as_ptr().add(self.pos as usize)) };
                Some(ApiEntry::V4(ApiEntryV4 { entry: e, map: *m }))
            },
            ApiMap::V6(m) => {
                if self.pos >= m.count {
                    return None;
                }
                let e = unsafe { &*(m.array.as_ptr().add(self.pos as usize)) };
                Some(ApiEntry::V6(ApiEntryV6 { entry: e, map: *m }))
            },
        };
        self.pos += 1;
        v
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.map {
            ApiMap::V2(m) => (m.count as usize, Some(m.count as usize)),
            ApiMap::V4(m) => (m.count as usize, Some(m.count as usize)),
            ApiMap::V6(m) => (m.count as usize, Some(m.count as usize)),
        }
    }
}
impl FusedIterator for ApiEntryIter<'_> {}

impl<'a> Iterator for ApiEntryValueIter<'a> {
    type Item = ApiEntryValue<'a>;

    fn next(&mut self) -> Option<ApiEntryValue<'a>> {
        let v = match self.entry {
            ApiEntry::V2(e) => {
                let a = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValuesV2) };
                if self.pos >= a.count {
                    return None;
                }
                let x = unsafe { &*(a as *const ApiSetEntryValuesV2 as *const ApiSetEntryValueV2).add(self.pos as usize) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V2(ApiEntryValueV2 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
            ApiEntry::V4(e) => {
                let a = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValuesV4) };
                if self.pos >= a.count {
                    return None;
                }
                let x = unsafe { &*(a as *const ApiSetEntryValuesV4 as *const ApiSetEntryValueV4).add(self.pos as usize) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V4(ApiEntryValueV4 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
            ApiEntry::V6(e) => {
                if self.pos >= e.entry.count {
                    return None;
                }
                let x = unsafe { &*((e.map as usize + e.entry.data_offset as usize) as *const ApiSetEntryValueV6).add(self.pos as usize) };
                if x.value_size > 0 {
                    Some(ApiEntryValue::V6(ApiEntryValueV6 {
                        entry: x,
                        map:   e.map,
                    }))
                } else {
                    None
                }
            },
        };
        self.pos += 1;
        v
    }
}
impl FusedIterator for ApiEntryValueIter<'_> {}
impl<'a> ExactSizeIterator for ApiEntryIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        match self.map {
            ApiMap::V2(m) => m.count as usize,
            ApiMap::V4(m) => m.count as usize,
            ApiMap::V6(m) => m.count as usize,
        }
    }
}

impl<'a> ApiName<'a> for ApiEntry<'a> {
    #[inline]
    fn name_len(&self) -> usize {
        match self {
            ApiEntry::V2(e) => e.entry.name_size as usize / 2,
            ApiEntry::V4(e) => e.entry.name_size as usize / 2,
            ApiEntry::V6(e) => e.entry.name_size as usize / 2,
        }
    }
    #[inline]
    fn name(&self) -> WCharSlice<'a> {
        match self {
            ApiEntry::V2(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
            ApiEntry::V4(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
            ApiEntry::V6(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
        }
    }
}
impl<'a> ApiName<'a> for ApiEntryValue<'a> {
    #[inline]
    fn name_len(&self) -> usize {
        match self {
            ApiEntryValue::V2(e) => e.entry.name_size as usize / 2,
            ApiEntryValue::V4(e) => e.entry.name_size as usize / 2,
            ApiEntryValue::V6(e) => e.entry.name_size as usize / 2,
        }
    }
    #[inline]
    fn name(&self) -> WCharSlice<'a> {
        match self {
            ApiEntryValue::V2(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
            ApiEntryValue::V4(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
            ApiEntryValue::V6(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.name_offset as usize) as *const u16,
                    e.entry.name_size as usize / 2,
                ))
            },
        }
    }
}
impl<'a> ApiValue<'a> for ApiEntryValue<'a> {
    #[inline]
    fn value_len(&self) -> usize {
        match self {
            ApiEntryValue::V2(e) => e.entry.value_size as usize / 2,
            ApiEntryValue::V4(e) => e.entry.value_size as usize / 2,
            ApiEntryValue::V6(e) => e.entry.value_size as usize / 2,
        }
    }
    #[inline]
    fn value(&self) -> WCharSlice<'a> {
        match self {
            ApiEntryValue::V2(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.value_offset as usize) as *const u16,
                    e.entry.value_size as usize / 2,
                ))
            },
            ApiEntryValue::V4(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.value_offset as usize) as *const u16,
                    e.entry.value_size as usize / 2,
                ))
            },
            ApiEntryValue::V6(e) => unsafe {
                WCharSlice::from(from_raw_parts(
                    (e.map as usize + e.entry.value_offset as usize) as *const u16,
                    e.entry.value_size as usize / 2,
                ))
            },
        }
    }
}
