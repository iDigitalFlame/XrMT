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

use alloc::boxed::Box;
use alloc::collections::{btree_map, BTreeMap};
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use core::{hint, mem, slice};

use crate::data::blob::Blob;
use crate::device::fs::File;
use crate::device::winapi::{self, AsHandle, Handle, MaybeString, Overlapped, OwnedHandle, ProcessBasicInfo, ProcessInfo, ProcessThreadAttrList, SecurityAttributes, StartInfo, StartupInfo, StartupInfoEx, StringBlock, WChar, Win32Error};
use crate::device::{env, fs};
use crate::process::{ChildExtra, Filter, MaybeFilter};
use crate::util::stx::ffi::{OsStr, Path};
use crate::util::stx::io::{self, ErrorKind, Read};
use crate::util::stx::prelude::*;
use crate::util::{crypt, ToStr};

const PATHEXT: [u16; 7] = [
    b'P' as u16,
    b'A' as u16,
    b'T' as u16,
    b'H' as u16,
    b'E' as u16,
    b'X' as u16,
    b'T' as u16,
];

static VERSION: AtomicU8 = AtomicU8::new(0);

pub enum Arg {
    Raw(String),
    Auto(String),
    Quoted(String),
}

pub struct Child {
    pub stdin:  Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
    info:       ProcessInfo,
    exit:       AtomicI32,
    done:       AtomicBool,
}
pub struct Stdio {
    v: StdioType,
    h: OwnedHandle,
}
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub struct Command {
    pub dir:      Option<String>,
    pub args:     Vec<Arg>,
    force_quotes: bool,
    split:        bool,
    env:          BTreeMap<String, String>,
    title:        Option<String>,
    stdin:        Stdio,
    stdout:       Stdio,
    stderr:       Stdio,
    filter:       Option<Filter>,
    user:         Option<String>,
    pass:         Option<String>,
    domain:       Option<String>,
    token:        OwnedHandle,
    mode:         u16,
    flags:        u32,
    start_flags:  u32,
    start_x:      u32,
    start_y:      u32,
    start_width:  u32,
    start_height: u32,
}
pub struct ChildStdout {
    file:    OwnedHandle,
    event:   OwnedHandle,
    parent:  Handle,
    overlap: Box<Overlapped>,
}
pub struct ExitCode(i32);
pub struct ExitStatus(i32);
pub struct ChildStdin(File);
pub struct CommandEnvs<'a> {
    iter: btree_map::Iter<'a, String, String>,
}
pub struct CommandArgs<'a> {
    iter: slice::Iter<'a, Arg>,
}
pub struct ChildStderr(ChildStdout);
pub struct ExitStatusError(ExitStatus);

pub trait ChildExt {}
pub trait CommandExt {
    fn creation_flags(&mut self, flags: u32) -> &mut Command;
    fn force_quotes(&mut self, enabled: bool) -> &mut Command;
    fn raw_arg(&mut self, raw: impl AsRef<OsStr>) -> &mut Command;
}
pub trait Termination {
    fn report(self) -> ExitCode;
}
pub trait ExitCodeExt {
    fn from_raw(raw: u32) -> ExitCode;
}
pub trait CommandExtra {
    fn flags(&self) -> u32;
    fn set_flags(&mut self, flags: u32) -> &mut Command;
    fn set_new_console(&mut self, new: bool) -> &mut Command;
    fn set_no_window(&mut self, window: bool) -> &mut Command;
    fn set_detached(&mut self, detached: bool) -> &mut Command;
    fn set_token(&mut self, token: OwnedHandle) -> &mut Command;
    fn set_suspended(&mut self, suspended: bool) -> &mut Command;
    fn set_inherit_env(&mut self, inherit: bool) -> &mut Command;
    fn set_fullscreen(&mut self, fullscreen: bool) -> &mut Command;
    fn set_window_display(&mut self, display: i16) -> &mut Command;
    fn set_window_position(&mut self, x: u32, y: u32) -> &mut Command;
    fn set_parent(&mut self, filter: impl MaybeFilter) -> &mut Command;
    fn set_window_size(&mut self, width: u32, height: u32) -> &mut Command;
    fn set_window_title(&mut self, title: impl MaybeString) -> &mut Command;
    fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<ExitStatus>;
    fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus>;
    fn set_login<V: AsRef<str>>(&mut self, user: Option<V>, domain: Option<V>, password: Option<V>) -> &mut Command;
}
pub trait ExitStatusExt {
    fn from_raw(raw: u32) -> ExitStatus;
}

pub type ThreadEntry = winapi::ThreadEntry;
pub type ProcessEntry = winapi::ProcessEntry;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PipeType {
    Stdin,
    Stdout,
    Stderr,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum StdioType {
    Null,
    Inherit,
    Pipe,
    Handle,
}

struct AsyncPipe<'a> {
    h:   ChildStdout,
    pos: usize,
    buf: &'a mut Vec<u8>,
}

impl Arg {
    #[inline]
    fn new(v: impl AsRef<OsStr>) -> Arg {
        Arg::Auto(v.as_ref().to_string_lossy().to_string())
    }
    #[inline]
    fn raw(v: impl AsRef<OsStr>) -> Arg {
        Arg::Raw(v.as_ref().to_string_lossy().to_string())
    }

    #[inline]
    fn len(&self) -> usize {
        match self {
            Arg::Raw(v) => v.len(),
            Arg::Auto(v) => v.len(),
            Arg::Quoted(v) => v.len(),
        }
    }
    #[inline]
    fn as_str(&self) -> &str {
        match self {
            Arg::Raw(v) => v,
            Arg::Auto(v) => v,
            Arg::Quoted(v) => v,
        }
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }
    #[inline]
    fn as_os_str(&self) -> &OsStr {
        match self {
            Arg::Raw(v) => v.as_ref(),
            Arg::Auto(v) => v.as_ref(),
            Arg::Quoted(v) => v.as_ref(),
        }
    }
}
impl Stdio {
    #[inline]
    pub fn null() -> Stdio {
        Stdio {
            v: StdioType::Null,
            h: OwnedHandle::empty(),
        }
    }
    #[inline]
    pub fn piped() -> Stdio {
        Stdio {
            v: StdioType::Pipe,
            h: OwnedHandle::empty(),
        }
    }
    #[inline]
    pub fn inherit() -> Stdio {
        Stdio {
            v: StdioType::Inherit,
            h: OwnedHandle::empty(),
        }
    }

    #[inline]
    pub fn makes_pipe(&self) -> bool {
        self.v == StdioType::Pipe
    }
    #[inline]
    pub fn handle(h: OwnedHandle) -> Stdio {
        Stdio { v: StdioType::Handle, h }
    }
    #[inline]
    pub fn file(path: impl AsRef<str>) -> io::Result<Stdio> {
        Ok(Stdio {
            v: StdioType::Handle,
            h: Stdio::file_handle(true, true, path)?,
        })
    }
    #[inline]
    pub fn file_read(path: impl AsRef<str>) -> io::Result<Stdio> {
        Ok(Stdio {
            v: StdioType::Handle,
            h: Stdio::file_handle(true, false, path)?,
        })
    }
    #[inline]
    pub fn file_write(path: impl AsRef<str>) -> io::Result<Stdio> {
        Ok(Stdio {
            v: StdioType::Handle,
            h: Stdio::file_handle(false, true, path)?,
        })
    }

    fn create(&self, pipe: PipeType, parent: Handle) -> io::Result<(Handle, Option<OwnedHandle>)> {
        match self.v {
            StdioType::Null => Ok((Stdio::null_pipe(pipe, parent)?, None)),
            StdioType::Inherit => Ok((Stdio::inherit_pipe(pipe, parent)?, None)),
            StdioType::Pipe => {
                let x = SecurityAttributes::inherit();
                let (r, w) = winapi::CreatePipe(Some(&x), 0x10000).map_err(io::Error::from)?;
                match pipe {
                    PipeType::Stdin if parent == winapi::CURRENT_PROCESS => Ok((Handle::take(r), Some(w))),
                    PipeType::Stdin => Ok((
                        r.into_duplicate(true, parent).map_err(io::Error::from)?,
                        Some(w),
                    )),
                    _ if parent == winapi::CURRENT_PROCESS => Ok((Handle::take(w), Some(r))),
                    _ => Ok((
                        w.into_duplicate(true, parent).map_err(io::Error::from)?,
                        Some(r),
                    )),
                }
            },
            StdioType::Handle => Ok((
                winapi::DuplicateHandleEx(&self.h, winapi::CURRENT_PROCESS, parent, 0, true, 0x2).map_err(io::Error::from)?,
                None,
            )),
        }
    }

    #[inline]
    fn null_pipe(pipe: PipeType, parent: Handle) -> io::Result<Handle> {
        let sa = SecurityAttributes::inherit();
        let n = winapi::NtCreateFile(
            crypt::get_or(0, r"\??\NUL"),
            winapi::INVALID,
            if pipe == PipeType::Stdin {
                0x80100080
            } else {
                0x40000000
            },
            Some(&sa),
            0,
            0x3,
            0x1,
            0,
        )
        .map_err(io::Error::from)?;
        if parent == winapi::CURRENT_PROCESS {
            Ok(Handle::take(n))
        } else {
            n.into_duplicate(true, parent).map_err(io::Error::from)
        }
    }
    #[inline]
    fn inherit_pipe(pipe: PipeType, parent: Handle) -> io::Result<Handle> {
        let p = unsafe { (*winapi::GetCurrentProcessPEB()).process_parameters.as_ref() }.ok_or_else(|| io::Error::from(ErrorKind::PermissionDenied))?;
        match pipe {
            PipeType::Stdin if !p.standard_input.is_invalid() => {
                // NOTE(dij): Until Win8 we can't duplicate a STDIN Handle for
                //            a child process that's under a different parent
                //            so we copy a NUL Handle as a fallback.
                if parent != winapi::CURRENT_PROCESS && !winapi::is_min_windows_8() {
                    Stdio::null_pipe(PipeType::Stdin, parent)
                } else {
                    winapi::DuplicateHandleEx(
                        p.standard_input,
                        winapi::CURRENT_PROCESS,
                        parent,
                        0,
                        true,
                        0x2,
                    )
                    .map_err(io::Error::from)
                }
            },
            PipeType::Stdout if !p.standard_output.is_invalid() => winapi::DuplicateHandleEx(
                p.standard_output,
                winapi::CURRENT_PROCESS,
                parent,
                0,
                true,
                0x2,
            )
            .map_err(io::Error::from),
            PipeType::Stderr if !p.standard_error.is_invalid() => winapi::DuplicateHandleEx(
                p.standard_error,
                winapi::CURRENT_PROCESS,
                parent,
                0,
                true,
                0x2,
            )
            .map_err(io::Error::from),
            _ => Stdio::null_pipe(pipe, parent),
        }
    }
    #[inline]
    fn file_handle(read: bool, write: bool, path: impl AsRef<str>) -> io::Result<OwnedHandle> {
        let (a, d, s) = match (read, write) {
            (true, false) => (0x80100000, 0x1, 0x1),
            (false, true) => (0x40100000, 0x4, 0x2),
            _ => (0xC0100000, 0x3, 0x3),
        };
        winapi::NtCreateFile(path, winapi::INVALID, a | 0x10000, None, 0, s, d, 0x20).map_err(io::Error::from)
    }
}
impl Child {
    #[inline]
    pub fn id(&self) -> u32 {
        self.info.process_id
    }
    #[inline]
    pub fn as_raw_handle(&self) -> Handle {
        *self.info.process
    }
    #[inline]
    pub fn into_raw_handle(self) -> Handle {
        Handle::take(self.info.process)
    }
    #[inline]
    pub fn kill(&mut self) -> io::Result<()> {
        winapi::TerminateProcess(&self.info.process, 0x1337).map_err(io::Error::from)
    }
    pub fn exit_code(&mut self) -> ExitStatus {
        // Wait for the Child process to finish if it hasn't yet.
        // Calls to this function when the process is running is similar to
        // 'wait' except this will ALWAYS return an ExitStatus even during
        // failure.
        let _ = winapi::WaitForSingleAsHandle(&self.info.process, -1, false); // IGNORE ERROR
                                                                              // ^ The wait is up here as we should make everyone wait instead of one
                                                                              // winning the race and waiting while the loosers return a bogus result.
        if self
            .done
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            let mut i = ProcessBasicInfo::default();
            // IGNORE ERROR
            // 0x0 - ProcessBasicInformation
            let _ = winapi::NtQueryInformationProcess(
                &self.info.process,
                0,
                &mut i,
                mem::size_of::<ProcessBasicInfo>() as u32,
            );
            self.exit.store(i.exit_status as i32, Ordering::Release);
        }
        ExitStatus(self.exit.load(Ordering::Relaxed))
    }
    #[inline]
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        // Close Stdin Handle.
        drop(self.stdin.take());
        winapi::WaitForSingleAsHandle(&self.info.process, -1, false)
            .map_err(io::Error::from)
            .map(|_| self.exit_code())
    }
    #[inline]
    pub fn wait_with_output(self) -> io::Result<Output> {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let e = self.wait_with_output_in(&mut out, &mut err)?;
        Ok(Output {
            status: e,
            stdout: out,
            stderr: err,
        })
    }
    #[inline]
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        winapi::WaitForSingleAsHandle(&self.info.process, 0, false)
            .map_err(io::Error::from)
            .map(|v| if v == 0 { Some(self.exit_code()) } else { None })
    }

    fn read_to_end_both(&mut self, stdout: ChildStdout, stderr: ChildStderr, o1: &mut Vec<u8>, o2: &mut Vec<u8>) -> io::Result<()> {
        let h = [stdout.event.0, stderr.event.0, self.info.process.0];
        let mut s_out = AsyncPipe::new(stdout, o1);
        let mut s_err = AsyncPipe::new(stderr.0, o2);
        s_out.lookahead()?;
        s_err.lookahead()?;
        loop {
            match winapi::wait_for_multiple_objects(&h, 3, false, -1, false) {
                Err(e) => return Err(e.into()),
                Ok(c) => match c {
                    /* STDOUT */ 0 => {
                        if !s_out.check()? {
                            s_out.complete()?;
                            break;
                        }
                    },
                    /* STDERR */ 1 => {
                        if !s_err.check()? {
                            s_err.complete()?;
                            break;
                        }
                    },
                    /* PROCESS */ 2 => break,
                    _ => core::unreachable!(),
                },
            }
        }
        s_out.done();
        s_err.done();
        Ok(())
    }
}
impl Command {
    #[inline]
    pub fn new(exe: impl AsRef<OsStr>) -> Command {
        Command {
            dir:          None,
            env:          BTreeMap::new(),
            args:         vec![Arg::raw(exe)],
            mode:         0,
            pass:         None,
            user:         None,
            split:        false,
            flags:        0,
            token:        OwnedHandle::empty(),
            title:        None,
            stdin:        Stdio::inherit(),
            stdout:       Stdio::inherit(),
            stderr:       Stdio::inherit(),
            domain:       None,
            filter:       None,
            start_x:      0,
            start_y:      0,
            start_flags:  0,
            start_width:  0,
            start_height: 0,
            force_quotes: false,
        }
    }

    #[inline]
    pub fn get_program(&self) -> &OsStr {
        self.args.get(0).map_or_else(|| OsStr::new(""), |v| v.as_os_str())
    }
    #[inline]
    pub fn get_args(&self) -> CommandArgs {
        CommandArgs { iter: self.args.iter() }
    }
    #[inline]
    pub fn get_envs(&self) -> CommandEnvs {
        CommandEnvs { iter: self.env.iter() }
    }
    #[inline]
    pub fn env_clear(&mut self) -> &mut Command {
        self.env.clear();
        self.split = true;
        self
    }
    #[inline]
    pub fn spawn(&mut self) -> io::Result<Child> {
        self.spawn_init(false, &self.stdin, &self.stdout, &self.stderr)
    }
    #[inline]
    pub fn output(&mut self) -> io::Result<Output> {
        self.spawn_init(false, &self.stdin, &Stdio::piped(), &Stdio::piped())?
            .wait_with_output()
    }
    #[inline]
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.dir.as_ref().map(|d| d.as_ref())
    }
    #[inline]
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.spawn_init(false, &self.stdin, &self.stdout, &self.stderr)?.wait()
    }
    #[inline]
    pub fn stdin(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stdin = v.into();
        self
    }
    #[inline]
    pub fn stdout(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stdout = v.into();
        self
    }
    #[inline]
    pub fn stderr(&mut self, v: impl Into<Stdio>) -> &mut Command {
        self.stderr = v.into();
        self
    }
    #[inline]
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Command {
        self.args.push(Arg::new(arg));
        self
    }
    #[inline]
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Command {
        self.env.remove(&*key.as_ref().to_string_lossy());
        self
    }
    #[inline]
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Command {
        self.dir = Some(dir.as_ref().to_string_lossy().to_string());
        self
    }
    #[inline]
    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Command {
        self.env.insert(
            key.as_ref().to_string_lossy().to_string(),
            val.as_ref().to_string_lossy().to_string(),
        );
        self
    }
    #[inline]
    pub fn args<V: AsRef<OsStr>>(&mut self, args: impl IntoIterator<Item = V>) -> &mut Command {
        self.args.extend(args.into_iter().map(|i| Arg::new(i)));
        self
    }
    #[inline]
    pub fn envs<V: AsRef<OsStr>>(&mut self, vars: impl IntoIterator<Item = (V, V)>) -> &mut Command {
        self.env.extend(vars.into_iter().map(|(k, v)| {
            (
                k.as_ref().to_string_lossy().to_string(),
                v.as_ref().to_string_lossy().to_string(),
            )
        }));
        self
    }

    #[allow(unused)] // TODO(dij)
    #[inline]
    pub(super) fn spawn_ex(&self, suspended: bool) -> io::Result<Child> {
        self.spawn_init(suspended, &self.stdin, &self.stdout, &self.stderr)
    }

    fn resolve_binary(&self) -> io::Result<String> {
        let n = &self.args.get(0).ok_or_else(|| io::Error::from(ErrorKind::NotFound))?;
        if n.is_empty() {
            return Err(ErrorKind::NotFound.into());
        }
        let env = winapi::GetEnvironment();
        let pathext = self
            .resolve_var(&PATHEXT, env)
            .unwrap_or_else(|| crypt::get_or(0, ".exe;.com;.bat;.cmd").as_bytes().into());
        let exts = pathext
            .split(|v| *v == b';')
            .map(|v| unsafe { core::str::from_utf8_unchecked(v) })
            .collect::<Vec<&str>>();
        if let Some(v) = Command::find_ext(winapi::normalize_path_to_dos(n.as_str()), &exts) {
            return Ok(v);
        }
        if n.as_str()
            .as_bytes()
            .iter()
            .rposition(|x| *x == b'\\' || *x == b'/' || *x == b':')
            .is_some()
        {
            return Err(ErrorKind::NotFound.into());
        }
        let path = env::split_paths(
            self.resolve_var(&PATHEXT[0..4], env)
                .unwrap_or_else(|| winapi::system_root().into())
                .as_str(),
        );
        for i in path {
            let mut v = i.to_string_lossy().to_string();
            v.reserve(4 + n.len());
            v.push('\\');
            v.push_str(n.as_str());
            if let Some(x) = Command::find_ext(winapi::normalize_path_to_dos(v), &exts) {
                return Ok(x);
            }
        }
        Err(ErrorKind::NotFound.into())
    }
    fn find_ext(p: String, e: &[&str]) -> Option<String> {
        if let Some(x) = p.as_bytes().iter().rposition(|v| *v == b'.') {
            if fs::exists(&p) {
                return Some(p);
            }
            if p.len() - x <= 4 {
                return None;
            }
        }
        for i in e {
            let mut t = p.clone();
            t.push_str(i);
            if fs::exists(&t) {
                return Some(t);
            }
        }
        None
    }
    fn cmd_line(force: bool, bin: &str, a: &[Arg]) -> String {
        let mut c = String::with_capacity(bin.len());
        let x = unsafe { c.as_mut_vec() };
        let v = x.iter().any(|v| *v == b' ' || *v == b'\t');
        if v {
            x.push(b'"');
        }
        x.extend_from_slice(bin.as_bytes());
        if v {
            x.push(b'"');
        }
        if a.len() == 1 {
            return c;
        }
        for i in 1..a.len() {
            x.reserve(a[i].len() * 2);
            x.push(b' ');
            let (q, e) = match &a[i] {
                Arg::Raw(_) => (false, false),
                Arg::Quoted(_) => (true, true),
                Arg::Auto(_) if force => (true, true),
                Arg::Auto(v) => (
                    v.is_empty() || v.as_bytes().iter().any(|v| *v == b' ' || *v == b'\t'),
                    true,
                ),
            };
            if q {
                x.push(b'"');
            }
            let mut v = 0;
            for u in a[i].as_str().as_bytes() {
                match *u {
                    b'\\' if e => v += 1,
                    b'"' if e => {
                        x.extend((0..=v).map(|_| b'\\'));
                        v = 0;
                    },
                    _ if e => v = 0,
                    _ => (),
                }
                x.push(*u);
            }
            if q {
                x.extend((0..v).map(|_| b'\\'));
                x.push(b'"')
            }
        }
        c
    }

    fn resolve_var(&self, target: &[u16], env: &StringBlock) -> Option<Blob<u8, 256>> {
        'loop1: for (k, v) in self.env.iter() {
            if k.len() != target.len() {
                continue;
            }
            for (i, c) in k.as_bytes().iter().enumerate() {
                if *c != target[i] as u8 {
                    continue 'loop1;
                }
            }
            return Some(v.as_bytes().into());
        }
        env.iter().find(|v| v.is_key(target)).and_then(|d| d.value_as_blob())
    }
    fn spawn_init(&self, suspended: bool, std_in: &Stdio, std_out: &Stdio, std_err: &Stdio) -> io::Result<Child> {
        // Find and resolve the binary fullpath.
        //  "cmd.exe" = > "C:\Windows\system32\cmd.exe"
        let bin = self.resolve_binary()?;
        // Find the process Parent, if set.
        let parent = Box::new(
            self.filter
                .as_ref()
                .map_or(Ok(None), |f| {
                    if winapi::is_min_windows_vista() {
                        f.handle_func(0x10C0, None).map(|h| Some(h))
                    } else {
                        Ok(None)
                    }
                })
                .map_err(|v| io::Error::from(v))?,
        );
        // ^ Parent is None if value is None.
        // This is in a Box to throw it on the Heap instead of the Stack.'
        //
        // Resolve Stdio/Stdout/Stderr
        //  We don't need to add NULs when using StartupInfo (non-ex), as it will
        //   technically inherit the handles we have anyway.
        //  If we don't set it when using StartupInfoEx, the process may crash
        //   when it's a console app.
        let (mut stdin, mut stdout, mut stderr) = match parent.as_ref() {
            Some(h) => (
                std_in.create(PipeType::Stdin, h.as_handle())?,
                std_out.create(PipeType::Stdout, h.as_handle())?,
                std_err.create(PipeType::Stderr, h.as_handle())?,
            ),
            None => (
                std_in.create(PipeType::Stdin, winapi::CURRENT_PROCESS)?,
                std_out.create(PipeType::Stdout, winapi::CURRENT_PROCESS)?,
                std_err.create(PipeType::Stderr, winapi::CURRENT_PROCESS)?,
            ),
        };
        let pipes = !stdin.0.is_invalid() && !stdout.0.is_invalid() && !stderr.0.is_invalid();
        // Build the Environment var block (at least our half, CreateProcess*
        // takes care of the rest).
        let mut env = Vec::with_capacity(self.env.len());
        for (k, v) in self.env.iter() {
            let mut n = String::with_capacity(k.len() + v.len() + 1);
            n.push_str(&k);
            n.push('=');
            n.push_str(&v);
            env.push(n);
        }
        // Build proper CMD args structure.
        let cmd = Command::cmd_line(self.force_quotes, &bin, &self.args);
        // Make the Window title (if any).
        let t = WChar::from(self.title.as_ref().map(|v| v.as_str()));
        // Take the 'Desktop' value from our current PEB to set the process to our
        // Desktop session.
        let d = WChar::from(unsafe {
            (*(*winapi::GetCurrentProcessPEB()).process_parameters)
                .desktop_info
                .to_string()
        });
        // Build the standard StartupInfo struct.
        let mut i = StartupInfo::default();
        i.flags = self.start_flags;
        i.pos_x = self.start_x;
        i.pos_y = self.start_y;
        i.show_window = self.mode;
        i.size_x = self.start_width;
        i.size_y = self.start_height;
        i.title = t.as_wchar_ptr();
        i.desktop = d.as_wchar_ptr();
        if pipes {
            i.flags |= 0x100; // STARTF_USESTDHANDLES
            i.stdin = stdin.0 .0;
            i.stdout = stdout.0 .0;
            i.stderr = stderr.0 .0;
        }
        // Check for StartupEx support. Bail early if not or if we have nothing
        // to set for an Ex structure.
        let (x, w) = version_support();
        let r = if !x || (!w && !pipes && parent.is_none()) {
            self._spawn(
                &bin,
                &cmd,
                parent.is_some(),
                &env,
                StartInfo::Basic(&i),
                suspended,
            )
        } else {
            // Build StartupInfoEx.
            let mut a = ProcessThreadAttrList::default();
            // At this point we can guarantee that we have Handles to be set (they may be
            // a NUL device). Now we need to check Sec and Parent settings.
            // We also know we have Ex support here.
            let m = Box::new(0x100100000000u64);
            let k = Box::new([stdin.0 .0, stdout.0 .0, stderr.0 .0]);
            match parent.as_ref() {
                Some(h) => {
                    a.set_parent(0, h);
                    a.set_handles(1, k.len(), k.as_ptr());
                    if w {
                        a.set_mitigation(2, m.as_ref());
                    }
                },
                None => {
                    a.set_handles(0, k.len(), k.as_ptr());
                    if w {
                        a.set_mitigation(1, m.as_ref());
                    }
                },
            };
            // Set StartupInfoEx size (if we don't, the function fails!).
            i.size += winapi::PTR_SIZE as u32;
            let v = StartupInfoEx { info: i, attrs: &a };
            // Run internal spawning process, that will check the Tokens and access rights.
            self._spawn(
                &bin,
                &cmd,
                parent.is_some(),
                &env,
                StartInfo::Extended(&v),
                suspended,
            )
        };
        // Close Remote/Parent Handles as they are not needed.
        match *parent {
            Some(ref p) => {
                winapi::DuplicateHandleEx(stdin.0, p, winapi::CURRENT_PROCESS, 0, false, 0x3).map_or((), |h| winapi::close_handle(h));
                winapi::DuplicateHandleEx(stdout.0, p, winapi::CURRENT_PROCESS, 0, false, 0x3).map_or((), |h| winapi::close_handle(h));
                winapi::DuplicateHandleEx(stderr.0, p, winapi::CURRENT_PROCESS, 0, false, 0x3).map_or((), |h| winapi::close_handle(h));
            },
            None => {
                if !stdin.0.is_invalid() {
                    winapi::close_handle(stdin.0)
                }
                if !stdout.0.is_invalid() {
                    winapi::close_handle(stdout.0)
                }
                if !stderr.0.is_invalid() {
                    winapi::close_handle(stderr.0)
                }
            },
        }
        // Break on error after closing Handles.
        let o = r?;
        // Cache reference to the Process Handle to wait on if we need to read.
        let v = o.process.as_handle();
        Ok(Child {
            exit:   AtomicI32::new(0),
            info:   o,
            done:   AtomicBool::new(false),
            stdin:  stdin.1.take().map(|h| {
                // Remove Inheritance
                let _ = winapi::SetHandleInformation(&h, false, false); // IGNORE ERROR
                ChildStdin(h.into())
            }),
            stdout: match stdout.1.take() {
                Some(h) => {
                    // Remove Inheritance
                    let _ = winapi::SetHandleInformation(&h, false, false); // IGNORE ERROR
                    let mut r = ChildStdout {
                        file:    h,
                        event:   winapi::CreateEvent(None, false, false, false, None).map_err(io::Error::from)?,
                        parent:  v,
                        overlap: Box::new(Overlapped::default()),
                    };
                    r.overlap.event = *r.event;
                    Some(r)
                },
                None => None,
            },
            stderr: match stderr.1.take() {
                Some(h) => {
                    // Remove Inheritance
                    let _ = winapi::SetHandleInformation(&h, false, false); // IGNORE ERROR
                    let mut r = ChildStderr(ChildStdout {
                        file:    h,
                        event:   winapi::CreateEvent(None, false, false, false, None).map_err(io::Error::from)?,
                        parent:  v,
                        overlap: Box::new(Overlapped::default()),
                    });
                    r.overlap.event = *r.event;
                    Some(r)
                },
                None => None,
            },
        })
    }
    fn _spawn(&self, name: &str, cmd: &str, p: bool, env: &[String], start: StartInfo, suspended: bool) -> io::Result<ProcessInfo> {
        let target = if self.token.is_invalid() && !p && !winapi::is_windows_xp() {
            winapi::OpenThreadToken(winapi::CURRENT_THREAD, 0xF01FF, true)
                .ok()
                .and_then(|h| {
                    if !winapi::is_user_network_token(&h) {
                        Some(h)
                    } else {
                        None
                    }
                })
        } else {
            None
        };
        // Clear impersonation Token if we're attempting to change users.
        let prev = if self.user.is_some() && target.is_some() {
            winapi::OpenThreadToken(winapi::CURRENT_THREAD, 0xF01FF, true)
                .ok()
                .map_or(None, |h| {
                    // IGNORE ERROR
                    let _ = winapi::SetThreadToken(winapi::CURRENT_THREAD, winapi::INVALID);
                    Some(h) // Only save the Handle if we remove it.
                })
        } else {
            None
        };
        // 0x4 - CREATE_SUSPENDED
        let f = self.flags | if suspended { 0x4 } else { 0 };
        let dir = self.dir.as_ref().map(|v| v.as_str());
        let r = match (self.user.as_ref(), target, self.token.is_invalid()) {
            (Some(u), ..) => {
                winapi::CreateProcessWithLogon(
                    u,
                    // If domain is 'None' check to see if we have a FQDN name.
                    // If not, set the domain value to '.' (local).
                    // (Required by the API call).
                    self.domain.as_ref().map_or_else(
                        || u.find('@').map_or_else(|| Some("."), |_| None),
                        |v| Some(v),
                    ),
                    self.pass.as_ref().map(|v| v.as_str()),
                    0,
                    name,
                    cmd,
                    f,
                    self.split,
                    &env,
                    dir,
                    start,
                )
            },
            (_, _, false) => winapi::CreateProcessWithToken(&self.token, 0x2, name, cmd, f, self.split, &env, dir, start),
            (_, Some(h), _) => winapi::CreateProcessWithToken(h, 0x2, name, cmd, f, self.split, &env, dir, start),
            _ => winapi::CreateProcess(name, cmd, None, None, true, f, self.split, &env, dir, start),
        };
        // Set back our token if we took it off to impersonate.
        // IGNORE ERROR
        let _ = prev.map_or((), |h| {
            winapi::SetThreadToken(winapi::CURRENT_THREAD, h).unwrap_or_default()
        });
        r.map_err(io::Error::from)
    }
}
impl ExitCode {
    pub const SUCCESS: ExitCode = ExitCode(0);
    pub const FAILURE: ExitCode = ExitCode(1);

    #[inline]
    pub fn exit_process(self) -> ! {
        winapi::exit_process(self.0 as u32)
    }
}
impl ExitStatus {
    #[inline]
    pub fn success(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    pub fn code(&self) -> Option<i32> {
        Some(self.0)
    }
    #[inline]
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        if self.0 == 0 {
            Ok(())
        } else {
            Err(ExitStatusError(*self))
        }
    }
}
impl AsyncPipe<'_> {
    #[inline]
    fn new(h: ChildStdout, buf: &mut Vec<u8>) -> AsyncPipe<'_> {
        AsyncPipe { h, pos: 0, buf }
    }

    #[inline]
    fn done(&mut self) {
        self.buf.truncate(self.pos);
    }
    fn check(&mut self) -> io::Result<bool> {
        let (r, z) = self.result(true)?.map_or_else(|| (0, true), |n| (n, n > 0));
        if !z {
            return Ok(false);
        }
        self.pos += r;
        self.lookahead()?;
        Ok(true)
    }
    #[inline]
    fn complete(&mut self) -> io::Result<()> {
        loop {
            if !self.check()? {
                break;
            }
            if let Some(n) = self.read()? {
                if n == 0 {
                    break;
                }
                self.pos += n;
            }
            hint::spin_loop();
        }
        Ok(())
    }
    #[inline]
    fn lookahead(&mut self) -> io::Result<()> {
        loop {
            let (r, z) = self.read()?.map_or((0, false), |n| (n, n > 0));
            self.pos += r;
            if !z {
                break;
            }
        }
        Ok(())
    }
    #[inline]
    fn read(&mut self) -> io::Result<Option<usize>> {
        if self.buf.len() <= self.pos {
            self.buf.resize(self.pos + 0x80, 0);
        }
        winapi::NtReadFile(
            &self.h.file,
            Some(&mut self.h.overlap),
            &mut self.buf[self.pos..],
            None,
        )
        .map_or_else(
            |e| match e {
                Win32Error::IoPending => Ok(None),
                Win32Error::BrokenPipe => Ok(Some(0)),
                _ => Err(e.into()),
            },
            |n| Ok(Some(n)),
        )
    }
    #[inline]
    fn result(&mut self, wait: bool) -> io::Result<Option<usize>> {
        winapi::GetOverlappedResult(&self.h.file, &self.h.overlap, wait).map_or_else(
            |e| match e {
                Win32Error::IoPending => Ok(None),
                Win32Error::BrokenPipe => Ok(Some(0)),
                _ => Err(e.into()),
            },
            |n| Ok(Some(n)),
        )
    }
}
impl ExitStatusError {
    #[inline]
    pub fn code(&self) -> Option<i32> {
        Some(self.0 .0)
    }
    #[inline]
    pub fn into_status(self) -> ExitStatus {
        self.0
    }
}

impl CommandExt for Command {
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
        self.flags = if new { self.flags | 0x10 } else { self.flags ^ 0x10 };
        self
    }
    #[inline]
    fn set_no_window(&mut self, window: bool) -> &mut Command {
        // 0x8000000 - CREATE_NO_WINDOW
        self.flags = if window {
            self.flags | 0x8000000
        } else {
            self.flags ^ 0x8000000
        };
        self
    }
    #[inline]
    fn set_detached(&mut self, detached: bool) -> &mut Command {
        // 0x8  - DETACHED_PROCESS
        // 0x10 - CREATE_NEW_CONSOLE
        self.flags = if detached {
            (self.flags | 0x8) ^ 0x10
        } else {
            self.flags ^ 0x8
        };
        self
    }
    #[inline]
    fn set_token(&mut self, token: OwnedHandle) -> &mut Command {
        self.token = token;
        self
    }
    #[inline]
    fn set_suspended(&mut self, suspended: bool) -> &mut Command {
        // 0x4 - CREATE_SUSPENDED
        self.flags = if suspended { self.flags | 0x4 } else { self.flags ^ 0x4 };
        self
    }
    #[inline]
    fn set_inherit_env(&mut self, inherit: bool) -> &mut Command {
        self.split = !inherit;
        self
    }
    #[inline]
    fn set_fullscreen(&mut self, fullscreen: bool) -> &mut Command {
        // 0x20 - STARTF_RUNFULLSCREEN
        self.start_flags = if fullscreen {
            self.start_flags | 0x20
        } else {
            self.start_flags ^ 0x20
        };
        self
    }
    #[inline]
    fn set_window_display(&mut self, display: i16) -> &mut Command {
        // 0x1 - STARTF_USESHOWWINDOW
        (self.mode, self.start_flags) = if display < 0 {
            (0, self.start_flags ^ 0x1)
        } else {
            (display as u16, self.start_flags | 0x1)
        };
        self
    }
    #[inline]
    fn set_window_position(&mut self, x: u32, y: u32) -> &mut Command {
        (self.start_x, self.start_y) = (x, y);
        // 0x4 - STARTF_USEPOSITION
        self.start_flags |= 0x4;
        self
    }
    #[inline]
    fn set_parent(&mut self, filter: impl MaybeFilter) -> &mut Command {
        self.filter = filter.into_filter();
        // 0x10 - CREATE_NEW_CONSOLE
        self.flags = if self.filter.is_some() {
            self.flags | 0x10
        } else {
            self.flags ^ 0x10
        };
        self
    }
    #[inline]
    fn set_window_size(&mut self, width: u32, height: u32) -> &mut Command {
        (self.start_width, self.start_height) = (width, height);
        // 0x2 - STARTF_USESIZE
        self.start_flags |= 0x2;
        self
    }
    #[inline]
    fn set_window_title(&mut self, title: impl MaybeString) -> &mut Command {
        (self.title, self.start_flags) = title.into_string().map_or((None, self.start_flags ^ 0x1000), |v| {
            (Some(v.to_string()), self.start_flags | 0x1000)
        });
        // 0x1000 - STARTF_TITLEISAPPID
        self
    }
    #[inline]
    fn output_in_combo(&mut self, out: &mut Vec<u8>) -> io::Result<ExitStatus> {
        self.spawn_init(false, &self.stdin, &Stdio::piped(), &Stdio::piped())?
            .wait_with_output_in_combo(out)
    }
    #[inline]
    fn output_in(&mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus> {
        self.spawn_init(false, &self.stdin, &Stdio::piped(), &Stdio::piped())?
            .wait_with_output_in(stdout, stderr)
    }
    #[inline]
    fn set_login<V: AsRef<str>>(&mut self, user: Option<V>, domain: Option<V>, password: Option<V>) -> &mut Command {
        self.user = user.map(|v| v.as_ref().to_string());
        self.user = domain.map(|v| v.as_ref().to_string());
        self.user = password.map(|v| v.as_ref().to_string());
        self
    }
}

impl From<Child> for OwnedHandle {
    #[inline]
    fn from(v: Child) -> OwnedHandle {
        v.info.process
    }
}
impl From<ChildStdout> for OwnedHandle {
    #[inline]
    fn from(v: ChildStdout) -> OwnedHandle {
        v.file
    }
}
impl From<ChildStderr> for OwnedHandle {
    #[inline]
    fn from(v: ChildStderr) -> OwnedHandle {
        v.0.file
    }
}

impl Drop for AsyncPipe<'_> {
    #[inline]
    fn drop(&mut self) {
        // Cancel any pending IO.
        // IGNORE ERROR
        let _ = winapi::CancelIoEx(&self.h.file, &mut self.h.overlap);
    }
}

impl AsHandle for Stdio {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.h
    }
}
impl From<File> for Stdio {
    #[inline]
    fn from(v: File) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<Handle> for Stdio {
    #[inline]
    fn from(v: Handle) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.into(),
        }
    }
}
impl From<ChildStdin> for Stdio {
    #[inline]
    fn from(v: ChildStdin) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.0.into(),
        }
    }
}
impl From<ChildStdout> for Stdio {
    #[inline]
    fn from(v: ChildStdout) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.file.into(),
        }
    }
}
impl From<ChildStderr> for Stdio {
    #[inline]
    fn from(v: ChildStderr) -> Stdio {
        Stdio {
            v: StdioType::Handle,
            h: v.0.file.into(),
        }
    }
}
impl From<OwnedHandle> for Stdio {
    #[inline]
    fn from(v: OwnedHandle) -> Stdio {
        Stdio { v: StdioType::Handle, h: v }
    }
}

impl Eq for Output {}
impl Clone for Output {
    #[inline]
    fn clone(&self) -> Output {
        Output {
            status: self.status,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        }
    }
}
impl PartialEq for Output {
    #[inline]
    fn eq(&self, other: &Output) -> bool {
        self.status == other.status && self.stdout == other.stdout && self.stderr == other.stderr
    }
}

impl AsHandle for Child {
    #[inline]
    fn as_handle(&self) -> Handle {
        *self.info.process
    }
}
impl ChildExt for Child {}
impl ChildExtra for Child {
    #[inline]
    fn wait_with_output_in_combo(self, out: &mut Vec<u8>) -> io::Result<ExitStatus> {
        let mut e = Vec::new();
        let r = self.wait_with_output_in(out, &mut e);
        if !e.is_empty() {
            out.reserve(e.len() + 1);
            out.push(b'\n');
            out.extend_from_slice(&e);
        }
        r
    }
    fn wait_with_output_in(mut self, stdout: &mut Vec<u8>, stderr: &mut Vec<u8>) -> io::Result<ExitStatus> {
        drop(self.stdin.take()); // Close Stdin Handle.
        match (self.stdout.take(), self.stderr.take()) {
            (Some(std_out), Some(std_err)) => {
                self.read_to_end_both(std_out, std_err, stdout, stderr)?;
            },
            (Some(mut std_out), None) => {
                std_out.read_to_end(stdout)?;
            },
            (None, Some(mut std_err)) => {
                std_err.read_to_end(stderr)?;
            },
            (None, None) => (),
        }
        Ok(self.exit_code())
    }
}

impl Deref for ChildStdin {
    type Target = File;

    #[inline]
    fn deref(&self) -> &File {
        &self.0
    }
}
impl AsHandle for ChildStdin {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0.as_handle()
    }
}
impl DerefMut for ChildStdin {
    #[inline]
    fn deref_mut(&mut self) -> &mut File {
        &mut self.0
    }
}

impl Read for ChildStdout {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        winapi::NtReadFile(&self.file, Some(&mut self.overlap), buf, None)
            .or_else(|e| {
                if e != Win32Error::IoPending {
                    Err(e)
                } else {
                    let _ = winapi::wait_for_multiple_objects(&[self.event.0, self.parent.0], 2, false, -1, false);
                    Ok(self.overlap.internal_high)
                }
            })
            .map_err(io::Error::from)
    }
}
impl AsHandle for ChildStdout {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.file.as_handle()
    }
}

impl Read for ChildStderr {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}
impl Deref for ChildStderr {
    type Target = ChildStdout;

    #[inline]
    fn deref(&self) -> &ChildStdout {
        &self.0
    }
}
impl AsHandle for ChildStderr {
    #[inline]
    fn as_handle(&self) -> Handle {
        self.0.file.as_handle()
    }
}
impl DerefMut for ChildStderr {
    #[inline]
    fn deref_mut(&mut self) -> &mut ChildStdout {
        &mut self.0
    }
}

impl Copy for ExitCode {}
impl Clone for ExitCode {
    #[inline]
    fn clone(&self) -> ExitCode {
        ExitCode(self.0)
    }
}
impl Deref for ExitCode {
    type Target = i32;

    #[inline]
    fn deref(&self) -> &i32 {
        &self.0
    }
}
impl From<u8> for ExitCode {
    #[inline]
    fn from(v: u8) -> ExitCode {
        ExitCode(v as i32)
    }
}
impl Termination for ExitCode {
    #[inline]
    fn report(self) -> ExitCode {
        self
    }
}
impl ExitCodeExt for ExitCode {
    #[inline]
    fn from_raw(raw: u32) -> ExitCode {
        ExitCode(raw as i32)
    }
}

impl Eq for ExitStatus {}
impl Copy for ExitStatus {}
impl Clone for ExitStatus {
    #[inline]
    fn clone(&self) -> ExitStatus {
        ExitStatus(self.0)
    }
}
impl Deref for ExitStatus {
    type Target = i32;

    #[inline]
    fn deref(&self) -> &i32 {
        &self.0
    }
}
impl PartialEq for ExitStatus {
    #[inline]
    fn eq(&self, other: &ExitStatus) -> bool {
        self.0 == other.0
    }
}
impl ExitStatusExt for ExitStatus {
    #[inline]
    fn from_raw(raw: u32) -> ExitStatus {
        ExitStatus(raw as i32)
    }
}
impl From<ExitStatusError> for ExitStatus {
    #[inline]
    fn from(v: ExitStatusError) -> ExitStatus {
        v.0
    }
}

impl Eq for ExitStatusError {}
impl Copy for ExitStatusError {}
impl Error for ExitStatusError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for ExitStatusError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Clone for ExitStatusError {
    #[inline]
    fn clone(&self) -> ExitStatusError {
        ExitStatusError(self.0)
    }
}
impl Display for ExitStatusError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut b = [0u8; 21];
        f.write_str(self.0 .0.into_str(&mut b))
    }
}
impl PartialEq for ExitStatusError {
    #[inline]
    fn eq(&self, other: &ExitStatusError) -> bool {
        self.0 == other.0
    }
}

impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;

    #[inline]
    fn next(&mut self) -> Option<&'a OsStr> {
        self.iter.next().map(|arg| OsStr::new(arg.as_str()))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
impl<'a> ExactSizeIterator for CommandArgs<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> Iterator for CommandEnvs<'a> {
    type Item = (&'a OsStr, Option<&'a OsStr>);

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[inline]
    fn next(&mut self) -> Option<(&'a OsStr, Option<&'a OsStr>)> {
        self.iter
            .next()
            .map(|(key, value)| (OsStr::new(key), Some(OsStr::new(value))))
    }
}
impl<'a> ExactSizeIterator for CommandEnvs<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

#[inline]
pub fn id() -> u32 {
    winapi::GetCurrentProcessID()
}
#[inline]
pub fn abort() -> ! {
    exit(1)
}
#[inline]
pub fn parent_id() -> u32 {
    winapi::current_process_info().map_or(0u32, |i| i.parent_process_id as u32)
}
#[inline]
pub fn exit(exit_code: i32) -> ! {
    winapi::exit_process(exit_code as u32)
}
#[inline]
pub fn processes() -> io::Result<Vec<ProcessEntry>> {
    winapi::list_processes().map_err(io::Error::from)
}
#[inline]
pub fn threads(pid: u32) -> io::Result<Vec<ThreadEntry>> {
    winapi::list_threads(pid).map_err(io::Error::from)
}

#[inline]
fn version_support() -> (bool, bool) {
    match VERSION.compare_exchange(0, 0x80, Ordering::AcqRel, Ordering::Relaxed) {
        Ok(_) => {
            let (m, x, _) = winapi::GetVersionNumbers();
            let r = match m {
                0..=5 => (false, false),
                6 => (x > 2, true),
                _ => (true, true),
            };
            VERSION.store(
                0x80 | if r.0 { 0x1 } else { 0 } | if r.1 { 0x2 } else { 0 },
                Ordering::Release,
            );
            r
        },
        Err(v) => (v & 0x1 != 0, v & 0x2 != 0),
    }
}

#[cfg(feature = "implant")]
mod display {
    use core::fmt::{self, Debug, Formatter};

    use super::ExitStatus;
    use crate::util::stx::prelude::*;
    use crate::util::ToStr;

    impl Debug for ExitStatus {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let mut b = [0; 21];
            f.write_str(self.0.into_str(&mut b))
        }
    }
    impl ToString for ExitStatus {
        #[inline]
        fn to_string(&self) -> String {
            self.0.into_string()
        }
    }
}
#[cfg(not(feature = "implant"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use super::ExitStatus;
    use crate::util::stx::prelude::*;

    impl Debug for ExitStatus {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for ExitStatus {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "exit {}", self.0)
        }
    }
}
