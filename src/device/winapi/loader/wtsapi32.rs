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

pub(crate) static WTSOpenServer: Function = Function::new();
pub(crate) static WTSCloseServer: Function = Function::new();
pub(crate) static WTSSendMessage: Function = Function::new();
pub(crate) static WTSLogoffSession: Function = Function::new();
pub(crate) static WTSEnumerateSessions: Function = Function::new();
pub(crate) static WTSDisconnectSession: Function = Function::new();
pub(crate) static WTSEnumerateProcesses: Function = Function::new();
pub(crate) static WTSQuerySessionInformation: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|wtsapi32| {
    wtsapi32.proc(&WTSOpenServer, 0xFE2B3B89);
    wtsapi32.proc(&WTSCloseServer, 0x1BCAB670);
    wtsapi32.proc(&WTSSendMessage, 0xACD5E389);
    wtsapi32.proc(&WTSLogoffSession, 0xE355D47E);
    wtsapi32.proc(&WTSEnumerateSessions, 0x81A0698B);
    wtsapi32.proc(&WTSDisconnectSession, 0x9A352247);
    wtsapi32.proc(&WTSEnumerateProcesses, 0x9BC0257D);
    wtsapi32.proc(&WTSQuerySessionInformation, 0xCEFF39A);
});
