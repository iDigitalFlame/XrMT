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

use core::net::SocketAddr;
use core::{mem, ptr};

use crate::device::winapi::loader::wtsapi32;
use crate::device::winapi::{self, Session, SessionHandle, SessionProcess, UnicodeString, WCharPtr, Win32Result, SID, WIN_TIME_EPOCH};
use crate::util::stx::prelude::*;

#[repr(C)]
pub struct SockAddr {
    pub family: u16,
    pub port:   u16,
    pub addr4:  u32,      // IPv4 Address or IPv6 FlowInfo
    pub addr6:  [u8; 16], // IPv6 Address
    pub scope:  u32,
}
#[repr(C)]
pub struct QuotaLimit {
    pub paged_pool_limit:     isize,
    pub non_paged_pool_limit: isize,
    pub min_working_set:      isize,
    pub max_working_set:      isize,
    pub page_file_limit:      isize,
    pub time_limit:           i64,
}
#[repr(C)]
pub struct LsaAttributes {
    pub length:     u32,
    pad1:           usize,
    pad2:           *const UnicodeString,
    pub attributes: u32,
    pad3:           [usize; 2],
}
#[repr(C)]
pub struct LsaAccountDomainInfo {
    pub domain: UnicodeString,
    pub sid:    SID,
}

#[repr(C)]
pub(super) struct FDSet {
    pub count: u32,
    pub array: [usize; 64],
}
#[repr(C)]
pub(super) struct SockInfo {
    pad1:               [u32; 19],
    pub address_family: u32,
    pad2:               [u32; 2],
    pub socket_type:    u32,
    pub protocol:       u32,
    pad3:               [u32; 5],
    pad4:               [u16; 256],
}
#[repr(C)]
pub(super) struct WTSProcess {
    pub session_id: u32,
    pid:            u32,
    name:           WCharPtr,
    sid:            SID,
}
#[repr(C)]
pub(super) struct WTSSession {
    session_id: u32,
    station:    WCharPtr,
    state:      u32,
}
#[repr(C)]
pub(super) struct SockBuffer {
    pub len: u32,
    pub buf: SockPointer,
}
#[repr(C, packed)]
pub(super) struct FilePipeWait {
    // This struct is packed as we get weird padding issues with the name value.
    // Name is a ANYSIZE array that follows this struct. Use with Blob instead.
    pub timeout:           i64,
    pub name_length:       u32,
    pub timeout_specified: u8,
}
#[repr(C)]
pub(super) struct IoStatusBlock {
    pub status: usize,
    pub info:   usize,
}
#[repr(C, packed)]
pub(super) struct FileLinkInformation {
    // This struct is packed as we get weird padding issues with the name value.
    // Name is a ANYSIZE array that follows this struct. Use with Blob instead.
    pub replace:        u32,
    pub pad:            u32,
    pub root_directory: usize,
    pub name_length:    u32,
}
#[repr(C)]
pub(super) struct RegKeyValuePartialInfo {
    pub index:      u32,
    pub value_type: u32,
    pub length:     u32,
}

#[repr(C)]
pub(super) union SockPointer {
    pub read:  *mut u8,
    pub write: *const u8,
}

#[repr(C)]
struct WTSInfo {
    pad1:       u32,
    pad2:       [u64; 3],
    pad3:       [u16; 32],
    domain:     [u16; 17],
    user:       [u16; 21],
    pad4:       [u64; 2],
    last_input: i64,
    time_login: i64,
    time_now:   i64,
}
#[repr(C)]
struct WTSAddress {
    family: u32,
    addr:   [u8; 16],
    pad1:   u32,
}

impl SockAddr {
    #[inline]
    pub fn convert(v: &SocketAddr) -> (SockAddr, u32) {
        match v {
            SocketAddr::V4(ref a) => (
                SockAddr {
                    family: 0x2,
                    port:   u16::from_be(a.port()),
                    addr4:  u32::from_le_bytes(a.ip().octets()),
                    addr6:  [0; 16],
                    scope:  0,
                },
                0x10,
            ),
            SocketAddr::V6(ref a) => (
                SockAddr {
                    family: 0x17,
                    port:   u16::from_be(a.port()),
                    addr4:  a.flowinfo(),
                    addr6:  a.ip().octets(),
                    scope:  a.scope_id(),
                },
                0x1C,
            ),
        }
    }
}
impl QuotaLimit {
    #[inline]
    pub fn empty() -> QuotaLimit {
        QuotaLimit {
            time_limit:           0,
            min_working_set:      -1,
            max_working_set:      -1,
            page_file_limit:      0,
            paged_pool_limit:     0,
            non_paged_pool_limit: 0,
        }
    }
}
impl WTSProcess {
    #[inline]
    pub(super) fn into_inner(&self) -> SessionProcess {
        SessionProcess {
            pid:        self.pid,
            name:       self.name.to_string(),
            user:       self.sid.user().unwrap_or_default(),
            session_id: self.session_id,
        }
    }
}
impl WTSSession {
    pub(super) fn into_inner(&self, h: &SessionHandle, win_7: bool) -> Win32Result<Session> {
        let mut x = Session {
            id:         self.session_id,
            addr:       [0; 16],
            host:       self.station.to_string(),
            user:       String::new(),
            status:     self.state as u8,
            domain:     String::new(),
            is_remote:  false,
            login_time: 0,
            last_input: 0,
        };
        let mut p = 0u32;
        let func = unsafe {
            winapi::make_syscall!(
                *wtsapi32::WTSQuerySessionInformation,
                extern "stdcall" fn(usize, u32, u32, *mut *mut u8, *mut u32) -> u32
            )
        };
        if win_7 {
            let mut v = ptr::null_mut();
            if func(h.0, x.id, 0x18, &mut v, &mut p) == 0 {
                return Err(winapi::last_error());
            }
            if let Some(i) = unsafe { (v as *mut WTSInfo).as_ref() } {
                x.user = winapi::utf16_to_str_trim(&i.user);
                x.domain = winapi::utf16_to_str_trim(&i.domain);
                if i.time_login > 0 {
                    x.login_time = (i.time_login - WIN_TIME_EPOCH) * 100;
                }
                if i.last_input > 0 {
                    x.last_input = (i.last_input - WIN_TIME_EPOCH) * 100;
                } else if i.time_login > 0 {
                    x.last_input = (i.time_now - WIN_TIME_EPOCH) * 100;
                }
            }
            winapi::LocalFree(v);
        } else {
            let mut v = ptr::null_mut();
            // 0x5 - WTSUserName
            if func(h.0, x.id, 0x5, &mut v, &mut p) == 0 {
                return Err(winapi::last_error());
            }
            x.user = WCharPtr::string(v as *const u16);
            winapi::LocalFree(v);
            let mut v = ptr::null_mut();
            // 0x7 - WTSDomainName
            if func(h.0, x.id, 0x7, &mut v, &mut p) == 0 {
                return Err(winapi::last_error());
            }
            x.domain = WCharPtr::string(v as *const u16);
            winapi::LocalFree(v);
        }
        let mut a = ptr::null_mut();
        if func(h.0, x.id, 0xE, &mut a, &mut p) == 0 {
            return Err(winapi::last_error());
        }
        if let Some(i) = unsafe { (a as *mut WTSAddress).as_ref() } {
            match i.family {
                0x2 => {
                    // IPv4
                    x.addr[0..4].copy_from_slice(&i.addr[2..6]);
                    x.is_remote = true;
                },
                0x17 => {
                    // IPv6
                    x.addr.copy_from_slice(&i.addr);
                    x.is_remote = true;
                },
                _ => x.is_remote = false,
            }
        }
        winapi::LocalFree(a);
        Ok(x)
    }
}
impl FilePipeWait {
    #[inline]
    pub fn new(timeout: i32, len: u32) -> FilePipeWait {
        FilePipeWait {
            timeout:           if timeout > 0 {
                timeout as i64 * -10000
            } else if timeout < 0 {
                0x4000000000000000
            } else {
                0
            },
            name_length:       len,
            timeout_specified: if timeout != 0 { 1 } else { 0 },
        }
    }
}

impl From<usize> for FDSet {
    #[inline]
    fn from(v: usize) -> FDSet {
        let mut f = FDSet { count: 1, array: [0; 64] };
        f.array[0] = v;
        f
    }
}

impl Default for SockInfo {
    #[inline]
    fn default() -> SockInfo {
        SockInfo {
            pad1:           [0; 19],
            pad2:           [0; 2],
            pad3:           [0; 5],
            pad4:           [0; 256],
            protocol:       0,
            socket_type:    0,
            address_family: 0,
        }
    }
}
impl Default for IoStatusBlock {
    #[inline]
    fn default() -> IoStatusBlock {
        IoStatusBlock { status: 0, info: 0 }
    }
}
impl Default for LsaAttributes {
    #[inline]
    fn default() -> LsaAttributes {
        LsaAttributes {
            pad1:       0,
            pad2:       ptr::null(),
            pad3:       [0, 0],
            length:     mem::size_of::<LsaAttributes>() as u32,
            attributes: 0,
        }
    }
}
