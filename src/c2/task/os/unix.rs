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
#![cfg(target_family = "unix")]

extern crate libc;

use core::ops::Deref;
use core::time::Duration;
use std::fs::{metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::MetadataExt;

use libc::{c_int, pollfd};

use crate::c2::event::{Context, Entry, Poll, Reason, Return, Task};
use crate::c2::mux::Mux;
use crate::c2::task::{OsCommand, OsFd, OsMetadata};
use crate::c2::{CoreError, CoreResult};
use crate::com::Packet;
use crate::data::Writer;
use crate::device::expand_fiber;
use crate::fs::{File, Metadata};
use crate::io::Error;
use crate::prelude::*;
use crate::process::{Child, Command, Stdio};
use crate::{ignore_error, io};

pub struct Driver {
    h:    Handles,
    send: usize,
    recv: usize,
}
pub struct Fd(usize);
pub struct Beacon(usize);

struct DataIn {
    buf:  [u8; 4096],
    file: File,
}
struct DataOut {
    pos:  usize,
    buf:  Packet,
    file: File,
}
#[repr(C)]
struct Handles {
    signal:  c_int,
    sig_ev:  i16,
    sig_rev: i16,
    events:  [pollfd; 63],
}

impl Fd {
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}
impl Driver {
    #[inline]
    pub fn new() -> io::Result<Driver> {
        let mut p = [0; 0];
        let r = unsafe { libc::pipe(p.as_mut_ptr()) };
        if r != 0 {
            return Err(Error::last_os_error());
        }
        ignore_error!(unsafe { libc::fcntl(p[1], libc::F_SETFL, libc::O_NONBLOCK) });
        Ok(Driver {
            send: p[1] as usize,
            recv: p[0] as usize,
            h:    Handles {
                signal:  p[0] as _,
                sig_ev:  libc::POLLIN,
                sig_rev: 0,
                events:  [pollfd {
                    fd:      0,
                    events:  0,
                    revents: 0,
                }; 63],
            },
        })
    }

    #[inline]
    pub fn reset(&self) {
        let mut b = [0u8];
        unsafe { libc::read(self.recv as _, b.as_mut_ptr() as _, 1) };
    }
    #[inline]
    pub fn beacon(&self) -> Beacon {
        Beacon(self.send)
    }
    pub fn update(&mut self, e: &[Entry]) {
        let n = e.len();
        for (i, v) in self.h.events.iter_mut().enumerate() {
            if i < n {
                v.fd = **(e[i].as_fd()) as _;
                (v.events, v.revents) = (libc::POLLOUT | libc::POLLIN, 0);
            } else {
                (v.fd, v.events, v.revents) = (0, 0, 0);
            }
        }
    }
    pub fn poll(&mut self, dur: Option<Duration>) -> Option<usize> {
        self.h.sig_rev = 0; // Reset
        let r = unsafe {
            libc::poll(
                &mut self.h as *mut Handles as *mut pollfd,
                64,
                dur.map(|v| v.as_millis() as _).unwrap_or(-1),
            )
        };
        if r <= 0 || self.h.sig_rev > 0 {
            return None;
        }
        for (i, e) in self.h.events.iter_mut().enumerate() {
            if e.revents == 0 {
                continue;
            }
            e.revents = 0;
            return Some(i);
        }
        None
    }
}
impl Beacon {
    #[inline]
    pub fn set(&self) {
        let b = [0u8];
        ignore_error!(unsafe { libc::write(self.0 as _, b.as_ptr() as _, 1) });
    }
}
impl DataIn {
    #[inline]
    fn read(&mut self, w: &mut Packet) -> io::Result<usize> {
        let n = self.file.read(&mut self.buf)?;
        if n > 0 {
            ignore_error!(w.write(&self.buf[0..n]));
        }
        Ok(n)
    }
}
impl DataOut {
    #[inline]
    fn write(&mut self) -> io::Result<usize> {
        let n = self.file.write(&self.buf[self.pos..])?;
        self.pos += n;
        Ok(n)
    }
}

impl Drop for Driver {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            libc::close(self.recv as _);
            libc::close(self.send as _);
        }
    }
}

impl Deref for Fd {
    type Target = usize;

    #[inline]
    fn deref(&self) -> &usize {
        &self.0
    }
}
impl Default for Fd {
    #[inline]
    fn default() -> Fd {
        Fd(0)
    }
}
impl From<i32> for Fd {
    #[inline]
    fn from(v: i32) -> Fd {
        Fd(v as usize)
    }
}
impl From<u32> for Fd {
    #[inline]
    fn from(v: u32) -> Fd {
        Fd(v as usize)
    }
}
impl From<usize> for Fd {
    #[inline]
    fn from(v: usize) -> Fd {
        Fd(v)
    }
}

impl OsFd for File {
    #[inline]
    fn get_fd(&self) -> Option<Fd> {
        Some(self.as_fd().as_raw_fd().into())
    }
}
impl OsFd for Child {
    #[inline]
    fn get_fd(&self) -> Option<Fd> {
        (self
            .stdout
            .as_ref()
            .map(|v| v.as_fd().as_raw_fd() as usize)
            .or_else(|| self.stderr.as_ref().map(|v| v.as_fd().as_raw_fd() as usize)))
        .map(Fd)
    }
}

impl OsCommand for Command {
    #[inline]
    fn add_compat(&mut self) {
        self.stdout(Stdio::piped());
    }
}

impl OsMetadata for Metadata {
    #[inline]
    fn get_mode(&self) -> u32 {
        self.mode()
    }
}

#[inline]
pub fn task_window_list() -> CoreResult<Vec<Packet>> {
    Err(CoreError::UnsupportedOs)
}
#[inline]
pub fn task_ui(_sched: &mut Mux, _r: Packet) -> CoreResult<()> {
    Err(CoreError::UnsupportedOs)
}
pub fn task_upload(mux: &mut Mux, mut r: Packet) -> CoreResult<Option<Packet>> {
    let p = expand_fiber(r.read_str_ptr()?.ok_or_else(|| CoreError::from(ErrorKind::InvalidInput))?);
    let f = OpenOptions::new().create(true).truncate(true).write(true).open(&p)?;
    let mut n = Packet::new();
    ignore_error!(n.write_str(&p));
    let h = f.get_fd();
    non_blocking(&h);
    let t = Task::new_with_packet(r.job, n, task_upload_done)
        .poll(task_upload_poll)
        .arg(DataOut { file: f, pos: 0usize, buf: r });
    mux.submit(h, t);
    Ok(None)
}
pub fn task_download(mux: &mut Mux, mut r: Packet) -> CoreResult<Option<Packet>> {
    let p = expand_fiber(r.read_str_ptr()?.ok_or_else(|| CoreError::from(ErrorKind::InvalidInput))?);
    let m = metadata(&p)?;
    let mut n = Packet::new();
    ignore_error!(n.write_str(&p));
    if m.is_dir() {
        ignore_error!(n.write_bool(true));
        ignore_error!(n.write_u64(0));
        return Ok(Some(n));
    }
    let s = m.len();
    ignore_error!(n.write_bool(false));
    ignore_error!(n.write_u64(s));
    let f = OpenOptions::new().read(true).open(&p)?;
    let h = f.get_fd();
    non_blocking(&h);
    n.reserve(s as usize);
    let t = Task::new_with_packet(r.job, n, task_download_done)
        .poll(task_download_poll)
        .arg(DataIn { file: f, buf: [0u8; 4096] });
    mux.submit(h, t);
    Ok(None)
}

#[inline]
fn non_blocking(fd: &Option<Fd>) {
    if let Some(f) = fd.as_ref() {
        unsafe {
            let v = libc::fcntl((**f) as _, libc::F_GETFL);
            if v != -1 {
                ignore_error!(libc::fcntl((**f) as _, libc::F_SETFL, v | libc::O_NONBLOCK));
            }
        }
    }
}
fn task_upload_done(ctx: &mut Context, w: &mut Packet) -> io::Result<Return> {
    let mut f = some_or_return!(ctx.arg::<DataOut>(), Ok(Return::Output));
    while f.write()? > 0 && f.pos < f.buf.len() {}
    ignore_error!(w.write_u64(f.pos as u64));
    Ok(Return::Output)
}
fn task_download_done(ctx: &mut Context, w: &mut Packet) -> io::Result<Return> {
    let mut f = some_or_return!(ctx.arg::<DataIn>(), Ok(Return::Output));
    while f.read(w)? > 0 {}
    Ok(Return::Output)
}
fn task_upload_poll(ctx: &mut Context, r: Reason, _w: &mut Packet) -> io::Result<Poll> {
    match r {
        Reason::Closing | Reason::Timeout => return Ok(Poll::Done),
        _ => (),
    }
    let f = some_or_return!(ctx.arg_mut::<DataOut>(), Ok(Poll::Done));
    if f.write()? == 0 || f.pos >= f.buf.len() {
        Ok(Poll::Done)
    } else {
        Ok(Poll::Pending)
    }
}
fn task_download_poll(ctx: &mut Context, r: Reason, w: &mut Packet) -> io::Result<Poll> {
    match r {
        Reason::Closing | Reason::Timeout => return Ok(Poll::Done),
        _ => (),
    }
    let f = some_or_return!(ctx.arg_mut::<DataIn>(), Ok(Poll::Done));
    if f.read(w)? == 0 {
        Ok(Poll::Done)
    } else {
        Ok(Poll::Pending)
    }
}
