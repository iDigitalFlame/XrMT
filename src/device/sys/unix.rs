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
#![cfg(unix)]

extern crate libc;

use core::ptr;

use libc::passwd;

use crate::data::blob::Blob;
use crate::device::Login;
use crate::util::crypt;
use crate::util::stx::io::{self, Error};
use crate::util::stx::prelude::*;

pub const SHELL_ARGS: &str = "-c";

pub fn shell() -> String {
    crypt::get_or(0, "bash").to_string()
}
#[inline]
pub fn powershell() -> String {
    crypt::get_or(0, "powershell.exe").to_string()
}
#[inline]
pub fn whoami() -> io::Result<String> {
    // WIP
    let c = unsafe { libc::geteuid() };
    let mut n = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) as usize };
    let mut b: Blob<u8, 256> = Blob::with_capacity(n);
    let mut p = passwd {
        pw_name:   ptr::null_mut(),
        pw_passwd: ptr::null_mut(),
        pw_uid:    0,
        pw_gid:    0,
        pw_gecos:  ptr::null_mut(),
        pw_dir:    ptr::null_mut(),
        pw_shell:  ptr::null_mut(),
    };
    b.resize(128);
    println!("get user? {c}");
    let mut v: *mut passwd = ptr::null_mut();
    loop {
        let r = unsafe { libc::getpwuid_r(c, &mut p, b.as_mut_ptr() as *mut i8, n, &mut v) };
        match r {
            libc::ERANGE => {
                n *= 2;
                unsafe { b.resize(n) };
                continue;
            },
            0 => {
                println!("n is {n}");
                unsafe {
                    b.truncate(n);
                }
                println!("output {:#?}", b);
                return Ok("me".to_string());
            },
            _ => return Err(Error::last_os_error()),
        }
    }
}
#[inline]
pub fn hostname() -> io::Result<String> {
    let n = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) as usize };
    let mut b: Blob<u8, 256> = Blob::with_capacity(n);
    b.resize(n);
    if unsafe { libc::gethostname(b.as_mut_ptr() as *mut i8, n) } == 0 {
        Ok(String::from_utf8_lossy(&b[0..b.iter().position(|&v| v == 0).unwrap_or(n)]).to_string())
    } else {
        Err(Error::last_os_error())
    }
}
pub fn logins() -> io::Result<Vec<Login>> {
    Err(io::ErrorKind::Unsupported.into())
}
pub fn mounts() -> io::Result<Vec<String>> {
    Err(io::ErrorKind::Unsupported.into())
}
#[inline]
#[allow(unused_variables)] // TODO(dij)
pub fn set_critical(is_critical: bool) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

pub mod env {
    extern crate std;
    pub use std::env::*;
}
