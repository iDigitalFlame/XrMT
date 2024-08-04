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
#![cfg(not(target_family = "windows"))]

use core::alloc::Allocator;

use crate::prelude::*;
use crate::process::filter::{Filter, FilterError, FilterFunc};

// TODO(dij): Now that we have the ability to view processes on all systems
//            we should revisit this on non-Windows systems as we can atleast
//            support the reterival of PIDs from names.

impl<A: Allocator> Filter<A> {
    #[inline]
    pub fn select(&self) -> Result<u32, FilterError> {
        self.select_func(None)
    }
    #[inline]
    pub fn select_func(&self, _func: FilterFunc) -> Result<u32, FilterError> {
        if self.pid > 0 {
            Ok(self.pid)
        } else {
            Err(FilterError::NoProcessFound)
        }
    }
}
