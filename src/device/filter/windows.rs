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
#![cfg(target_family = "windows")]

use core::alloc::Allocator;

use crate::data::rand::Rand;
use crate::data::str::Fiber;
use crate::device::winapi::{self, OwnedHandle, ProcessItem, Win32Error};
use crate::prelude::*;
use crate::process::filter::{Filter, FilterError, FilterFunc};
use crate::process::{FilterHandle, FilterResult};

impl<A: Allocator> Filter<A> {
    #[inline]
    pub fn select(&self) -> FilterResult<u32> {
        self.select_func(None)
    }
    #[inline]
    pub fn token(&self, access: u32) -> FilterResult<FilterHandle> {
        self.token_func(access, None)
    }
    #[inline]
    pub fn thread(&self, access: u32) -> FilterResult<FilterHandle> {
        self.thread_func(access, None)
    }
    #[inline]
    pub fn handle(&self, access: u32) -> FilterResult<FilterHandle> {
        self.handle_func(access, None)
    }
    #[inline]
    pub fn select_func(&self, func: FilterFunc) -> FilterResult<u32> {
        if self.is_empty() {
            return Err(FilterError::NoProcessFound);
        }
        if self.pid > 4 && func.is_none() {
            return Ok(self.pid);
        }
        // 0x400 - PROCESS_QUERY_LIMITED_INFORMATION
        Ok(self.open(0x1000, false, func, false)?.0.pid)
    }
    #[inline]
    pub fn token_func(&self, access: u32, func: FilterFunc) -> FilterResult<FilterHandle> {
        if self.is_empty() {
            Err(FilterError::NoProcessFound)
        } else {
            // 0x400 - PROCESS_QUERY_INFORMATION
            winapi::OpenProcessToken(self.handle_func(0x400, func)?, access).map_err(FilterError::from)
        }
    }
    pub fn thread_func(&self, access: u32, func: FilterFunc) -> FilterResult<FilterHandle> {
        let p = self.select_func(func)?;
        winapi::acquire_debug();
        let r = winapi::list_threads(p).or_else(|e| {
            winapi::release_debug();
            Err(e)
        })?;
        for e in r {
            if let Ok(h) = e.handle(access) {
                winapi::release_debug();
                return Ok(h);
            }
        }
        winapi::release_debug();
        Err(FilterError::NoProcessFound)
    }
    pub fn handle_func(&self, access: u32, func: FilterFunc) -> FilterResult<FilterHandle> {
        if self.is_empty() {
            return Err(FilterError::NoProcessFound);
        }
        if self.pid <= 4 {
            return Ok(self.open(access, false, func, true)?.1);
        }
        winapi::acquire_debug();
        let h = winapi::OpenProcess(access, false, self.pid).or_else(|e| {
            winapi::release_debug();
            Err(e)
        })?;
        let r = if let Some(f) = func {
            let n = winapi::GetProcessFileName(&h)?;
            // 0x20008 - TOKEN_READ | TOKEN_QUERY
            let r = f(
                self.pid,
                winapi::is_token_elevated(winapi::OpenProcessToken(&h, 0x20008)?),
                &n,
                h.0,
            );
            if r {
                Ok(h)
            } else {
                Err(FilterError::NoProcessFound)
            }
        } else {
            Ok(h)
        };
        winapi::release_debug();
        r
    }

    fn open(&self, access: u32, retry: bool, func: FilterFunc, handle: bool) -> FilterResult<(ProcessItem, OwnedHandle)> {
        let p = winapi::GetCurrentProcessID();
        let (s, v) = (
            self.session > Filter::EMPTY,
            self.elevated > Filter::EMPTY || func.is_some(), /* TODO(dij): <- Why is this here?
                                                              * The Go version also has it? Is it a cross-platform bug? */
        );
        let mut r = Rand::new();
        let mut z: Vec<(ProcessItem, OwnedHandle)> = Vec::with_capacity(64);
        winapi::acquire_debug();
        let w = winapi::list_processes().map_err(FilterError::from).or_else(|e| {
            winapi::release_debug();
            Err(e)
        })?;
        for e in w {
            if e.pid == p || e.pid < 5 || e.name.is_empty() || (self.pid > 0 && self.pid != e.pid) {
                continue;
            }
            if (!self.exclude.is_empty() && in_list(&e.name, &self.exclude)) || (!self.include.is_empty() && !in_list(&e.name, &self.include)) {
                continue;
            }
            if (func.is_none() && !s && !v) || retry {
                let x = ok_or_continue!(e.handle(access));
                bugtrack!(
                    "process::filter::Filter::open(): Added process e.name={}, e.pid={} for eval.",
                    e.name,
                    e.pid
                );
                z.push((e, x));
                continue;
            }
            if let Ok((h, k, i)) = e.info_ex(access, v, s, func.is_some() || handle) {
                if v && ((k && self.elevated == Filter::FALSE) || (!k && self.elevated == Filter::TRUE)) {
                    continue;
                }
                if s && ((i > 0 && self.session == Filter::FALSE) || (i == 0 && self.session == Filter::TRUE)) {
                    continue;
                }
                if func.map_or(true, |f| {
                    f(e.pid, k, &e.name, h.as_ref().map_or(0, |v| v.0))
                }) {
                    bugtrack!(
                        "process::filter::Filter::open(): Added process e.name={}, e.pid={} for eval.",
                        e.name,
                        e.pid
                    );
                    z.push((e, h.unwrap_or_default()));
                }
            }
        }
        winapi::release_debug();
        if z.is_empty() {
            return if !retry && func.is_none() && self.fallback {
                bugtrack!("process::filter::Filter::open(): First run failed, starting fallback run!");

                self.open(access, true, func, handle)
            } else {
                Err(FilterError::NoProcessFound)
            };
        }
        let r = if z.len() == 1 {
            z.remove(0)
        } else {
            z.remove(r.rand_u32n(z.len() as u32) as usize)
        };
        bugtrack!(
            "process::filter::Filter::open(): Returning process e.name={}, e.pid={}, handle={:?}.",
            r.0.name,
            r.0.pid,
            r.1
        );
        Ok(r)
    }
}

impl From<Win32Error> for FilterError {
    #[inline]
    fn from(v: Win32Error) -> FilterError {
        FilterError::OsError(v.code() as i32)
    }
}

#[inline]
fn in_list<A: Allocator>(m: &str, c: &[Fiber<A>]) -> bool {
    for i in c {
        if m.eq_ignore_ascii_case(&i) {
            return true;
        }
    }
    return false;
}
