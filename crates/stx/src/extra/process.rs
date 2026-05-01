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
#![cfg(all(target_family = "windows", not(feature = "std")))]
#![allow(unused_variables)] // TODO(dij): Finish this

extern crate alloc;
extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_crypt;
extern crate xrmt_data;
extern crate xrmt_winapi;

use alloc::vec::Vec;
use core::convert::{AsRef, From};
use core::marker::Sized;
use core::mem::ManuallyDrop;
use core::option::Option::{self, None, Some};
use core::result::Result::Ok;

use xrmt_data::MaybeString;
use xrmt_winapi::functions::{current_process_info, CreatePipe, NtCreateFile};
use xrmt_winapi::structs::{OwnedHandle, WCharLike};

use crate::ffi::OsStr;
use crate::fs::File;
use crate::io::{IoResult, Read};
use crate::os::windows::io::{AsHandle, BorrowedHandle};
use crate::os::windows::process::{ChildExt, CommandExt, ExitCodeExt, ExitStatusExt};
use crate::os::Handle;
use crate::process::{Arg, Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitCode, ExitStatus, StartParameters, Stdio, StdioType};

pub trait ChildExtra {
    fn thread_handle(&self) -> Handle;
    fn into_handle(self) -> OwnedHandle;
    fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> IoResult<ExitStatus>;
}
pub trait CommandExtra {
    fn flags(&self) -> u32;
    fn set_flags(&mut self, flags: u32) -> &mut Self;
    fn set_new_console(&mut self, new: bool) -> &mut Self;
    fn set_no_window(&mut self, window: bool) -> &mut Self;
    fn set_detached(&mut self, detached: bool) -> &mut Self;
    fn set_suspended(&mut self, suspended: bool) -> &mut Self;
    fn set_inherit_envs(&mut self, inherit: bool) -> &mut Self;
    fn set_fullscreen(&mut self, fullscreen: bool) -> &mut Self;
    fn output_combined(&mut self, out: &mut Vec<u8>) -> IoResult<ExitStatus>;
    fn spawn_with_params<'a>(&self, params: &StartParameters<'a>) -> IoResult<Child>;
    fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> IoResult<ExitStatus>;
}
pub trait StdioExtra: Sized {
    fn handle(h: OwnedHandle) -> Self;
    fn file(read: bool, write: bool, path: impl AsRef<str>) -> IoResult<Self>;
}

impl CommandExt for Command {
    #[inline]
    fn show_window(&mut self, cmd_show: u16) -> &mut Command {
        self.mode = (cmd_show as u8 & 0x3F) | 0x40;
        self
    }
    #[inline]
    fn creation_flags(&mut self, flags: u32) -> &mut Command {
        self.flags = flags;
        self
    }
    #[inline]
    fn force_quotes(&mut self, enabled: bool) -> &mut Command {
        self.force_quotes = enabled;
        self
    }
    #[inline]
    fn raw_arg(&mut self, raw: impl AsRef<OsStr>) -> &mut Command {
        self.args.push(Arg::raw(raw));
        self
    }
}
impl CommandExtra for Command {
    #[inline]
    fn flags(&self) -> u32 {
        self.flags
    }
    #[inline]
    fn set_flags(&mut self, flags: u32) -> &mut Command {
        self.flags = flags;
        self
    }
    #[inline]
    fn set_new_console(&mut self, new: bool) -> &mut Command {
        // 0x10 - CREATE_NEW_CONSOLE
        if new {
            self.flags |= 0x10;
        } else {
            self.flags ^= 0x10;
        }
        self
    }
    #[inline]
    fn set_no_window(&mut self, window: bool) -> &mut Command {
        // 0x8000000 - CREATE_NO_WINDOW
        if window {
            self.flags |= 0x8000000;
        } else {
            self.flags ^= 0x8000000;
        }
        self
    }
    #[inline]
    fn set_detached(&mut self, detached: bool) -> &mut Command {
        // 0x8  - DETACHED_PROCESS
        // 0x10 - CREATE_NEW_CONSOLE
        if detached {
            self.flags = (self.flags | 0x8) ^ 0x10;
        } else {
            self.flags ^= 0x8;
        }
        self
    }
    #[inline]
    fn set_suspended(&mut self, suspended: bool) -> &mut Command {
        // 0x4 - CREATE_SUSPENDED
        if suspended {
            self.flags |= 0x4;
        } else {
            self.flags ^= 0x4;
        }
        self
    }
    #[inline]
    fn set_inherit_envs(&mut self, inherit: bool) -> &mut Command {
        self.clear = !inherit;
        self
    }
    #[inline]
    fn set_fullscreen(&mut self, fullscreen: bool) -> &mut Command {
        if fullscreen {
            self.mode |= 0x80;
        } else {
            self.mode ^= 0x80;
        }
        self
    }
    fn output_combined(&mut self, out: &mut Vec<u8>) -> IoResult<ExitStatus> {
        // Create our reading Pipe.
        let (r, w) = CreatePipe(None, 0x1000, true)?;
        // This copy will NOT drop, but it's ok since it's a "shallow" clone of 'w'.
        // When 'w' is dropped, this will become invalid anyway.
        let e = ManuallyDrop::new(Stdio::handle(unsafe { Handle::shallow_clone(&w) }));
        let mut x = self.spawn_outer(None, &self.stdin, &Stdio::handle(w), &e)?;
        // Stdout and Stderr will both pipe to the File below, so we'll just read from
        // it.
        let _ = File::from(r).read_to_end(out)?;
        // Dropping the file will drop 'r'.
        x.wait()
    }
    #[inline]
    fn spawn_with_params<'a>(&self, params: &StartParameters<'a>) -> IoResult<Child> {
        self.spawn_base(Some(params))
    }
    #[inline]
    fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> IoResult<ExitStatus> {
        self.spawn_outer(None, &self.stdin, &Stdio::piped(), &Stdio::piped())?
            .wait_output(stdout, stderr)
    }
}

impl ChildExt for Child {
    #[inline]
    fn main_thread_handle(&self) -> BorrowedHandle<'_> {
        self.info.thread.as_handle()
    }
}
impl ChildExtra for Child {
    #[inline]
    fn thread_handle(&self) -> Handle {
        *self.info.thread
    }
    #[inline]
    fn into_handle(self) -> OwnedHandle {
        self.info.process
    }
    #[inline]
    fn wait_with_output_in(self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> IoResult<ExitStatus> {
        self.wait_output(stdout, stderr)
    }
}
impl AsRef<Handle> for Child {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.info.process
    }
}

impl ExitCodeExt for ExitCode {
    #[inline]
    fn from_raw(raw: u32) -> ExitCode {
        ExitCode(raw as i32)
    }
}
impl ExitStatusExt for ExitStatus {
    #[inline]
    fn from_raw(raw: u32) -> ExitStatus {
        ExitStatus(raw as i32)
    }
}

impl StdioExtra for Stdio {
    #[inline]
    fn handle(h: OwnedHandle) -> Stdio {
        Stdio { v: StdioType::Handle, h }
    }
    #[inline]
    fn file(read: bool, write: bool, path: impl AsRef<str>) -> IoResult<Stdio> {
        let (a, d, s) = match (read, write) {
            (true, false) => (0x80100000, 0x1, 0x1),
            (false, true) => (0x40100000, 0x4, 0x2),
            _ => (0xC0100000, 0x3, 0x3),
        };
        Ok(Stdio::handle(NtCreateFile(
            path.as_ref(),
            Handle::EMPTY,
            a | 0x10000,
            None,
            0,
            s,
            d,
            0x20,
        )?))
    }
}
impl AsRef<Handle> for Stdio {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.h
    }
}

impl AsRef<Handle> for ChildStdin {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}
impl AsRef<Handle> for ChildStdout {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.h
    }
}
impl AsRef<Handle> for ChildStderr {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.0.as_ref()
    }
}

impl<'a> StartParameters<'a> {
    pub fn new() -> StartParameters<'a> {
        StartParameters {
            x:           0i32,
            y:           0i32,
            user:        WCharLike::Null,
            flags:       0u32,
            title:       WCharLike::Null,
            token:       None,
            width:       0u32,
            height:      0u32,
            domain:      WCharLike::Null,
            desktop:     WCharLike::Null,
            password:    WCharLike::Null,
            mitigations: None,
        }
    }

    pub fn set_token(&mut self, token: Option<&'a OwnedHandle>) -> &mut StartParameters<'a> {
        self.token = token;
        self
    }
    pub fn set_mitigations(&mut self, mitigations: u64) -> &mut StartParameters<'a> {
        self
    }
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> &mut StartParameters<'a> {
        self
    }
    pub fn set_window_display(&mut self, display: i16) -> &mut StartParameters<'a> {
        self
    }
    pub fn set_window_position(&mut self, x: u32, y: u32) -> &mut StartParameters<'a> {
        self
    }
    pub fn set_window_size(&mut self, width: u32, height: u32) -> &mut StartParameters<'a> {
        self
    }
    pub fn set_window_title(&mut self, title: impl MaybeString) -> &mut StartParameters<'a> {
        self
    }
    // fn set_parent(&mut self, filter: impl MaybeFilter) -> &mut Command;

    pub fn set_login<V: AsRef<str>>(&mut self, user: Option<V>, domain: Option<V>, password: Option<V>) -> &mut StartParameters<'a> {
        self
    }
}

#[inline]
pub fn parent_id() -> u32 {
    current_process_info().map_or(0, |i| i.parent_process_id as u32)
}
