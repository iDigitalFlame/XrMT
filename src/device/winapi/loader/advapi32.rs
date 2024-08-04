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
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::{Function, Loader};

pub(crate) static LsaClose: Function = Function::new();
pub(crate) static LsaOpenPolicy: Function = Function::new();
pub(crate) static LsaQueryInformationPolicy: Function = Function::new();

// pub(crate) static IsWellKnownSID: Function = Function::new(); // Forward to
// kernelbase.dll in Win7+

// TODO(dij): Do we need this now?
pub(crate) static LookupAccountSid: Function = Function::new();
pub(crate) static LookupPrivilegeValue: Function = Function::new(); // TODO(dij): Might not be needed with predefined SID privs.

pub(crate) static SystemFunction036: Function = Function::new(); // Forward to cryptbase.dll -> bcrypt.dll in Win7+
pub(crate) static InitiateSystemShutdownEx: Function = Function::new();
pub(crate) static ConvertStringSecurityDescriptorToSecurityDescriptor: Function = Function::new();

pub(crate) static LogonUser: Function = Function::new(); // Loads sspicli.dll / secur32.dll
pub(crate) static CreateProcessWithLogon: Function = Function::new();
pub(crate) static CreateProcessWithToken: Function = Function::new();

pub(crate) static SetServiceStatus: Function = Function::new();
pub(crate) static StartServiceCtrlDispatcher: Function = Function::new();
pub(crate) static RegisterServiceCtrlHandlerEx: Function = Function::new();
pub(crate) static QueryServiceDynamicInformation: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|advapi32| {
    advapi32.proc(&LsaClose, 0xB9C1C829);
    advapi32.proc(&LsaOpenPolicy, 0x34D221F9);
    advapi32.proc(&LsaQueryInformationPolicy, 0xD67C4D8B);

    // advapi32.proc(&IsWellKnownSID, 0xF855936A);

    advapi32.proc(&LookupAccountSid, 0x59E27333);
    advapi32.proc(&LookupPrivilegeValue, 0xEC6FF8D6);

    advapi32.proc(&SystemFunction036, 0x7FD1A2D9); // This is a forwarded function.
    advapi32.proc(&InitiateSystemShutdownEx, 0xDA8731DD);
    advapi32.proc(
        &ConvertStringSecurityDescriptorToSecurityDescriptor,
        0x9EF78621,
    );

    advapi32.proc(&LogonUser, 0x5BAC4A5A);
    advapi32.proc(&CreateProcessWithLogon, 0x62F9BC50);
    advapi32.proc(&CreateProcessWithToken, 0xC20739FE);

    advapi32.proc(&SetServiceStatus, 0xC09B613A);
    advapi32.proc(&StartServiceCtrlDispatcher, 0x99A279E7);
    advapi32.proc(&RegisterServiceCtrlHandlerEx, 0x5046FA66);
    advapi32.proc(&QueryServiceDynamicInformation, 0x2F5CB537);
});
