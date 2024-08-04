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

use core::mem::{size_of, MaybeUninit};
use core::slice::{from_raw_parts, from_raw_parts_mut};
use core::{cmp, ptr};

use crate::data::blob::{Blob, Slice};
use crate::data::read_u32;
use crate::device::winapi::{self, AsHandle, Win32Result};
use crate::prelude::*;
use crate::util::{ToStr, HEXTABLE};

#[repr(C)]
pub struct SID {
    pub revision:        u8,
    pub sub_authorities: u8,
    pub identifiers:     [u8; 6],
    pub authorities:     [u32; 2],
}
#[repr(C)]
pub struct LUID {
    pub low:  u32,
    pub high: i32,
}
#[repr(C)]
pub struct TokenUser<'a> {
    pub user: SIDAndAttributes<'a>,
}
#[repr(transparent)]
pub struct PSID<'a>(&'a SID);
#[repr(C)]
pub struct TokenPrivileges {
    pub count:      u32,
    pub privileges: [MaybeUninit<LUIDAndAttributes>; 10],
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
pub struct SIDAndAttributes<'a> {
    pub sid:        PSID<'a>,
    pub attributes: u32,
}
#[repr(C)]
pub struct SecurityQualityOfService {
    pub length:                u32,
    pub impersonation_level:   u32,
    pub context_tracking_mode: u8,
    pub effective_only:        u8,
}

pub type SecQoS<'a> = Option<&'a SecurityQualityOfService>;
pub type SecAttrs<'a> = Option<&'a SecurityAttributes>;

impl SID {
    #[inline]
    pub fn empty() -> SID {
        SID {
            revision:        1u8,
            identifiers:     [0u8, 0u8, 0u8, 0u8, 0u8, 0u8],
            authorities:     [0u32, 0u32],
            sub_authorities: 0u8,
        }
    }
    #[inline]
    pub fn well_known(domain: u8, authority: u32) -> SID {
        SID {
            revision:        1u8,
            identifiers:     [0u8, 0u8, 0u8, 0u8, 0u8, domain],
            authorities:     [authority, 0u32],
            sub_authorities: 1u8,
        }
    }

    #[inline]
    pub fn len(&self) -> u32 {
        8 + (self.sub_authorities as u32 * 4)
    }
    #[inline]
    pub fn as_psid(&self) -> PSID {
        PSID(self)
    }
    #[inline]
    pub fn to_string(&self) -> String {
        unsafe { String::from_utf8_unchecked(self.to_string_raw()) }
    }
    #[inline]
    pub fn authorities_slice(&self) -> &[u32] {
        // 0xF - SID_MAX_SUB_AUTHORITIES
        unsafe {
            from_raw_parts(
                self.authorities.as_ptr(),
                cmp::min(self.sub_authorities as usize, 0xF),
            )
        }
    }

    #[inline]
    fn is_nt(&self) -> bool {
        self.identifiers[0] == 0 && self.identifiers[1] == 0 && self.identifiers[2] == 0 && self.identifiers[3] == 0 && self.identifiers[4] == 0 && self.identifiers[5] == 0x5
    }
    fn to_string_raw(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32);
        b.push(b'S');
        b.push(b'-');
        b.push(b'1');
        b.push(b'-');
        if self.identifiers[0] == 0 && self.identifiers[1] == 0 {
            read_u32(&self.identifiers[2..5]).into_vec(&mut b)
        } else {
            write_hex(&mut b, self.identifiers[0]);
            write_hex(&mut b, self.identifiers[1]);
            write_hex(&mut b, self.identifiers[2]);
            write_hex(&mut b, self.identifiers[3]);
            write_hex(&mut b, self.identifiers[4]);
            write_hex(&mut b, self.identifiers[5]);
        }
        for i in self.authorities_slice() {
            b.push(b'-');
            i.into_vec(&mut b);
        }
        b
    }
}
impl PSID<'_> {
    #[inline]
    pub fn is_admin(&self) -> bool {
        if !self.0.is_nt() {
            return false;
        }
        // Check for Identifier [0, 0, 0, 0, 5] and Authority [32, 544] (Local
        // Administrators Group)
        if self.0.sub_authorities == 2 && self.0.authorities[0] == 0x20 && self.0.authorities[1] == 0x220 {
            return true;
        }
        if self.0.sub_authorities < 3 || self.0.authorities[0] != 0x15 {
            return false;
        }
        let a = self.0.authorities_slice();
        // Check the last entry, Domain Administrators should be 512 (0x200).
        // https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/understand-security-identifiers
        a[a.len() - 1] == 0x200
    }
    #[inline]
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
    #[inline]
    pub fn to_slice(&self) -> Slice<u8, 256> {
        (&self.0.to_string_raw()[..]).into()
    }
    #[inline]
    pub fn user(&self) -> Win32Result<String> {
        winapi::username_from_sid(self.0)
    }
    #[inline]
    pub fn is_well_known(&self, sid: u32) -> bool {
        // Check for the SID group value in Authorities
        self.0.sub_authorities == 1 && self.0.is_nt() && self.0.authorities[0] == sid
    }
}
impl TokenPrivileges {
    #[inline]
    pub fn as_slice(&self) -> &[LUIDAndAttributes] {
        unsafe {
            from_raw_parts(
                self.privileges.as_ptr() as *const LUIDAndAttributes,
                self.count as usize,
            )
        }
    }
    #[inline]
    pub fn set(&mut self, pos: usize, v: LUIDAndAttributes) {
        if pos > 10 || self.count > 10 {
            return;
        }
        self.privileges[pos].write(v);
        self.count += 1;
    }
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [LUIDAndAttributes] {
        unsafe {
            from_raw_parts_mut(
                self.privileges.as_mut_ptr() as *mut LUIDAndAttributes,
                self.count as usize,
            )
        }
    }
}
impl SecurityAttributes {
    #[inline]
    pub fn inherit() -> SecurityAttributes {
        SecurityAttributes {
            length:              size_of::<SecurityAttributes>() as u32,
            inherit:             1u32,
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl SecurityQualityOfService {
    #[inline]
    pub fn level(level: u32) -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0xCu32,
            effective_only:        0u8,
            impersonation_level:   level,
            context_tracking_mode: 0u8,
        }
    }
}

impl Default for SID {
    #[inline]
    fn default() -> SID {
        SID::empty()
    }
}
impl Default for LUIDAndAttributes {
    #[inline]
    fn default() -> LUIDAndAttributes {
        LUIDAndAttributes {
            luid:       LUID { low: 0u32, high: 0i32 },
            attributes: 0u32,
        }
    }
}
impl Default for SecurityAttributes {
    #[inline]
    fn default() -> SecurityAttributes {
        SecurityAttributes {
            length:              size_of::<SecurityAttributes>() as u32,
            inherit:             0u32,
            security_descriptor: ptr::null_mut(),
        }
    }
}
impl Default for SecurityQualityOfService {
    #[inline]
    fn default() -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0xCu32,
            effective_only:        0u8,
            impersonation_level:   0u32,
            context_tracking_mode: 0u8,
        }
    }
}

impl Drop for TokenPrivileges {
    #[inline]
    fn drop(&mut self) {
        for i in 0..self.count as usize {
            unsafe { self.privileges[i].assume_init_drop() }
        }
    }
}
impl Default for TokenPrivileges {
    #[inline]
    fn default() -> TokenPrivileges {
        TokenPrivileges {
            count:      0u32,
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

#[inline]
pub fn token_sid(h: impl AsHandle) -> Win32Result<String> {
    token_info(h, |u| Ok(u.user.sid.to_string()))
}
#[inline]
pub fn token_username(h: impl AsHandle) -> Win32Result<String> {
    token_info(h, |u| u.user.sid.user())
}
pub fn token_info<'a, T, F: FnOnce(&TokenUser) -> Win32Result<T>>(h: impl AsHandle, func: F) -> Win32Result<T> {
    let mut b = Blob::new();
    let r = {
        // Do stuff down here to avoid freeing the Blob to early.
        let u = winapi::token_user(h, &mut b)?;
        func(u)
    };
    Ok(r?)
}

#[inline]
fn write_hex(buf: &mut Vec<u8>, v: u8) {
    match v {
        0 => {
            buf.push(b'0');
            buf.push(b'0');
        },
        1..=16 => {
            buf.push(b'0');
            buf.push(HEXTABLE[(v as usize) & 0x0F])
        },
        _ => {
            buf.push(HEXTABLE[(v as usize) >> 4]);
            buf.push(HEXTABLE[(v as usize) & 0x0F]);
        },
    }
}
