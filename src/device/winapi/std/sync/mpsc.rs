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

use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};
use core::cell::UnsafeCell;
use core::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use core::mem;
use core::time::Duration;

use crate::device::winapi::{self, AsHandle};
use crate::ignore_error;
use crate::io::ErrorKind;
use crate::prelude::*;
use crate::sync::Event;

pub enum TryRecvError {
    Empty,
    Disconnected,
}
pub enum TrySendError<T> {
    Full(T),
    Disconnected(T),
}
pub enum RecvTimeoutError {
    Timeout,
    Disconnected,
}

impl Copy for TryRecvError {}
impl Clone for TryRecvError {
    #[inline]
    fn clone(&self) -> TryRecvError {
        *self
    }
}
impl Eq for TryRecvError {}
impl PartialEq for TryRecvError {
    #[inline]
    fn eq(&self, other: &TryRecvError) -> bool {
        match (self, other) {
            (TryRecvError::Empty, TryRecvError::Empty) => true,
            (TryRecvError::Disconnected, TryRecvError::Disconnected) => true,
            _ => false,
        }
    }
}

impl Copy for RecvTimeoutError {}
impl Clone for RecvTimeoutError {
    #[inline]
    fn clone(&self) -> RecvTimeoutError {
        *self
    }
}
impl Eq for RecvTimeoutError {}
impl PartialEq for RecvTimeoutError {
    #[inline]
    fn eq(&self, other: &RecvTimeoutError) -> bool {
        match (self, other) {
            (RecvTimeoutError::Timeout, RecvTimeoutError::Timeout) => true,
            (RecvTimeoutError::Disconnected, RecvTimeoutError::Disconnected) => true,
            _ => false,
        }
    }
}

pub struct RecvError;
pub struct SendError<T>(pub T);
pub struct IntoIter<T>(Receiver<T>);
pub struct Sender<T>(Weak<Inner<T>>);
pub struct Receiver<T>(Arc<Inner<T>>);
pub struct SyncSender<T>(Weak<Inner<T>>);
pub struct Iter<'a, T: 'a>(&'a Receiver<T>);
pub struct TryIter<'a, T: 'a>(&'a Receiver<T>);

struct Inner<T> {
    recv:    Event,
    send:    Event,
    limit:   usize,
    entries: UnsafeCell<VecDeque<T>>,
}

impl<T> Inner<T> {
    #[inline]
    fn push(&self, value: T) -> Result<(), SendError<T>> {
        self.push_inner(true, value)
    }
    #[inline]
    fn pull(&self, r: &Receiver<T>) -> Result<T, RecvError> {
        loop {
            if self.limit != usize::MAX {
                // If we're a "rendezvous channel" or "limited channel" then we
                // let the the Sender know we're ready and can accept.
                self.send.set_ignore()
            }
            if let Some(x) = unsafe { &mut *self.entries.get() }.pop_front() {
                return Ok(x);
            } else if Arc::weak_count(&r.0) == 0 {
                return Err(RecvError);
            }
            self.recv.wait() // Wait for another Sender.
        }
    }
    #[inline]
    fn push_try(&self, value: T) -> Result<(), TrySendError<T>> {
        self.push_inner(false, value).map_err(|e| e.into())
    }
    #[inline]
    fn pull_try(&self, r: &Receiver<T>) -> Result<T, TryRecvError> {
        if let Some(x) = unsafe { &mut *self.entries.get() }.pop_front() {
            if self.limit != usize::MAX {
                // If we're a "rendezvous channel" or "limited channel" then we
                // let the the Sender know we pulled out an item so they can send
                // another.
                self.send.set_ignore()
            }
            return Ok(x);
        }
        if Arc::weak_count(&r.0) == 0 {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }
    fn push_inner(&self, wait: bool, value: T) -> Result<(), SendError<T>> {
        if self.limit == usize::MAX {
            unsafe { &mut *self.entries.get() }.push_back(value);
            self.recv.set_ignore(); // Tell Receiver there's mail!
            return Ok(());
        }
        let v = unsafe { &mut *self.entries.get() };
        if self.limit == 0 {
            // We're a "rendezvous channel".
            // Check if it's empty or someone else is waiting. (Only if we don't)
            // want to wait.
            if !wait && (!v.is_empty() || !self.send.is_set()) {
                return Err(SendError(value));
            }
            self.send.wait(); // Wait for Receiver to be ready.
            v.push_back(value);
            self.recv.set_ignore(); // Tell Receiver we're ready.
                                    //self.send.reset_ignore();
            return Ok(());
        }
        if v.len() >= self.limit {
            if !wait {
                return Err(SendError(value));
            }
            //self.send.reset_ignore();
            self.send.wait(); // Wait for Receiver to be ready, then try again.
            return self.push_inner(true, value);
        }
        v.push_back(value);
        self.recv.set_ignore(); // Tell Receiver there's mail!
        Ok(())
    }
    fn pull_timeout(&self, r: &Receiver<T>, d: Duration) -> Result<T, RecvTimeoutError> {
        if let Some(x) = unsafe { &mut *self.entries.get() }.pop_front() {
            if self.limit != usize::MAX {
                // If we're a "rendezvous channel" or "limited channel" then we
                // let the the Sender know we pulled out an item so they can send
                // another.
                self.send.set_ignore()
            }
            return Ok(x);
        }
        if Arc::weak_count(&r.0) == 0 {
            return Err(RecvTimeoutError::Disconnected);
        }
        if self.limit != usize::MAX {
            // If we're a "rendezvous channel" or "limited channel" then we
            // let the the Sender know we're ready and can accept.
            self.send.set_ignore()
        }
        // Wait for a Sender.
        ignore_error!(self.recv.wait_for(d));
        self.pull_try(r).map_err(|e| e.into())
    }
}
impl<T> Sender<T> {
    #[inline]
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        match self.0.upgrade() {
            Some(i) => i.push(value),
            None => Err(SendError(value)),
        }
    }
}
impl<T> Receiver<T> {
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self)
    }
    #[inline]
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter(self)
    }
    #[inline]
    pub fn recv(&self) -> Result<T, RecvError> {
        self.0.pull(self)
    }
    #[inline]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0.pull_try(self)
    }
    #[inline]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        self.0.pull_timeout(self, timeout)
    }
}
impl<T> SyncSender<T> {
    #[inline]
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        match self.0.upgrade() {
            Some(i) => i.push(value),
            None => Err(SendError(value)),
        }
    }
    #[inline]
    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        match self.0.upgrade() {
            Some(i) => i.push_try(value),
            None => Err(TrySendError::Disconnected(value)),
        }
    }
}

impl<T> Drop for Sender<T> {
    #[inline]
    fn drop(&mut self) {
        let e = self.0.upgrade().map(|i| i.recv.as_handle());
        let v = mem::take(&mut self.0);
        drop(v); // Update Ref count.
                 // Signal to any receivers that we've dropped.
        if let Some(h) = e {
            ignore_error!(winapi::SetEvent(h));
        }
    }
}
impl<T> Clone for Sender<T> {
    #[inline]
    fn clone(&self) -> Sender<T> {
        Sender(self.0.clone())
    }
}

impl<T> Drop for SyncSender<T> {
    #[inline]
    fn drop(&mut self) {
        let e = self.0.upgrade().map(|i| i.recv.as_handle());
        let v = mem::take(&mut self.0);
        drop(v); // Update Ref count.
                 // Signal to any receivers that we've dropped.
        if let Some(h) = e {
            ignore_error!(winapi::SetEvent(h));
        }
    }
}
impl<T> Clone for SyncSender<T> {
    #[inline]
    fn clone(&self) -> SyncSender<T> {
        SyncSender(self.0.clone())
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}
impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}

impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }
}
impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl Error for TryRecvError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for TryRecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for TryRecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TryRecvError::Empty => f.write_str(&ErrorKind::NotFound.to_string()),
            TryRecvError::Disconnected => f.write_str(&ErrorKind::BrokenPipe.to_string()),
        }
    }
}
impl From<RecvError> for TryRecvError {
    #[inline]
    fn from(_v: RecvError) -> TryRecvError {
        TryRecvError::Disconnected
    }
}

impl Error for RecvTimeoutError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for RecvTimeoutError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for RecvTimeoutError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RecvTimeoutError::Timeout => f.write_str(&ErrorKind::TimedOut.to_string()),
            RecvTimeoutError::Disconnected => f.write_str(&ErrorKind::BrokenPipe.to_string()),
        }
    }
}
impl From<RecvError> for RecvTimeoutError {
    #[inline]
    fn from(_v: RecvError) -> RecvTimeoutError {
        RecvTimeoutError::Disconnected
    }
}
impl From<TryRecvError> for RecvTimeoutError {
    #[inline]
    fn from(v: TryRecvError) -> RecvTimeoutError {
        match v {
            TryRecvError::Empty => RecvTimeoutError::Timeout,
            TryRecvError::Disconnected => RecvTimeoutError::Disconnected,
        }
    }
}

impl Eq for RecvError {}
impl Copy for RecvError {}
impl Clone for RecvError {
    #[inline]
    fn clone(&self) -> RecvError {
        RecvError {}
    }
}
impl Error for RecvError {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Debug for RecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl Display for RecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&ErrorKind::BrokenPipe.to_string())
    }
}
impl PartialEq for RecvError {
    #[inline]
    fn eq(&self, _other: &RecvError) -> bool {
        true
    }
}

impl<T> Error for TrySendError<T> {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T> Debug for TrySendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl<T: Eq> Eq for TrySendError<T> {}
impl<T> Display for TrySendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&ErrorKind::ResourceBusy.to_string())
    }
}
impl<T: Copy> Copy for TrySendError<T> {}
impl<T: Clone> Clone for TrySendError<T> {
    #[inline]
    fn clone(&self) -> TrySendError<T> {
        match self {
            TrySendError::Full(v) => TrySendError::Full(v.clone()),
            TrySendError::Disconnected(v) => TrySendError::Disconnected(v.clone()),
        }
    }
}
impl<T> From<SendError<T>> for TrySendError<T> {
    #[inline]
    fn from(v: SendError<T>) -> TrySendError<T> {
        TrySendError::Full(v.0)
    }
}
impl<T: PartialEq> PartialEq<TrySendError<T>> for TrySendError<T> {
    #[inline]
    fn eq(&self, other: &TrySendError<T>) -> bool {
        match (self, other) {
            (TrySendError::Full(v), TrySendError::Full(x)) => v == x,
            (TrySendError::Disconnected(v), TrySendError::Disconnected(x)) => v == x,
            _ => false,
        }
    }
}

impl<T> Error for SendError<T> {
    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T> Debug for SendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
impl<T> Display for SendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&ErrorKind::BrokenPipe.to_string())
    }
}
impl<T: Eq> Eq for SendError<T> {}
impl<T: Copy> Copy for SendError<T> {}
impl<T: Clone> Clone for SendError<T> {
    #[inline]
    fn clone(&self) -> SendError<T> {
        SendError(self.0.clone())
    }
}
impl<T: PartialEq> PartialEq<SendError<T>> for SendError<T> {
    #[inline]
    fn eq(&self, other: &SendError<T>) -> bool {
        self.0 == other.0
    }
}

unsafe impl<T> Send for Inner<T> {}
unsafe impl<T> Sync for Inner<T> {}

unsafe impl<T> Send for Sender<T> {}
unsafe impl<T> Send for Receiver<T> {}

unsafe impl<T> Send for SyncSender<T> {}
unsafe impl<T> Sync for SyncSender<T> {}

#[inline]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let r = Receiver(Arc::new(Inner {
        recv:    Event::new(),
        send:    Event::new(),
        limit:   usize::MAX,
        entries: UnsafeCell::new(VecDeque::new()),
    }));
    (Sender(Arc::downgrade(&r.0)), r)
}
#[inline]
pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
    let r = Receiver(Arc::new(Inner {
        recv:    Event::new(),
        send:    Event::new(),
        limit:   bound,
        entries: UnsafeCell::new(VecDeque::new()),
    }));
    (SyncSender(Arc::downgrade(&r.0)), r)
}
