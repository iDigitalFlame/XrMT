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

#![no_implicit_prelude]
#![cfg(target_family = "windows")]
#![allow(non_snake_case)]

extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_crypt;
extern crate xrmt_winapi_fnv;

use xrmt_crypt::crypt;

crate::dll!(
    Dbghelp,
    DBGHELP,
    dbghelp,
    || crypt!(0, "dgbhelp.dll"),
    MiniDumpWriteDump
);
crate::dll!(
    Iphlpapi,
    IPHLPAPI,
    iphlpapi,
    || crypt!(0, "iphlpapi.dll"),
    GetAdaptersAddresses
);
crate::dll!(
    WinHTTP,
    WINHTTP,
    winhttp,
    || crypt!(0, "winhttp.dll"),
    WinHTTPGetDefaultProxyConfiguration
);
crate::dll!(
    Amsi,
    AMSI,
    amsi,
    || crypt!(0, "amsi.dll"),
    AmsiInitialize,
    AmsiScanBuffer,
    AmsiScanString
);

crate::dll!(
    DnsAPi,
    DNSAPI,
    dnsapi,
    || crypt!(0, "dnsapi.dll"),
    DnsQueryExW /* TODO(dij): This isn't loaded yet as we're unsure if it's needed.
                 *            Probally when we switch to AFD networking, we'll use it? */
);
