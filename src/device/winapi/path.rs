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

use crate::data::blob::Blob;
use crate::device::winapi;
use crate::prelude::*;

const PREFIX: [u8; 8] = [b'\\', b'?', b'?', b'\\', b'U', b'N', b'C', b'\\'];

pub fn normalize_path_to_nt(path: impl AsRef<str>) -> String {
    // BUG(dij): Not sure if these are bugs we should worry about tbh?
    // https://googleprojectzero.blogspot.com/2016/02/the-definitive-guide-on-win32-to-nt.html
    //
    // "Bugs":
    // - We're ignoring special device names.
    // - Adding the "\??\" prefix does NOT change the Win32 path.
    //
    // TODO(dij): Do we need to worry about special device names?
    let b = path.as_ref().as_bytes();
    if b.is_empty() {
        return String::new();
    }
    match 0 {
        // NT Full Path "\??\C:\Windows\System32\notepad.exe"
        // - Does not parse the string and returns it directly.
        //   - All "//", "\\", "," or ".." are NOT parsed and treated as raw file name data.
        // - MUST match the "\??\" prefix and does NOT accept '/' in place.
        _ if b.len() >= 4 && b[0] == b'\\' && b[1] == b'?' && b[2] == b'?' && b[3] == b'\\' => path.as_ref().to_string(),
        // NT Full Device Path "\Device\Ntfs\path"
        // - Does not parse the string and returns it directly.
        //   - All "//", "\\", "," or ".." are NOT parsed and treated as raw file name data.
        // - MUST match the "\Device\" prefix and does NOT accept '/' in place.
        _ if b.len() >= 9 && b[0] == b'\\' && (b[1] == b'D' || b[1] == b'd') && (b[6] == b'E' || b[6] == b'e') && b[7] == b'\\' => path.as_ref().to_string(),
        // Win32 Full Path "C:\Windows\System32\notepad.exe"
        // - Converts all '/' to '\'
        // - Collapses  duplicate slashes and "."
        // - Parses ".." up until the drive letter.
        //   - C:\Windows\..\..\..\..\..\..\ == C:\
        //   - Always ends with a '\'.
        // - Collapses '.' on the END of the path only.
        //   - C:\cmd.exe....e.... == C:\cmd.exe....e
        _ if b.len() >= 3 && b[1] == b':' && ((b[0] >= b'a' && b[0] <= b'z') || (b[0] >= b'A' && b[0] <= b'Z')) && sep(b[2]) => unsafe { convert_full(b) },
        // Win32 Drive-Local Path "C:Windows\System32\notepad.exe"
        // - Converts all '/' to '\'
        // - Collapses  duplicate slashes and "."
        // - Replaces "<Letter>:" with the current directory of that drive letter.
        // - Parses ".." up until the drive letter.
        //   - C:\Windows\..\..\..\..\..\..\ == C:\
        //   - Always ends with a '\'.
        // - Collapses '.' on the END of the path only.
        //   - C:\cmd.exe....e.... == C:\cmd.exe....e
        _ if b.len() >= 3 && b[1] == b':' && ((b[0] >= b'a' && b[0] <= b'z') || (b[0] >= b'A' && b[0] <= b'Z')) => unsafe { convert_drive(b) },
        // Win32 Device Path "\\.\NamedPipes\Pipe\MyPipe"
        // - Converts all '/' to '\'
        // - Collapses  duplicate slashes and "."
        // - Parses ".." up until the "\\.\".
        //   - \\.\NamedPipes\Pipe\..\..\..\..\..\ == \\.\
        //   - Always ends with a '\'.
        // - Collapses '.' on the END of the path only.
        //   - C:\cmd.exe....e.... == C:\cmd.exe....e
        _ if b.len() >= 4 && sep(b[0]) && sep(b[1]) && b[2] == b'.' && sep(b[3]) => unsafe { convert_device(b) },
        // UNC (Network) Path "\\server\c$\Windows\System32\notepad.exe"
        // - Converts all '/' to '\'
        // - Does not touch the "server" and "share_path" sections.
        //   - Does convert the '/' between them to a '\'.
        // - Collapses  duplicate slashes and "."
        // - Parses ".." up until the share path.
        //   - \\server\share_path\Windows\System32\..\..\..\..\..\..\ == \\server\share_path\
        //   - Always ends with a '\'.
        // - Collapses '.' on the END of the path only.
        //   - C:\cmd.exe....e.... == C:\cmd.exe....e
        _ if b.len() >= 4 && sep(b[0]) && sep(b[1]) && b[2] != b'?' => unsafe { convert_unc(b) },
        // NT Short Path "\\?\C:Windows\System32\notepad.exe"
        // - Does not parse the string and returns it with the full prefix "\??\".
        //   - All "//", "\\", "," or ".." are NOT parsed and treated as raw file name data.
        // - MUST match the "\??\" prefix and does NOT accept '/' in place.
        _ if b.len() >= 3 && b[0] == b'\\' && b[1] == b'\\' && b[2] == b'?' => unsafe {
            let mut v = String::with_capacity(b.len() + 1);
            let x = v.as_mut_vec();
            x.extend_from_slice(&PREFIX[0..2]);
            x.extend_from_slice(&b[2..]);
            v
        },
        // NT Device Path? "//?/NamedPipes\Pipe\MyPipe"
        // - Acts like the Win32 Device Path
        // - Seems to be a weird quirk.
        _ if b.len() >= 3 && sep(b[0]) && sep(b[1]) && b[2] == b'?' => unsafe { convert_device(b) },
        // Win32 Drive-Local Without Path "C:"
        // - Replaces "<Letter>:" with the current directory of that drive letter.
        _ if b.len() == 2 && b[1] == b':' && ((b[0] >= b'a' && b[0] <= b'z') || (b[0] >= b'A' && b[0] <= b'Z')) => unsafe { convert_drive(b) },
        // Win32 Empty Device Path "\\."
        // - Returns static "\??\".
        _ if b.len() == 2 && sep(b[0]) && sep(b[1]) && b[2] == b'.' => unsafe { core::str::from_utf8_unchecked(&PREFIX[0..4]) }.to_string(),
        // UNC (Network) Empty Path "\\" or "//"
        // - Returns static "\??\UNC\".
        _ if b.len() == 2 && sep(b[0]) && sep(b[1]) => unsafe { core::str::from_utf8_unchecked(&PREFIX) }.to_string(),
        // Mostly absolute paths, we'll append the current dir to them (unless they start with a '/' or '\').
        // These are then treated as Win32 Full Paths.
        // - Converts all '/' to '\'
        // - Collapses  duplicate slashes and "."
        // - Parses ".." up until the drive letter.
        //   - C:\Windows\..\..\..\..\..\..\ == C:\
        //   - Always ends with a '\'.
        // - Collapses '.' on the END of the path only.
        //   - C:\cmd.exe....e.... == C:\cmd.exe....e
        _ => unsafe { convert_absolute(b) },
    }
}
#[inline]
pub fn normalize_path_to_dos(path: impl AsRef<str>) -> String {
    normalize_path_to_nt(path)[4..].to_string()
}

#[inline]
fn sep(b: u8) -> bool {
    b == b'\\' || b == b'/'
}
fn check_dots(pos: usize, end: usize, buf: &[u8]) -> usize {
    let (mut p, mut e) = (0, false);
    for i in pos..end {
        match buf[i] {
            b'.' | b' ' if !e => (p, e) = (i, true),
            b'.' | b' ' => (),
            _ if e => e = false,
            _ => (),
        }
    }
    if e {
        p
    } else {
        end
    }
}
fn split_combine(path: &[u8], start: usize) -> (Vec<&[u8]>, usize, bool) {
    let (mut t, mut x) = (0usize, start);
    let mut d: Vec<&[u8]> = Vec::new();
    for i in start..path.len() {
        if !sep(path[i]) {
            continue;
        }
        match i - x {
            2 if path[x] == b'.' && path[x + 1] == b'.' => match d.pop() {
                Some(v) => t -= v.len(),
                None => (),
            },
            1 if path[x] == b'.' => (),
            0 => (),
            _ => {
                d.push(&path[x..i]);
                t += i - x;
            },
        }
        x = i + 1;
    }
    let mut e = sep(path[path.len() - 1]);
    if x < path.len() {
        match path.len() - x {
            2 if path[x] == b'.' && path[x + 1] == b'.' => match d.pop() {
                Some(v) => t -= v.len(),
                None => (),
            },
            1 if path[x] == b'.' => (),
            0 => (),
            _ => {
                let c = check_dots(x, path.len(), path);
                if c > x {
                    d.push(&path[x..c]);
                    t += c - x;
                    if c < path.len() {
                        e = sep(path[c]);
                    }
                } else if c == x {
                    e = sep(path[c - 1])
                }
            },
        }
    }
    (d, t, e)
}

unsafe fn convert_unc(b: &[u8]) -> String {
    let mut r = String::with_capacity(8);
    let x = r.as_mut_vec();
    x.extend_from_slice(&PREFIX);
    if b.len() == 2 {
        return r;
    }
    let (mut c, mut s, mut h) = (0u32, 0usize, 0usize);
    for i in 2..b.len() {
        if !sep(b[i]) {
            continue;
        }
        c += 1;
        match c {
            2 => {
                if i > 0 && sep(b[i - 1]) {
                    h = i
                } else {
                    s = i
                }
                break;
            },
            1 => h = i,
            _ => (),
        }
    }
    let (d, n, e) = split_combine(b, s);
    x.reserve(n);
    if s > 0 {
        if h > 0 {
            x.extend_from_slice(&b[2..h]);
            x.push(b'\\');
            x.extend_from_slice(&b[h + 1..s]);
        } else {
            x.extend_from_slice(&b[2..s]);
        }
        if (n > 0 || s - (h + 1) > 0) && h - 2 > 0 && (!d.is_empty() || e) {
            x.push(b'\\');
        }
    } else if h <= 3 {
        x.push(b'\\');
    }
    if d.is_empty() {
        return r;
    }
    for (i, v) in d.iter().enumerate() {
        if i > 0 {
            x.push(b'\\');
        }
        x.extend_from_slice(v);
    }
    if e {
        x.push(b'\\');
    }
    r
}
unsafe fn convert_full(b: &[u8]) -> String {
    let (d, n, e) = split_combine(b, 3);
    let mut r = String::with_capacity(8 + n);
    let x = r.as_mut_vec();
    x.extend_from_slice(&PREFIX[0..4]);
    x.extend_from_slice(&b[0..2]);
    x.push(b'\\');
    if d.is_empty() {
        return r;
    }
    for (i, v) in d.iter().enumerate() {
        if i > 0 {
            x.push(b'\\');
        }
        x.extend_from_slice(v);
    }
    if e {
        x.push(b'\\');
    }
    r
}
unsafe fn convert_drive(b: &[u8]) -> String {
    let c = winapi::current_directory();
    let d = match 0 {
        _ if b[0] < b'a' && c[0] >= b'a' && b[0] == c[0] - 0x20 => Some(c),
        _ if b[0] >= b'a' && c[0] < b'a' && b[0] - 0x20 == c[0] => Some(c),
        _ if b[0] == c[0] => Some(c),
        _ => {
            let v = [
                b'=' as u16,
                if b[0] >= b'a' { b[0] - 0x20 } else { b[0] } as u16,
                b':' as u16,
            ];
            winapi::GetEnvironment()
                .iter()
                .find(|k| k.is_key(&v))
                .and_then(|i| i.value_as_blob())
        },
    };
    let mut r = match d {
        Some(v) => v,
        None => {
            let mut v = Blob::new();
            v.extend_from_slice(&b[0..2]);
            v
        },
    };
    r.reserve(b.len());
    if r.last().map(|v| !sep(*v)).unwrap_or_default() {
        r.push(b'\\');
    }
    r.extend_from_slice(&b[2..]);
    convert_full(&r)
}
unsafe fn convert_device(b: &[u8]) -> String {
    let (d, n, e) = split_combine(b, 3);
    let mut r = String::with_capacity(8 + n);
    let x = r.as_mut_vec();
    x.extend_from_slice(&PREFIX[0..4]);
    if d.is_empty() {
        return r;
    }
    for (i, v) in d.iter().enumerate() {
        if i > 0 {
            x.push(b'\\');
        }
        x.extend_from_slice(v);
    }
    if e {
        x.push(b'\\');
    }
    r
}
unsafe fn convert_absolute(b: &[u8]) -> String {
    let mut c = winapi::current_directory();
    if sep(b[0]) {
        // NOTE(dij): The '\' first path operator only works on non-UNC paths
        //            so this should be fine.
        if let Some(i) = c.iter().position(|v| *v == b':') {
            c.truncate(i + 1);
            c.extend_from_slice(b);
            return convert_full(&c);
        }
    }
    c.reserve(b.len());
    if c.last().map(|v| !sep(*v)).unwrap_or_default() {
        c.push(b'\\');
    }
    c.extend_from_slice(b);
    convert_full(&c)
}
