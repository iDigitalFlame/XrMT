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
#![cfg(windows)]

use super::{Filter, FilterError, FilterFunc};
use crate::device::rand::Rand;
use crate::device::winapi::{self, OwnedHandle, ProcessEntry, Win32Error};
use crate::util::stx::prelude::*;

impl Filter {
    #[inline]
    pub fn select(&self) -> Result<u32, FilterError> {
        self.select_func(None)
    }
    #[inline]
    pub fn token(&self, access: u32) -> Result<OwnedHandle, FilterError> {
        self.token_func(access, None)
    }
    #[inline]
    pub fn thread(&self, access: u32) -> Result<OwnedHandle, FilterError> {
        self.thread_func(access, None)
    }
    #[inline]
    pub fn handle(&self, access: u32) -> Result<OwnedHandle, FilterError> {
        self.handle_func(access, None)
    }
    #[inline]
    pub fn select_func(&self, func: FilterFunc) -> Result<u32, FilterError> {
        if self.pid > 4 && func.is_none() {
            return Ok(self.pid);
        }
        // 0x400 - PROCESS_QUERY_LIMITED_INFORMATION
        Ok(self.open(0x1000, false, func)?.process_id)
    }
    #[inline]
    pub fn token_func(&self, access: u32, func: FilterFunc) -> Result<OwnedHandle, FilterError> {
        // 0x400 - PROCESS_QUERY_INFORMATION
        Ok(winapi::OpenProcessToken(self.handle_func(0x400, func)?, access).map_err(FilterError::from)?)
    }
    pub fn thread_func(&self, access: u32, func: FilterFunc) -> Result<OwnedHandle, FilterError> {
        let p = self.select_func(func)?;
        let _ = winapi::acquire_privilege(winapi::SE_DEBUG_PRIVILEGE); // IGNORE ERROR
        for e in winapi::list_threads(p).map_err(FilterError::from)? {
            if let Ok(h) = e.handle(access) {
                return Ok(h);
            }
        }
        Err(FilterError::NoProcessFound)
    }
    pub fn handle_func(&self, access: u32, func: FilterFunc) -> Result<OwnedHandle, FilterError> {
        if self.pid <= 4 {
            return self.open(access, false, func)?.handle(access).map_err(FilterError::from);
        }
        let _ = winapi::acquire_privilege(winapi::SE_DEBUG_PRIVILEGE); // IGNORE ERROR
        let h = winapi::OpenProcess(access, false, self.pid).map_err(FilterError::from)?;
        if let Some(f) = func {
            let n = winapi::GetProcessFileName(&h).map_err(FilterError::from)?;
            // 0x20008 - TOKEN_READ | TOKEN_QUERY
            let r = f(
                self.pid,
                winapi::is_token_elevated(winapi::OpenProcessToken(&h, 0x20008).map_err(FilterError::from)?),
                &n,
                h.0,
            );
            return if r { Ok(h) } else { Err(FilterError::NoProcessFound) };
        }
        Ok(h)
    }

    fn open(&self, access: u32, retry: bool, func: FilterFunc) -> Result<ProcessEntry, FilterError> {
        let p = winapi::GetCurrentProcessID();
        let (s, v) = (
            self.session > Filter::EMPTY,
            self.elevated > Filter::EMPTY || func.is_some(), /* TODO(dij): <- Why is this here?
                                                              * The Go version also has it? Is it a cross-platform bug? */
        );
        let mut r = Rand::new();
        let mut z: Vec<ProcessEntry> = Vec::with_capacity(64);
        let _ = winapi::acquire_privilege(winapi::SE_DEBUG_PRIVILEGE); // IGNORE ERROR
        for e in winapi::list_processes().map_err(FilterError::from)? {
            if e.process_id == p || e.process_id < 5 || e.name.is_empty() || (self.pid > 0 && self.pid != e.process_id) {
                continue;
            }
            if (!self.exclude.is_empty() && in_list(&e.name, &self.exclude)) || (!self.include.is_empty() && !in_list(&e.name, &self.include)) {
                continue;
            }
            if (func.is_none() && !s && !v) || retry {
                if e.handle(access).is_ok() {
                    z.push(e)
                }
                continue;
            }
            if let Ok((h, k, i)) = e.info_ex(access, v, s, func.is_some()) {
                if v && ((k && self.elevated == Filter::FALSE) || (!k && self.elevated == Filter::TRUE)) {
                    continue;
                }
                if s && ((i > 0 && self.session == Filter::FALSE) || (i == 0 && self.session == Filter::TRUE)) {
                    continue;
                }
                if func.map_or(true, |f| f(e.process_id, k, &e.name, h.map_or(0, |v| v.0))) {
                    z.push(e);
                }
            }
        }
        if z.is_empty() {
            return if !retry && func.is_none() && self.fallback {
                self.open(access, true, func)
            } else {
                Err(FilterError::NoProcessFound)
            };
        }
        if z.len() == 1 {
            Ok(z[0].clone())
        } else {
            Ok(z[r.rand_u32n(z.len() as u32) as usize].clone())
        }
    }
}

impl From<Win32Error> for FilterError {
    #[inline]
    fn from(v: Win32Error) -> FilterError {
        FilterError::FindError(v.to_string())
    }
}

#[inline]
fn in_list(m: &str, c: &[String]) -> bool {
    for i in c {
        if m.eq_ignore_ascii_case(&i) {
            return true;
        }
    }
    return false;
}
