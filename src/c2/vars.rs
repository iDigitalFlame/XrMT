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
use core::cmp::Ordering;
use core::fmt::Display;

use crate::c2::{CoreError, CoreResult, Transform, Wrapper};
use crate::com::{Flag, Packet, PacketAddError};
use crate::data::crypto::KeyPair;
use crate::data::{Chunk, Writable, Writer};
use crate::fs::{DirEntry, Metadata};
use crate::ignore_error;
use crate::io::{self, Read, Write};
use crate::prelude::*;
use crate::sync::mpsc::{SyncSender, TrySendError};

const MAX_FRAGS: u16 = 0xFFFFu16;
const MAX_SWEEPS: u8 = 5u8;

pub enum InfoClass {
    Hello,
    Migrate,
    Refresh,
    Sync,
    Proxy,
    SyncAndMigrate,
    Invalid,
}

pub struct FileEntry {
    pub dir:  DirEntry,
    pub meta: Option<Metadata>,
}
pub struct Cluster<A: Allocator = Global> {
    max:   u16,
    data:  Vec<Frag<A>, A>,
    empty: u16,
    count: u8,
}

struct Frag<A: Allocator = Global>(Packet<A>);

impl Cluster {
    #[inline]
    pub fn new(n: Packet) -> CoreResult<Cluster> {
        Cluster::new_in(n, Global)
    }
}
impl FileEntry {
    #[inline]
    pub fn new(d: DirEntry) -> FileEntry {
        let m = d.metadata().ok();
        FileEntry { dir: d, meta: m }
    }

    #[inline]
    fn is_dir(&self) -> bool {
        self.meta.as_ref().map_or(false, |v| v.is_dir())
    }
}
impl<A: Allocator> Cluster<A> {
    #[inline]
    pub fn new_in(n: Packet<A>, alloc: A) -> CoreResult<Cluster<A>> {
        let mut c = Cluster {
            max:   0u16,
            data:  Vec::new_in(alloc),
            empty: 0u16,
            count: MAX_SWEEPS,
        };
        c.add(n)?;
        Ok(c)
    }

    #[inline]
    pub fn clear(&mut self) {
        for i in self.data.iter_mut() {
            i.0.clear()
        }
    }
    #[inline]
    pub fn decrement(&mut self) -> bool {
        self.count = self.count.checked_sub(1).unwrap_or(0);
        self.count == 0
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        self.data.len() > 0 && (self.data.len() as u16) > (self.max + self.empty)
    }
    pub fn add(&mut self, n: Packet<A>) -> CoreResult<()> {
        if self.data.len() > 0 && !self.data[0].0.belongs(&n) {
            return Err(CoreError::InvalidPacketFrag);
        }
        // Reset Counter
        (self.count, self.max) = (MAX_SWEEPS, n.flags.len() - 1);
        if n.is_empty() {
            self.empty += 1;
        } else {
            self.data.push(Frag(n));
        }
        Ok(())
    }
}

impl From<u8> for InfoClass {
    #[inline]
    fn from(v: u8) -> InfoClass {
        match v {
            0 => InfoClass::Hello,
            1 => InfoClass::Migrate,
            2 => InfoClass::Refresh,
            3 => InfoClass::Sync,
            4 => InfoClass::Proxy,
            5 => InfoClass::SyncAndMigrate,
            _ => InfoClass::Invalid,
        }
    }
}

impl Eq for FileEntry {}
impl Ord for FileEntry {
    #[inline]
    fn cmp(&self, other: &FileEntry) -> Ordering {
        match (self.is_dir(), other.is_dir()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => (),
        }
        self.dir.file_name().cmp(&other.dir.file_name())
    }
}
impl PartialEq for FileEntry {
    #[inline]
    fn eq(&self, other: &FileEntry) -> bool {
        (self.is_dir() == other.is_dir()) && self.dir.file_name().eq(&other.dir.file_name())
    }
}
impl PartialOrd for FileEntry {
    #[inline]
    fn partial_cmp(&self, other: &FileEntry) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<A: Allocator> Eq for Frag<A> {}
impl<A: Allocator> Ord for Frag<A> {
    #[inline]
    fn cmp(&self, other: &Frag<A>) -> Ordering {
        self.0.flags.position().cmp(&other.0.flags.position())
    }
}
impl<A: Allocator> PartialEq for Frag<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.flags.position().eq(&other.0.flags.position())
    }
}
impl<A: Allocator> PartialOrd for Frag<A> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.flags.position().partial_cmp(&other.0.flags.position())
    }
}

impl<A: Allocator> Into<Packet<A>> for Cluster<A> {
    fn into(mut self) -> Packet<A> {
        self.data.sort();
        let mut n = self.data.remove(0).0;
        for i in self.data.into_iter() {
            ignore_error!(n.add(i.0)); // CANT ERROR, WE CHECKED IT ALREADY
        }
        n.flags.clear();
        n
    }
}

#[inline]
pub fn is_packet_nop(n: &Packet) -> bool {
    n.id < 2 && n.is_empty() && (n.flags == 0 || n.flags == Flag::PROXY)
}
#[inline]
pub fn key_crypt<A: Allocator>(n: &mut Packet, k: &KeyPair<A>) {
    let s = k.shared_key();
    let c = s.len();
    let b = n.as_mut_slice();
    for i in 0..b.len() {
        b[i] = b[i] ^ s[i % c]
    }
}
#[inline]
pub fn error_to_packet<A: Allocator>(n: &mut Packet<A>, msg: impl Display) {
    n.clear();
    ignore_error!(n.write_string(&msg.to_string())); // DOES NOT ERROR
    n.flags |= Flag::ERROR;
}
#[inline]
pub fn io_error_to_packet<A: Allocator>(n: &mut Packet<A>, msg: io::Error) {
    n.clear();
    ignore_error!(n.write_string(&msg.to_string())); // DOES NOT ERROR
    n.flags |= Flag::ERROR;
}
pub fn write_unpack(d: &mut Packet, s: &Packet) -> Result<(), PacketAddError> {
    if s.flags & Flag::MULTI != 0 || s.flags & Flag::MULTI_DEVICE != 0 {
        let n = s.flags.len();
        if n == 0 {
            return Err(PacketAddError::InvalidCount);
        }
        if n + d.flags.len() > MAX_FRAGS {
            return Err(PacketAddError::LimitError);
        }
        //ignore_error!(s.write_stream(d)); // CAN'T ERROR
        ignore_error!(d.extend_from_slice(&s)); // CAN'T ERROR
        d.flags.set_len(d.flags.len() + n);
        return Ok(());
    }
    if d.flags.len() + 1 > MAX_FRAGS {
        return Err(PacketAddError::LimitError);
    }
    ignore_error!(s.write_stream(d)); // CAN'T ERROR
    d.flags.set_len(d.flags.len() + 1);
    if s.flags & Flag::CHANNEL != 0 {
        d.flags |= Flag::CHANNEL;
    }
    if s.flags & Flag::MULTI_DEVICE != 0 {
        d.flags |= Flag::MULTI_DEVICE;
    }
    d.flags |= Flag::MULTI;
    if s.tags.len() > 0 {
        d.tags.extend_from_slice(&s.tags);
        d.tags.dedup();
    }
    Ok(())
}
#[inline]
pub fn try_send<A: Allocator>(wait: bool, c: &SyncSender<Packet<A>>, n: Packet<A>) -> Option<Packet<A>> {
    if wait {
        c.send(n).err().map(|e| e.0)
    } else {
        c.try_send(n).err().map(|e| match e {
            TrySendError::Full(v) => v,
            TrySendError::Disconnected(v) => v,
        })
    }
}
pub fn read_packet<'a, A: Allocator>(input: &mut impl Read, w: &Wrapper<'a, A>, t: &Transform<'a, A>) -> io::Result<Packet> {
    if w.is_none() && t.is_none() {
        return Packet::from_reader(input);
    }
    if t.is_none() {
        let mut r = w.unwrap(input)?;
        return Packet::from_reader(&mut r);
    }
    let mut y = Chunk::new();
    {
        let mut x = Chunk::new();
        input.read_to_end(x.as_mut())?;
        t.read(&x, &mut y)?;
        x.clear();
    }
    {
        let mut r = w.unwrap(&mut y)?;
        Packet::from_reader(&mut r)
    }
}
pub fn write_packet<'a, A: Allocator, B: Allocator>(out: &mut impl Write, w: &Wrapper<'a, A>, t: &Transform<'a, A>, n: Packet<B>) -> io::Result<()> {
    if w.is_none() && t.is_none() {
        return n.write_packet(out);
    }
    if t.is_none() {
        let mut w = w.wrap(out)?;
        return n.write_packet(&mut w);
    }
    let mut x = Chunk::new();
    {
        // Inner wrap to drop Writer after this.
        let mut w = w.wrap(&mut x)?;
        n.write_packet(&mut w)?;
    }
    t.write(&x, out)
}
