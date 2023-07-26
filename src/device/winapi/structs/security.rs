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

use core::mem::MaybeUninit;
use core::{mem, ptr};

use crate::data::blob::{Blob, Slice};
use crate::device::winapi::loader::{advapi32, ntdll};
use crate::device::winapi::{self, AsHandle, DecodeUtf16, Handle, WCharPtr, Win32Result};
use crate::util::stx::prelude::*;

#[repr(C)]
pub struct LUID {
    pub low:  u32,
    pub high: i32,
}
#[repr(transparent)]
pub struct SID(usize);
#[repr(C)]
pub struct TokenUser {
    pub user: SIDAndAttributes,
}
#[repr(C)]
pub struct TokenPrivileges {
    pub count:      u32,
    pub privileges: [MaybeUninit<LUIDAndAttributes>; 10],
}
#[repr(C)]
pub struct SIDAndAttributes {
    pub sid:        SID,
    pub attributes: u32,
}
#[repr(C)]
pub struct LUIDAndAttributes {
    pub luid:       LUID,
    pub attributes: u32,
}
#[repr(C)]
pub struct SecurityDescriptor {
    pad1: [u8; 2],
    pad2: u16,
    pad3: [usize; 2],
    pad4: [usize; 2],
}
#[repr(C)]
pub struct SecurityAttributes {
    pub length:              u32,
    pub security_descriptor: *mut SecurityDescriptor,
    pub inherit:             u32,
}
#[repr(C)]
pub struct SecurityQualityOfService {
    pub length:                u32,
    pub impersonation_level:   u32,
    pub context_tracking_mode: u32,
    pub effective_only:        u32,
}

pub type SecQoS<'a> = Option<&'a SecurityQualityOfService>;
pub type SecAttrs<'a> = Option<&'a SecurityAttributes>;

impl SID {
    pub fn user(&self) -> Win32Result<String> {
        winapi::init_advapi32();
        let (mut c, mut x, mut t) = (64u32, 64u32, 0u32);
        let mut n = Blob::<u16, 256>::with_capacity(c as usize);
        let mut d = Blob::<u16, 256>::with_capacity(x as usize);
        let func = unsafe {
            winapi::make_syscall!(
                *advapi32::LookupAccountSid,
                extern "stdcall" fn(*const u16, usize, *mut u16, *mut u32, *mut u16, *mut u32, *mut u32) -> u32
            )
        };
        loop {
            n.resize(c as usize * 2);
            d.resize(x as usize * 2);
            let r = func(
                ptr::null(),
                self.0,
                n.as_mut_ptr(),
                &mut c,
                d.as_mut_ptr(),
                &mut x,
                &mut t,
            );
            if r > 0 {
                return if x == 0 {
                    Ok((&n[0..c as usize]).decode_utf16())
                } else {
                    d.truncate(x as usize);
                    d.push(b'\\' as u16);
                    d.extend_from_slice(&n[0..c as usize]);
                    Ok((&d[0..(c + x) as usize + 1]).decode_utf16())
                };
            } else if r != 0x7A || c < n.len() as u32 {
                // 0x7A - ERROR_INSUFFICIENT_BUFFER
                return Err(winapi::last_error());
            }
        }
    }
    #[inline]
    pub fn is_well_known(&self, sid: u32) -> bool {
        winapi::init_advapi32();
        unsafe {
            winapi::syscall!(
                *advapi32::IsWellKnownSID,
                extern "stdcall" fn(usize, u32) -> u32,
                self.0,
                sid
            ) > 0
        }
    }
    pub fn to_string(&self) -> Win32Result<String> {
        winapi::init_advapi32();
        let mut b = WCharPtr::null();
        let r = unsafe {
            winapi::syscall!(
                *advapi32::ConvertSIDToStringSID,
                extern "stdcall" fn(usize, *mut WCharPtr) -> u32,
                self.0,
                &mut b
            ) == 0
        };
        if r {
            return Err(winapi::last_error());
        }
        let o = b.to_string();
        winapi::LocalFree(b.as_ptr());
        Ok(o)
    }
    pub fn to_slice(&self) -> Win32Result<Slice<u8, 256>> {
        winapi::init_advapi32();
        let mut b = WCharPtr::null();
        let r = unsafe {
            winapi::syscall!(
                *advapi32::ConvertSIDToStringSID,
                extern "stdcall" fn(usize, *mut WCharPtr) -> u32,
                self.0,
                &mut b
            ) == 0
        };
        if r {
            return Err(winapi::last_error());
        }
        // NOTE(dij): SIDs are only numbers and hex values, so u8's are OK!
        let r = b.as_slice();
        let mut o = Slice::with_len(r.len());
        for (i, v) in r.iter().enumerate() {
            o.data[i] = *v as u8;
        }
        winapi::LocalFree(b.as_ptr());
        Ok(o)
    }
}
impl TokenUser {
    pub fn from_token(h: impl AsHandle, buf: &mut Blob<u8, 256>) -> Win32Result<&TokenUser> {
        winapi::init_ntdll();
        let (mut n, mut c) = (64u32, 64u32);
        let func = unsafe {
            winapi::make_syscall!(
                *ntdll::NtQueryInformationToken,
                extern "stdcall" fn(Handle, u32, *mut u8, u32, *mut u32) -> u32
            )
        };
        let v = h.as_handle();
        loop {
            buf.resize(n as usize);
            // 0x1 - TokenUser
            let r = func(v, 0x1, buf.as_mut_ptr(), n, &mut c);
            match r {
                0x7A => return Err(winapi::nt_error(r)), // 0x7A - ERROR_INSUFFICIENT_BUFFER
                0 => return Ok(unsafe { &*(buf.as_ptr() as *const TokenUser) }),
                _ if c < n => return Err(winapi::nt_error(r)),
                _ => n = c,
            }
        }
    }
}
impl TokenPrivileges {
    #[inline]
    pub fn set(&mut self, pos: usize, v: LUIDAndAttributes) {
        if pos > 10 || self.count > 10 {
            return;
        }
        self.privileges[pos].write(v);
        self.count += 1;
    }
}
impl SecurityAttributes {
    #[inline]
    pub fn inherit() -> SecurityAttributes {
        SecurityAttributes {
            length:              mem::size_of::<SecurityAttributes>() as u32,
            inherit:             1,
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl SecurityQualityOfService {
    #[inline]
    pub fn level(level: u32) -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0x10,
            effective_only:        0,
            impersonation_level:   level,
            context_tracking_mode: 0,
        }
    }
}

impl Default for TokenPrivileges {
    #[inline]
    fn default() -> TokenPrivileges {
        TokenPrivileges {
            count:      0,
            privileges: [
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }
}
impl Default for LUIDAndAttributes {
    #[inline]
    fn default() -> LUIDAndAttributes {
        LUIDAndAttributes {
            luid:       LUID { low: 0, high: 0 },
            attributes: 0,
        }
    }
}
impl Default for SecurityAttributes {
    #[inline]
    fn default() -> SecurityAttributes {
        SecurityAttributes {
            length:              mem::size_of::<SecurityAttributes>() as u32,
            inherit:             0,
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl Default for SecurityQualityOfService {
    #[inline]
    fn default() -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0x10,
            effective_only:        0,
            impersonation_level:   0,
            context_tracking_mode: 0,
        }
    }
}
