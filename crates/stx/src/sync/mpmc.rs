// Copyright (C) 2023 - 2025 iDigitalFlame
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

//! Multi-producer, multi-consumer FIFO queue communication primitives.
//!
//! This module provides message-based communication over channels, concretely
//! defined by two types:
//!
//! * [`Sender`]
//! * [`Receiver`]
//!
//! [`Sender`]s are used to send data to a set of [`Receiver`]s. Both
//! sender and receiver are cloneable (multi-producer) such that many threads
//! can send simultaneously to receivers (multi-consumer).
//!
//! These channels come in two flavors:
//!
//! 1. An asynchronous, infinitely buffered channel. The [`channel`] function
//!    will return a `(Sender, Receiver)` tuple where all sends will be
//!    **asynchronous** (they never block). The channel conceptually has an
//!    infinite buffer.
//!
//! 2. A synchronous, bounded channel. The [`sync_channel`] function will return
//!    a `(Sender, Receiver)` tuple where the storage for pending messages is a
//!    pre-allocated buffer of a fixed size. All sends will be **synchronous**
//!    by blocking until there is buffer space available. Note that a bound of 0
//!    is allowed, causing the channel to become a "rendezvous" channel where
//!    each sender atomically hands off a message to a receiver.
//!
//! [`send`]: Sender::send
//!
//! ## Disconnection
//!
//! The send and receive operations on channels will all return a [`Result`]
//! indicating whether the operation succeeded or not. An unsuccessful operation
//! is normally indicative of the other half of a channel having "hung up" by
//! being dropped in its corresponding thread.
//!
//! Once half of a channel has been deallocated, most operations can no longer
//! continue to make progress, so [`Err`] will be returned. Many applications
//! will continue to [`unwrap`] the results returned from this module,
//! instigating a propagation of failure among threads if one unexpectedly dies.
//!
//! [`unwrap`]: Result::unwrap
//!
//! # Examples
//!
//! Simple usage:
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use xrmt_stx::thread;
//! use xrmt_stx::sync::mpmc::channel;
//!
//! // Create a simple streaming channel
//! let (tx, rx) = channel();
//! thread::spawn(move || {
//!     tx.send(10).unwrap();
//! });
//! assert_eq!(rx.recv().unwrap(), 10);
//! ```
//!
//! Shared usage:
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use xrmt_stx::thread;
//! use xrmt_stx::sync::mpmc::channel;
//!
//! thread::scope(|s| {
//!     // Create a shared channel that can be sent along from many threads
//!     // where tx is the sending half (tx for transmission), and rx is the receiving
//!     // half (rx for receiving).
//!     let (tx, rx) = channel();
//!     for i in 0..10 {
//!         let tx = tx.clone();
//!         s.spawn(move || {
//!             tx.send(i).unwrap();
//!         });
//!     }
//!
//!     for _ in 0..5 {
//!         let rx1 = rx.clone();
//!         let rx2 = rx.clone();
//!         s.spawn(move || {
//!             let j = rx1.recv().unwrap();
//!             assert!(0 <= j && j < 10);
//!         });
//!         s.spawn(move || {
//!             let j = rx2.recv().unwrap();
//!             assert!(0 <= j && j < 10);
//!         });
//!     }
//! })
//! ```
//!
//! Propagating panics:
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use xrmt_stx::sync::mpmc::channel;
//!
//! // The call to recv() will return an error because the channel has already
//! // hung up (or been deallocated)
//! let (tx, rx) = channel::<i32>();
//! drop(tx);
//! assert!(rx.recv().is_err());
//! ```

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate core;

use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::From;
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::iter::{IntoIterator, Iterator};
use core::marker::{Copy, Send, Sync};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{self, Err, Ok};
use core::time::Duration;

use crate::io::FmtResult;
use crate::sync::extra::{Carrier, FullError, Ref, Weak, ZERO};
use crate::time::Instant;

#[cfg_attr(rustfmt, rustfmt_skip)]
pub use crate::sync::mpsc::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};

/// An error returned from the [`send_timeout`] method.
///
/// The error contains the message being sent so it can be recovered.
///
/// [`send_timeout`]: Sender::send_timeout
pub enum SendTimeoutError<T> {
    /// The message could not be sent because the channel is full and the
    /// operation timed out.
    ///
    /// If this is a zero-capacity channel, then the error indicates that there
    /// was no receiver available to receive the message and the operation
    /// timed out.
    Timeout(T),
    /// The message could not be sent because the channel is disconnected.
    Disconnected(T),
}

/// An owning iterator over messages on a [`Receiver`],
/// created by [`into_iter`].
///
/// This iterator will block whenever [`next`]
/// is called, waiting for a new message, and [`None`] will be
/// returned if the corresponding channel has hung up.
///
/// [`into_iter`]: Receiver::into_iter
/// [`next`]: Iterator::next
///
/// # Examples
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send(1u8).unwrap();
///     send.send(2u8).unwrap();
///     send.send(3u8).unwrap();
/// });
///
/// for x in recv.into_iter() {
///     println!("Got: {x}");
/// }
/// ```
pub struct IntoIter<T>(Receiver<T>);
/// The sending-half of Rust's synchronous [`channel`] type.
///
/// Messages can be sent through this channel with [`send`].
///
/// Note: all senders (the original and its clones) need to be dropped for the
/// receiver to stop blocking to receive messages with [`Receiver::recv`].
///
/// [`send`]: Sender::send
///
/// # Examples
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
///
/// let (sender, receiver) = channel();
/// let sender2 = sender.clone();
///
/// // First thread owns sender
/// thread::spawn(move || {
///     sender.send(1).unwrap();
/// });
///
/// // Second thread owns sender2
/// thread::spawn(move || {
///     sender2.send(2).unwrap();
/// });
///
/// let msg = receiver.recv().unwrap();
/// let msg2 = receiver.recv().unwrap();
///
/// assert_eq!(3, msg + msg2);
/// ```
pub struct Sender<T>(Weak<Carrier<T>>);
/// The receiving half of Rust's [`channel`] (or [`sync_channel`]) type.
/// Different threads can share this [`Receiver`] by cloning it.
///
/// Messages sent to the channel can be retrieved using [`recv`].
///
/// [`recv`]: Receiver::recv
///
/// # Examples
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
/// use xrmt_stx::time::Duration;
///
/// let (send, recv) = channel();
///
/// let tx_thread = thread::spawn(move || {
///     send.send("Hello world!").unwrap();
///     thread::sleep(Duration::from_secs(2)); // block for two seconds
///     send.send("Delayed for 2 seconds").unwrap();
/// });
///
/// let (rx1, rx2) = (recv.clone(), recv.clone());
/// let rx_thread_1 = thread::spawn(move || {
///     println!("{}", rx1.recv().unwrap()); // Received immediately
/// });
/// let rx_thread_2 = thread::spawn(move || {
///     println!("{}", rx2.recv().unwrap()); // Received after 2 seconds
/// });
///
/// tx_thread.join().unwrap();
/// rx_thread_1.join().unwrap();
/// rx_thread_2.join().unwrap();
/// ```
pub struct Receiver<T>(Ref<Carrier<T>>);
/// An iterator over messages on a [`Receiver`], created by [`iter`].
///
/// This iterator will block whenever [`next`] is called,
/// waiting for a new message, and [`None`] will be returned
/// when the corresponding channel has hung up.
///
/// [`iter`]: Receiver::iter
/// [`next`]: Iterator::next
///
/// # Examples
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send(1u8).unwrap();
///     send.send(2u8).unwrap();
///     send.send(3u8).unwrap();
/// });
///
/// for x in recv.iter() {
///     println!("Got: {x}");
/// }
/// ```
pub struct Iter<'a, T: 'a>(&'a Receiver<T>);
/// An iterator that attempts to yield all pending values for a [`Receiver`],
/// created by [`try_iter`].
///
/// [`None`] will be returned when there are no pending values remaining or
/// if the corresponding channel has hung up.
///
/// This iterator will never block the caller in order to wait for data to
/// become available. Instead, it will return [`None`].
///
/// [`try_iter`]: Receiver::try_iter
///
/// # Examples
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
/// use xrmt_stx::time::Duration;
///
/// let (sender, receiver) = channel();
///
/// // Nothing is in the buffer yet
/// assert!(receiver.try_iter().next().is_none());
/// println!("Nothing in the buffer...");
///
/// thread::spawn(move || {
///     sender.send(1).unwrap();
///     sender.send(2).unwrap();
///     sender.send(3).unwrap();
/// });
///
/// println!("Going to sleep...");
/// thread::sleep(Duration::from_secs(2)); // block for two seconds
///
/// for x in receiver.try_iter() {
///     println!("Got: {x}");
/// }
/// ```
pub struct TryIter<'a, T: 'a>(&'a Receiver<T>);

impl<T> Sender<T> {
    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, _recv) = mpmc::channel();
    /// let (tx1, tx2) = (send.clone(), send.clone());
    ///
    /// assert_eq!(tx1.len(), 0);
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(tx1.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.access(|c| c.len()).unwrap_or(0)
    }
    /// Returns `true` if the channel is full.
    ///
    /// Note: Zero-capacity channels are always full.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, _recv) = mpmc::sync_channel(1);
    ///
    /// let (tx1, tx2) = (send.clone(), send.clone());
    /// assert!(!tx1.is_full());
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(tx1.is_full());
    /// ```
    #[inline]
    pub fn is_full(&self) -> bool {
        self.0.access(|c| c.is_full()).unwrap_or(false)
    }
    /// Returns `true` if the channel is empty.
    ///
    /// Note: Zero-capacity channels are always empty.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, _recv) = mpmc::channel();
    ///
    /// let tx1 = send.clone();
    /// let tx2 = send.clone();
    ///
    /// assert!(tx1.is_empty());
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(!tx1.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.access(|c| c.is_empty()).unwrap_or(false)
    }
    /// If the channel is bounded, returns its capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, _recv) = mpmc::sync_channel(3);
    /// let (tx1, tx2) = (send.clone(), send.clone());
    ///
    /// assert_eq!(tx1.capacity(), Some(3));
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(tx1.capacity(), Some(3));
    /// ```
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        self.0.access(|c| c.capacity()).flatten()
    }
    /// Returns `true` if senders belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (tx1, _) = mpmc::channel::<i32>();
    /// let (tx2, _) = mpmc::channel::<i32>();
    ///
    /// assert!(tx1.same_channel(&tx1));
    /// assert!(!tx1.same_channel(&tx2));
    /// ```
    #[inline]
    pub fn same_channel(&self, other: &Sender<T>) -> bool {
        self.0.ptr_eq(&other.0)
    }
    /// Attempts to send a value on this channel, returning it back if it could
    /// not be sent.
    ///
    /// A successful send occurs when it is determined that the other end of
    /// the channel has not hung up already. An unsuccessful send would be one
    /// where the corresponding receiver has already been deallocated. Note
    /// that a return value of [`Err`] means that the data will never be
    /// received, but a return value of [`Ok`] does *not* mean that the data
    /// will be received. It is possible for the corresponding receiver to
    /// hang up immediately after this function returns [`Ok`]. However, if
    /// the channel is zero-capacity, it acts as a rendezvous channel and a
    /// return value of [`Ok`] means that the data has been received.
    ///
    /// If the channel is full and not disconnected, this call will block until
    /// the send operation can proceed. If the channel becomes disconnected,
    /// this call will wake up and return an error. The returned error contains
    /// the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a
    /// receive operation to appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::channel;
    ///
    /// let (tx, rx) = channel();
    ///
    /// // This send is always successful
    /// tx.send(1).unwrap();
    ///
    /// // This send will fail because the receiver is gone
    /// drop(rx);
    /// assert!(tx.send(1).is_err());
    /// ```
    #[inline]
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, None)?),
            None => Err(SendError(v)),
        }
    }
    /// Attempts to send a message into the channel without blocking.
    ///
    /// This method will either send a message into the channel immediately or
    /// return an error if the channel is full or disconnected. The returned
    /// error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will send the message
    /// only if there happens to be a receive operation on the other side of
    /// the channel at the same time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::{channel, Receiver, Sender};
    ///
    /// let (sender, _receiver): (Sender<i32>, Receiver<i32>) = channel();
    ///
    /// assert!(sender.try_send(1).is_ok());
    /// ```
    #[inline]
    pub fn try_send(&self, v: T) -> Result<(), TrySendError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send_try(v)?),
            None => Err(TrySendError::Disconnected(v)),
        }
    }
    /// Waits for a message to be sent into the channel, but only for a limited
    /// time.
    ///
    /// If the channel is full and not disconnected, this call will block until
    /// the send operation can proceed or the operation times out. If the
    /// channel becomes disconnected, this call will wake up and return an
    /// error. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a
    /// receive operation to appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::channel;
    /// use xrmt_stx::time::Duration;
    ///
    /// let (tx, rx) = channel();
    ///
    /// tx.send_timeout(1, Duration::from_millis(400)).unwrap();
    /// ```
    #[inline]
    pub fn send_timeout(&self, v: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, Some(timeout))?),
            None => Err(SendTimeoutError::Disconnected(v)),
        }
    }
    /// Waits for a message to be sent into the channel, but only until a given
    /// deadline.
    ///
    /// If the channel is full and not disconnected, this call will block until
    /// the send operation can proceed or the operation times out. If the
    /// channel becomes disconnected, this call will wake up and return an
    /// error. The returned error contains the original message.
    ///
    /// If called on a zero-capacity channel, this method will wait for a
    /// receive operation to appear on the other side of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::channel;
    /// use xrmt_stx::time::{Duration, Instant};
    ///
    /// let (tx, rx) = channel();
    ///
    /// let t = Instant::now() + Duration::from_millis(400);
    /// tx.send_deadline(1, t).unwrap();
    /// ```
    #[inline]
    pub fn send_deadline(&self, v: T, deadline: Instant) -> Result<(), SendTimeoutError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, Some(deadline.duration_since(Instant::now())))?),
            None => Err(SendTimeoutError::Disconnected(v)),
        }
    }
}
impl<T> Receiver<T> {
    /// Returns the number of messages in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// assert_eq!(recv.len(), 0);
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(recv.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Returns `true` if the channel is full.
    ///
    /// Note: Zero-capacity channels are always full.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpmc::sync_channel(1);
    ///
    /// assert!(!recv.is_full());
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(recv.is_full());
    /// ```
    #[inline]
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }
    /// Returns `true` if the channel is empty.
    ///
    /// Note: Zero-capacity channels are always empty.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// assert!(recv.is_empty());
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(!recv.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns an iterator that will block waiting for messages, but never
    /// [`panic!`]. It will return [`None`] when the channel has hung up.
    ///
    /// [`panic!`]: core::panic!
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::channel;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = channel();
    ///
    /// thread::spawn(move || {
    ///     send.send(1).unwrap();
    ///     send.send(2).unwrap();
    ///     send.send(3).unwrap();
    /// });
    ///
    /// let mut iter = recv.iter();
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self)
    }
    /// If the channel is bounded, returns its capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpmc::sync_channel(3);
    ///
    /// assert_eq!(recv.capacity(), Some(3));
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(recv.capacity(), Some(3));
    /// ```
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        self.0.capacity()
    }
    /// Returns an iterator that will attempt to yield all pending values.
    /// It will return `None` if there are no more pending values or if the
    /// channel has hung up. The iterator will never [`panic!`] or block the
    /// user by waiting for values.
    ///
    /// [`panic!`]: core::panic!
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::channel;
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::Duration;
    ///
    /// let (sender, receiver) = channel();
    ///
    /// // nothing is in the buffer yet
    /// assert!(receiver.try_iter().next().is_none());
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     sender.send(1).unwrap();
    ///     sender.send(2).unwrap();
    ///     sender.send(3).unwrap();
    /// });
    ///
    /// // nothing is in the buffer yet
    /// assert!(receiver.try_iter().next().is_none());
    ///
    /// // block for two seconds
    /// thread::sleep(Duration::from_secs(2));
    ///
    /// let mut iter = receiver.try_iter();
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter(self)
    }
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent (at least one
    /// sender still exists). Once a message is sent to the corresponding
    /// [`Sender`], this receiver will wake up and return that message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects
    /// while this call is blocking, this call will wake up and return
    /// [`Err`] to indicate that no more messages can ever be received on
    /// this channel. However, since channels are buffered, messages sent
    /// before the disconnect will still be properly received.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(Ok(1), recv.recv());
    /// ```
    ///
    /// Buffering behavior:
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    /// use xrmt_stx::thread;
    /// use xrmt_stx::sync::mpmc::RecvError;
    ///
    /// let (send, recv) = mpmc::channel();
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    ///     send.send(2).unwrap();
    ///     send.send(3).unwrap();
    ///     drop(send);
    /// });
    ///
    /// // wait for the thread to join so we ensure the sender is dropped
    /// handle.join().unwrap();
    ///
    /// assert_eq!(Ok(1), recv.recv());
    /// assert_eq!(Ok(2), recv.recv());
    /// assert_eq!(Ok(3), recv.recv());
    /// assert_eq!(Err(RecvError), recv.recv());
    /// ```
    #[inline]
    pub fn recv(&self) -> Result<T, RecvError> {
        Ok(Ref::as_mut(&self.0).recv(None)?)
    }
    /// Attempts to receive a message from the channel without blocking.
    ///
    /// This method will never block the caller in order to wait for data to
    /// become available. Instead, this will always return immediately with a
    /// possible option of pending data on the channel.
    ///
    /// If called on a zero-capacity channel, this method will receive a message
    /// only if there happens to be a send operation on the other side of
    /// the channel at the same time.
    ///
    /// This is useful for a flavor of "optimistic check" before deciding to
    /// block on a receiver.
    ///
    /// Compared with [`recv`], this function has two failure cases instead of
    /// one (one for disconnection, one for an empty buffer).
    ///
    /// [`recv`]: Self::recv
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc::{Receiver, channel};
    ///
    /// let (_, receiver): (_, Receiver<i32>) = channel();
    ///
    /// assert!(receiver.try_recv().is_err());
    /// ```
    #[inline]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        Ok(Ref::as_mut(&self.0).recv(ZERO)?)
    }
    /// Returns `true` if receivers belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (_, rx1) = mpmc::channel::<i32>();
    /// let (_, rx2) = mpmc::channel::<i32>();
    ///
    /// assert!(rx1.same_channel(&rx1));
    /// assert!(!rx1.same_channel(&rx2));
    /// ```
    #[inline]
    pub fn same_channel(&self, other: &Receiver<T>) -> bool {
        Ref::ptr_eq(&self.0, &other.0)
    }
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if it waits more than `timeout`.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent (at least one
    /// sender still exists). Once a message is sent to the corresponding
    /// [`Sender`], this receiver will wake up and return that message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects
    /// while this call is blocking, this call will wake up and return
    /// [`Err`] to indicate that no more messages can ever be received on
    /// this channel. However, since channels are buffered, messages sent
    /// before the disconnect will still be properly received.
    ///
    /// # Examples
    ///
    /// Successfully receiving value before encountering timeout:
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::Duration;
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Ok('a')
    /// );
    /// ```
    ///
    /// Receiving an error upon reaching timeout:
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::Duration;
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Err(mpmc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[inline]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        Ok(Ref::as_mut(&self.0).recv(Some(timeout))?)
    }
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if `deadline` is reached.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent. Once a message is
    /// sent to the corresponding [`Sender`], then this receiver will wake up
    /// and return that message.
    ///
    /// If the corresponding [`Sender`] has disconnected, or it disconnects
    /// while this call is blocking, this call will wake up and return
    /// [`Err`] to indicate that no more messages can ever be received on
    /// this channel. However, since channels are buffered, messages sent
    /// before the disconnect will still be properly received.
    ///
    /// # Examples
    ///
    /// Successfully receiving value before reaching deadline:
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::{Duration, Instant};
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Ok('a')
    /// );
    /// ```
    ///
    /// Receiving an error upon reaching deadline:
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::{Duration, Instant};
    /// use xrmt_stx::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Err(mpmc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[inline]
    pub fn recv_deadline(&self, deadline: Instant) -> Result<T, RecvTimeoutError> {
        Ok(Ref::as_mut(&self.0).recv(Some(deadline.duration_since(Instant::now())))?)
    }
}

impl<T> Clone for Sender<T> {
    #[inline]
    fn clone(&self) -> Sender<T> {
        Sender(self.0.clone())
    }
}
impl<T> UnwindSafe for Sender<T> {}
impl<T> RefUnwindSafe for Sender<T> {}

impl<T> Clone for Receiver<T> {
    #[inline]
    fn clone(&self) -> Receiver<T> {
        Receiver(self.0.clone())
    }
}
impl<T> UnwindSafe for Receiver<T> {}
impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }
}
impl<T> RefUnwindSafe for Receiver<T> {}
impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T> Debug for SendTimeoutError<T> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            SendTimeoutError::Timeout(_) => f.write_str("Timeout"),
            SendTimeoutError::Disconnected(_) => f.write_str("Disconnected"),
        }
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T> Error for SendTimeoutError<T> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T: Eq> Eq for SendTimeoutError<T> {}
impl<T> Display for SendTimeoutError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
impl<T: Copy> Copy for SendTimeoutError<T> {}
impl<T: Clone> Clone for SendTimeoutError<T> {
    #[inline]
    fn clone(&self) -> SendTimeoutError<T> {
        match self {
            SendTimeoutError::Timeout(v) => SendTimeoutError::Timeout(v.clone()),
            SendTimeoutError::Disconnected(v) => SendTimeoutError::Disconnected(v.clone()),
        }
    }
}
impl<T> From<SendError<T>> for SendTimeoutError<T> {
    #[inline]
    fn from(v: SendError<T>) -> SendTimeoutError<T> {
        SendTimeoutError::Disconnected(v.0)
    }
}
impl<T: PartialEq> PartialEq for SendTimeoutError<T> {
    #[inline]
    fn eq(&self, other: &SendTimeoutError<T>) -> bool {
        match (self, other) {
            (SendTimeoutError::Timeout(x), SendTimeoutError::Timeout(y)) => x.eq(&y),
            (SendTimeoutError::Disconnected(x), SendTimeoutError::Disconnected(y)) => x.eq(&y),
            _ => false,
        }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}
impl<'a, T> UnwindSafe for Iter<'a, T> {}
impl<'a, T> RefUnwindSafe for Iter<'a, T> {}

impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}
impl<'a, T> UnwindSafe for TryIter<'a, T> {}
impl<'a, T> RefUnwindSafe for TryIter<'a, T> {}

impl<T> From<FullError<T>> for SendTimeoutError<T> {
    #[inline]
    fn from(v: FullError<T>) -> SendTimeoutError<T> {
        SendTimeoutError::Timeout(v.0)
    }
}

unsafe impl<T: Send> Sync for Sender<T> {}
unsafe impl<T: Send> Send for Sender<T> {}

unsafe impl<T: Send> Sync for Receiver<T> {}
unsafe impl<T: Send> Send for Receiver<T> {}

unsafe impl<T: Send> Sync for Iter<'_, T> {}
unsafe impl<T: Send> Send for Iter<'_, T> {}

unsafe impl<T: Send> Sync for IntoIter<T> {}
unsafe impl<T: Send> Send for IntoIter<T> {}

unsafe impl<T: Send> Sync for TryIter<'_, T> {}
unsafe impl<T: Send> Send for TryIter<'_, T> {}

/// Creates a new asynchronous channel, returning the sender/receiver halves.
///
/// All data sent on the [`Sender`] will become available on the [`Receiver`] in
/// the same order as it was sent, and no [`send`] will block the calling thread
/// (this channel has an "infinite buffer", unlike [`sync_channel`], which will
/// block after its buffer limit is reached). [`recv`] will block until a
/// message is available while there is at least one [`Sender`] alive (including
/// clones).
///
/// The [`Sender`] can be cloned to [`send`] to the same channel multiple times.
/// The [`Receiver`] also can be cloned to have multi receivers.
///
/// If the [`Receiver`] is disconnected while trying to [`send`] with the
/// [`Sender`], the [`send`] method will return a [`SendError`]. Similarly, if
/// the [`Sender`] is disconnected while trying to [`recv`], the [`recv`] method
/// will return a [`RecvError`].
///
/// [`send`]: Sender::send
/// [`recv`]: Receiver::recv
///
/// # Examples
///
/// ```
/// #![feature(mpmc_channel)]
///
/// use xrmt_stx::sync::mpmc::channel;
/// use xrmt_stx::thread;
///
/// let (sender, receiver) = channel();
///
/// // Spawn off an expensive computation
/// thread::spawn(move || {
/// #   fn expensive_computation() {}
///     sender.send(expensive_computation()).unwrap();
/// });
///
/// // Do some useful work for awhile
///
/// // Let's see what that answer was
/// println!("{:?}", receiver.recv().unwrap());
/// ```
#[inline]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let r = Receiver(Ref::new(Carrier::new_list()));
    (Sender(Ref::weak(&r.0)), r)
}

/// Creates a new synchronous, bounded channel.
///
/// All data sent on the [`Sender`] will become available on the [`Receiver`]
/// in the same order as it was sent. Like asynchronous [`channel`]s, the
/// [`Receiver`] will block until a message becomes available. `sync_channel`
/// differs greatly in the semantics of the sender, however.
///
/// This channel has an internal buffer on which messages will be queued.
/// `bound` specifies the buffer size. When the internal buffer becomes full,
/// future sends will *block* waiting for the buffer to open up. Note that a
/// buffer size of 0 is valid, in which case this becomes "rendezvous channel"
/// where each [`send`] will not return until a [`recv`] is paired with it.
///
/// The [`Sender`] can be cloned to [`send`] to the same channel multiple
/// times. The [`Receiver`] also can be cloned to have multi receivers.
///
/// Like asynchronous channels, if the [`Receiver`] is disconnected while trying
/// to [`send`] with the [`Sender`], the [`send`] method will return a
/// [`SendError`]. Similarly, If the [`Sender`] is disconnected while trying
/// to [`recv`], the [`recv`] method will return a [`RecvError`].
///
/// [`send`]: Sender::send
/// [`recv`]: Receiver::recv
///
/// # Examples
///
/// ```
/// use xrmt_stx::sync::mpsc::sync_channel;
/// use xrmt_stx::thread;
///
/// let (sender, receiver) = sync_channel(1);
///
/// // this returns immediately
/// sender.send(1).unwrap();
///
/// thread::spawn(move || {
///     // this will block until the previous message has been received
///     sender.send(2).unwrap();
/// });
///
/// assert_eq!(receiver.recv().unwrap(), 1);
/// assert_eq!(receiver.recv().unwrap(), 2);
/// ```
#[inline]
pub fn sync_channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let r = Receiver(Ref::new(if cap == 0 {
        Carrier::new_single()
    } else {
        Carrier::new_array(cap)
    }));
    (Sender(Ref::weak(&r.0)), r)
}
