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

pub(crate) static CertCloseStore: Function = Function::new();
pub(crate) static CertGetNameString: Function = Function::new();
pub(crate) static CertFindCertificateInStore: Function = Function::new();
pub(crate) static CertFreeCertificateContext: Function = Function::new();

pub(crate) static CryptMsgClose: Function = Function::new();
pub(crate) static CryptMsgGetParam: Function = Function::new();
pub(crate) static CryptQueryObject: Function = Function::new();

pub(super) static DLL: Loader = Loader::new(|crypt32| {
    crypt32.proc(&CertCloseStore, 0xF614DAC4);
    crypt32.proc(&CertGetNameString, 0x3F6B7692);
    crypt32.proc(&CertFindCertificateInStore, 0x38707435);
    crypt32.proc(&CertFreeCertificateContext, 0x6F27DE27);

    crypt32.proc(&CryptMsgClose, 0x9B5720EA);
    crypt32.proc(&CryptMsgGetParam, 0xEE8C1C55);
    crypt32.proc(&CryptQueryObject, 0xEAEDD248);
});
