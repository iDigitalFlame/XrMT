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
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::{Function, Loader};

// NOTE(dij): AmsiScanString calls AmsiScanBuffer, but we're keeping this action
//            as it may change in the future? I need to check Win11 amsi.dll.
// TODO(dij): ^
pub(crate) static AmsiScanString: Function = Function::new();
pub(crate) static AmsiScanBuffer: Function = Function::new();
pub(crate) static AmsiInitialize: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|amsi| {
    amsi.proc(&AmsiScanString, 0x18AB3DF);
    amsi.proc(&AmsiScanBuffer, 0x7AB1BB42);
    amsi.proc(&AmsiInitialize, 0xBFB2E53D);
});
