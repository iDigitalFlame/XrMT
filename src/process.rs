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

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::filter::*;
#[cfg_attr(rustfmt, rustfmt_skip)]
pub use self::inner::*;

use crate::util::stx::io;
use crate::util::stx::prelude::*;

pub trait ChildExtra {
    fn wait_with_output_in_combo(self, out: &mut Vec<u8>) -> io::Result<ExitStatus>;
    fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus>;
}

#[path = "device/filter.rs"]
mod filter;

#[cfg(unix)]
mod inner {
    extern crate alloc;
    extern crate core;
    extern crate libc;
    extern crate std;

    use alloc::vec::Vec;
    use core::convert::Into;
    use core::result::Result::Err;

    use super::ChildExtra;
    use crate::util::stx::io::{self, ErrorKind};
    use crate::util::stx::prelude::*;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub use std::process::*;

    pub struct ThreadEntry;
    pub struct ProcessEntry;

    pub trait CommandExtra {
        fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<ExitStatus>;
        fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus>;
    }

    impl ChildExtra for Child {
        fn wait_with_output_in_combo(self, out: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.wait_with_output()?;
            out.reserve(o.stdout.len() + 1 + o.stderr.len());
            if !o.stdout.is_empty() {
                out.extend_from_slice(&o.stdout);
                if !o.stderr.is_empty() {
                    out.push(b'\n');
                }
            }
            if !o.stderr.is_empty() {
                out.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
        fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus> {
            let o = self.wait_with_output()?;
            if !o.stdout.is_empty() {
                stdout.reserve(o.stdout.len());
                stdout.extend_from_slice(&o.stdout);
            }
            if !o.stderr.is_empty() {
                stderr.reserve(o.stderr.len());
                stderr.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
    }
    impl CommandExtra for Command {
        fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<super::ExitStatus> {
            let o = self.output()?;
            out.reserve(o.stdout.len() + 1 + o.stderr.len());
            if !o.stdout.is_empty() {
                out.extend_from_slice(&o.stdout);
                if !o.stderr.is_empty() {
                    out.push(b'\n');
                }
            }
            if !o.stderr.is_empty() {
                out.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
        fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<super::ExitStatus> {
            let o = self.output()?;
            if !o.stdout.is_empty() {
                stdout.reserve(o.stdout.len());
                stdout.extend_from_slice(&o.stdout);
            }
            if !o.stderr.is_empty() {
                stderr.reserve(o.stderr.len());
                stderr.extend_from_slice(&o.stderr);
            }
            Ok(o.status)
        }
    }

    #[inline]
    pub fn parent_id() -> u32 {
        unsafe { libc::getppid() as u32 }
    }
    #[inline]
    pub fn processes() -> io::Result<Vec<ProcessEntry>> {
        Err(ErrorKind::Unsupported.into())
    }
    #[allow(unused_variables)] // TODO(dij)
    #[inline]
    pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
        Err(ErrorKind::Unsupported.into())
    }
}
#[cfg(windows)]
#[path = "device/winapi/std"]
mod inner {
    mod process;
    pub use self::process::*;
}
