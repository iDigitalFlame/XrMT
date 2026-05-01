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

//! Multi-producer, single-consumer FIFO queue communication primitives.
//!
//! This module provides message-based communication over channels, concretely
//! defined among three types:
//!
//! * [`Sender`]
//! * [`SyncSender`]
//! * [`Receiver`]
//!
//! A [`Sender`] or [`SyncSender`] is used to send data to a [`Receiver`]. Both
//! senders are clone-able (multi-producer) such that many threads can send
//! simultaneously to one receiver (single-consumer).
//!
//! These channels come in two flavors:
//!
//! 1. An asynchronous, infinitely buffered channel. The [`channel`] function
//!    will return a `(Sender, Receiver)` tuple where all sends will be
//!    **asynchronous** (they never block). The channel conceptually has an
//!    infinite buffer.
//!
//! 2. A synchronous, bounded channel. The [`sync_channel`] function will return
//!    a `(SyncSender, Receiver)` tuple where the storage for pending messages
//!    is a pre-allocated buffer of a fixed size. All sends will be
//!    **synchronous** by blocking until there is buffer space available. Note
//!    that a bound of 0 is allowed, causing the channel to become a
//!    "rendezvous" channel where each sender atomically hands off a message to
//!    a receiver.
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
//! use xrmt_stx::thread;
//! use xrmt_stx::sync::mpsc::channel;
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
//! use xrmt_stx::thread;
//! use xrmt_stx::sync::mpsc::channel;
//!
//! // Create a shared channel that can be sent along from many threads
//! // where tx is the sending half (tx for transmission), and rx is the receiving
//! // half (rx for receiving).
//! let (tx, rx) = channel();
//! for i in 0..10 {
//!     let tx = tx.clone();
//!     thread::spawn(move || {
//!         tx.send(i).unwrap();
//!     });
//! }
//!
//! for _ in 0..10 {
//!     let j = rx.recv().unwrap();
//!     assert!(0 <= j && j < 10);
//! }
//! ```
//!
//! Propagating panics:
//!
//! ```
//! use xrmt_stx::sync::mpsc::channel;
//!
//! // The call to recv() will return an error because the channel has already
//! // hung up (or been deallocated)
//! let (tx, rx) = channel::<i32>();
//! drop(tx);
//! assert!(rx.recv().is_err());
//! ```
//!
//! Synchronous channels:
//!
//! ```
//! use xrmt_stx::thread;
//! use xrmt_stx::sync::mpsc::sync_channel;
//!
//! let (tx, rx) = sync_channel::<i32>(0);
//! thread::spawn(move || {
//!     // This will wait for the parent thread to start receiving
//!     tx.send(53).unwrap();
//! });
//! rx.recv().unwrap();
//! ```
//!
//! Unbounded receive loop:
//!
//! ```
//! use xrmt_stx::sync::mpsc::sync_channel;
//! use xrmt_stx::thread;
//!
//! let (tx, rx) = sync_channel(3);
//!
//! for _ in 0..3 {
//!     // It would be the same without thread and clone here
//!     // since there will still be one `tx` left.
//!     let tx = tx.clone();
//!     // cloned tx dropped within thread
//!     thread::spawn(move || tx.send("ok").unwrap());
//! }
//!
//! // Drop the last sender to stop `rx` waiting for message.
//! // The program will not complete if we comment this out.
//! // **All** `tx` needs to be dropped for `rx` to have `Err`.
//! drop(tx);
//!
//! // Unbounded receiver waiting for all senders to complete.
//! while let Ok(msg) = rx.recv() {
//!     println!("{msg}");
//! }
//!
//! println!("completed");
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
use core::marker::{Copy, PhantomData, Send, Sync};
use core::option::Option::{self, None, Some};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::result::Result::{self, Err, Ok};
use core::time::Duration;

use crate::io::FmtResult;
use crate::sync::extra::{Carrier, FullError, Ref, TimeoutError, Weak, ZERO};
use crate::sync::mpmc::SendTimeoutError;
use crate::time::Instant;

/// This enumeration is the list of the possible reasons that [`try_recv`] could
/// not return data when called. This can occur with both a [`channel`] and
/// a [`sync_channel`].
///
/// [`try_recv`]: Receiver::try_recv
pub enum TryRecvError {
    /// This **channel** is currently empty, but the **Sender**(s) have not yet
    /// disconnected, so data may yet become available.
    Empty,
    /// The **channel**'s sending half has become disconnected, and there will
    /// never be any more data received on it.
    Disconnected,
}
/// This enumeration is the list of the possible error outcomes for the
/// [`try_send`] method.
///
/// [`try_send`]: SyncSender::try_send
pub enum TrySendError<T> {
    /// The data could not be sent on the [`sync_channel`] because it would
    /// require that the callee block to send the data.
    ///
    /// If this is a buffered channel, then the buffer is full at this time. If
    /// this is not a buffered channel, then there is no [`Receiver`] available
    /// to acquire the data.
    Full(T),
    /// This [`sync_channel`]'s receiving half has disconnected, so the data
    /// could not be sent. The data is returned back to the callee in this
    /// case.
    Disconnected(T),
}
/// This enumeration is the list of possible errors that made [`recv_timeout`]
/// unable to return data when called. This can occur with both a [`channel`]
/// and a [`sync_channel`].
///
/// [`recv_timeout`]: Receiver::recv_timeout
pub enum RecvTimeoutError {
    /// This **channel** is currently empty, but the **Sender**(s) have not yet
    /// disconnected, so data may yet become available.
    Timeout,
    /// The **channel**'s sending half has become disconnected, and there will
    /// never be any more data received on it.
    Disconnected,
}

/// The receiving half of Rust's [`channel`] (or [`sync_channel`]) type.
/// This half can only be owned by one thread.
///
/// Messages sent to the channel can be retrieved using [`recv`].
///
/// [`recv`]: Receiver::recv
///
/// # Examples
///
/// ```rust
/// use xrmt_stx::sync::mpsc::channel;
/// use xrmt_stx::thread;
/// use xrmt_stx::time::Duration;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send("Hello world!").unwrap();
///     thread::sleep(Duration::from_secs(2)); // block for two seconds
///     send.send("Delayed for 2 seconds").unwrap();
/// });
///
/// println!("{}", recv.recv().unwrap()); // Received immediately
/// println!("Waiting...");
/// println!("{}", recv.recv().unwrap()); // Received after 2 seconds
/// ```
pub struct Receiver<T> {
    v:  Ref<Carrier<T>>,
    _p: PhantomData<*mut ()>,
}
/// An error returned from the [`recv`] function on a [`Receiver`].
///
/// The [`recv`] operation can only fail if the sending half of a
/// [`channel`] (or [`sync_channel`]) is disconnected, implying that no further
/// messages will ever be received.
///
/// [`recv`]: Receiver::recv
pub struct RecvError(());
/// An error returned from the [`Sender::send`] or [`SyncSender::send`]
/// function on **channel**s.
///
/// A **send** operation can only fail if the receiving end of a channel is
/// disconnected, implying that the data could never be received. The error
/// contains the data being sent as a payload so it can be recovered.
pub struct SendError<T>(pub T);
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
/// use xrmt_stx::sync::mpsc::channel;
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
/// The sending-half of Rust's asynchronous [`channel`] type.
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
/// use xrmt_stx::sync::mpsc::channel;
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

/// The sending-half of Rust's synchronous [`sync_channel`] type.
///
/// Messages can be sent through this channel with [`send`] or [`try_send`].
///
/// [`send`] will block if there is no space in the internal buffer.
///
/// [`send`]: SyncSender::send
/// [`try_send`]: SyncSender::try_send
///
/// # Examples
///
/// ```rust
/// use xrmt_stx::sync::mpsc::sync_channel;
/// use xrmt_stx::thread;
///
/// // Create a sync_channel with buffer size 2
/// let (sync_sender, receiver) = sync_channel(2);
/// let sync_sender2 = sync_sender.clone();
///
/// // First thread owns sync_sender
/// thread::spawn(move || {
///     sync_sender.send(1).unwrap();
///     sync_sender.send(2).unwrap();
/// });
///
/// // Second thread owns sync_sender2
/// thread::spawn(move || {
///     sync_sender2.send(3).unwrap();
///     // thread will now block since the buffer is full
///     println!("Thread unblocked!");
/// });
///
/// let mut msg;
///
/// msg = receiver.recv().unwrap();
/// println!("message {msg} received");
///
/// // "Thread unblocked!" will be printed now
///
/// msg = receiver.recv().unwrap();
/// println!("message {msg} received");
///
/// msg = receiver.recv().unwrap();
///
/// println!("message {msg} received");
/// ```
pub struct SyncSender<T>(Weak<Carrier<T>>);
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
/// use xrmt_stx::sync::mpsc::channel;
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
/// use xrmt_stx::sync::mpsc::channel;
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
    /// Attempts to send a value on this channel, returning it back if it could
    /// not be sent.
    ///
    /// A successful send occurs when it is determined that the other end of
    /// the channel has not hung up already. An unsuccessful send would be one
    /// where the corresponding receiver has already been deallocated. Note
    /// that a return value of [`Err`] means that the data will never be
    /// received, but a return value of [`Ok`] does *not* mean that the data
    /// will be received. It is possible for the corresponding receiver to
    /// hang up immediately after this function returns [`Ok`].
    ///
    /// This method will never block the current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::mpsc::channel;
    ///
    /// let (tx, rx) = channel();
    ///
    /// // This send is always successful
    /// tx.send(1).unwrap();
    ///
    /// // This send will fail because the receiver is gone
    /// drop(rx);
    /// assert_eq!(tx.send(1).unwrap_err().0, 1);
    /// ```
    #[inline]
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, None)?),
            None => Err(SendError(v)),
        }
    }
}
impl<T> Receiver<T> {
    /// Returns an iterator that will block waiting for messages, but never
    /// [`panic!`]. It will return [`None`] when the channel has hung up.
    ///
    /// [`panic!`]: core::panic!
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xrmt_stx::sync::mpsc::channel;
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
    /// use xrmt_stx::sync::mpsc::channel;
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
    /// [`Sender`] (or [`SyncSender`]), this receiver will wake up and
    /// return that message.
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
    /// use xrmt_stx::sync::mpsc;
    /// use xrmt_stx::thread;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// use xrmt_stx::sync::mpsc;
    /// use xrmt_stx::thread;
    /// use xrmt_stx::sync::mpsc::RecvError;
    ///
    /// let (send, recv) = mpsc::channel();
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
        Ok(Ref::as_mut(&self.v).recv(None)?)
    }
    /// Attempts to return a pending value on this receiver without blocking.
    ///
    /// This method will never block the caller in order to wait for data to
    /// become available. Instead, this will always return immediately with a
    /// possible option of pending data on the channel.
    ///
    /// This is useful for a flavor of "optimistic check" before deciding to
    /// block on a receiver.
    ///
    /// Compared with [`recv`], this function has two failure cases instead of
    /// one (one for disconnection, one for an empty buffer).
    ///
    /// [`recv`]: Receiver::recv
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xrmt_stx::sync::mpsc::{Receiver, channel};
    ///
    /// let (_, receiver): (_, Receiver<i32>) = channel();
    ///
    /// assert!(receiver.try_recv().is_err());
    /// ```
    #[inline]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        Ok(Ref::as_mut(&self.v).recv(ZERO)?)
    }
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if it waits more than `timeout`.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent (at least one
    /// sender still exists). Once a message is sent to the corresponding
    /// [`Sender`] (or [`SyncSender`]), this receiver will wake up and
    /// return that message.
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
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::Duration;
    /// use xrmt_stx::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::Duration;
    /// use xrmt_stx::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Err(mpsc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[inline]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        Ok(Ref::as_mut(&self.v).recv(Some(timeout))?)
    }
    /// Attempts to wait for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if `deadline` is reached.
    ///
    /// This function will always block the current thread if there is no data
    /// available and it's possible for more data to be sent. Once a message is
    /// sent to the corresponding [`Sender`] (or [`SyncSender`]), then this
    /// receiver will wake up and return that message.
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
    /// #![feature(deadline_api)]
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::{Duration, Instant};
    /// use xrmt_stx::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// #![feature(deadline_api)]
    /// use xrmt_stx::thread;
    /// use xrmt_stx::time::{Duration, Instant};
    /// use xrmt_stx::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Err(mpsc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[inline]
    pub fn recv_deadline(&self, deadline: Instant) -> Result<T, RecvTimeoutError> {
        Ok(Ref::as_mut(&self.v).recv(Some(deadline.duration_since(Instant::now())))?)
    }
}
impl<T> SyncSender<T> {
    /// Sends a value on this synchronous channel.
    ///
    /// This function will *block* until space in the internal buffer becomes
    /// available or a receiver is available to hand off the message to.
    ///
    /// Note that a successful send does *not* guarantee that the receiver will
    /// ever see the data if there is a buffer on this channel. Items may be
    /// enqueued in the internal buffer for the receiver to receive at a later
    /// time. If the buffer size is 0, however, the channel becomes a rendezvous
    /// channel and it guarantees that the receiver has indeed received
    /// the data if this function returns success.
    ///
    /// This function will never panic, but it may return [`Err`] if the
    /// [`Receiver`] has disconnected and is no longer able to receive
    /// information.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xrmt_stx::sync::mpsc::sync_channel;
    /// use xrmt_stx::thread;
    ///
    /// // Create a rendezvous sync_channel with buffer size 0
    /// let (sync_sender, receiver) = sync_channel(0);
    ///
    /// thread::spawn(move || {
    ///    println!("sending message...");
    ///    sync_sender.send(1).unwrap();
    ///    // Thread is now blocked until the message is received
    ///
    ///    println!("...message received!");
    /// });
    ///
    /// let msg = receiver.recv().unwrap();
    /// assert_eq!(1, msg);
    /// ```
    #[inline]
    pub fn send(&self, v: T) -> Result<(), SendError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, None)?),
            None => Err(SendError(v)),
        }
    }
    /// Attempts to send a value on this channel without blocking.
    ///
    /// This method differs from [`send`] by returning immediately if the
    /// channel's buffer is full or no receiver is waiting to acquire some
    /// data. Compared with [`send`], this function has two failure cases
    /// instead of one (one for disconnection, one for a full buffer).
    ///
    /// See [`send`] for notes about guarantees of whether the
    /// receiver has received the data or not if this function is successful.
    ///
    /// [`send`]: SyncSender::send
    ///
    /// # Examples
    ///
    /// ```rust
    /// use xrmt_stx::sync::mpsc::sync_channel;
    /// use xrmt_stx::thread;
    ///
    /// // Create a sync_channel with buffer size 1
    /// let (sync_sender, receiver) = sync_channel(1);
    /// let sync_sender2 = sync_sender.clone();
    ///
    /// // First thread owns sync_sender
    /// thread::spawn(move || {
    ///     sync_sender.send(1).unwrap();
    ///     sync_sender.send(2).unwrap();
    ///     // Thread blocked
    /// });
    ///
    /// // Second thread owns sync_sender2
    /// thread::spawn(move || {
    ///     // This will return an error and send
    ///     // no message if the buffer is full
    ///     let _ = sync_sender2.try_send(3);
    /// });
    ///
    /// let mut msg;
    /// msg = receiver.recv().unwrap();
    /// println!("message {msg} received");
    ///
    /// msg = receiver.recv().unwrap();
    /// println!("message {msg} received");
    ///
    /// // Third message may have never been sent
    /// match receiver.try_recv() {
    ///     Ok(msg) => println!("message {msg} received"),
    ///     Err(_) => println!("the third message was never sent"),
    /// }
    /// ```
    #[inline]
    pub fn try_send(&self, v: T) -> Result<(), TrySendError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send_try(v)?),
            None => Err(TrySendError::Disconnected(v)),
        }
    }
    /// Attempts to send for a value on this receiver, returning an error if the
    /// corresponding channel has hung up, or if it waits more than `timeout`.
    #[inline]
    pub fn send_timeout(&self, v: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match self.0.upgrade() {
            Some(mut c) => Ok(c.send(v, Some(timeout))?),
            None => Err(SendTimeoutError::Disconnected(v)),
        }
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

impl<T> Clone for SyncSender<T> {
    #[inline]
    fn clone(&self) -> SyncSender<T> {
        SyncSender(self.0.clone())
    }
}
impl<T> UnwindSafe for SyncSender<T> {}
impl<T> RefUnwindSafe for SyncSender<T> {}

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

impl<T> Debug for SendError<T> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("SendError(")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T> Error for SendError<T> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T: Eq> Eq for SendError<T> {}
impl<T> Display for SendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
impl<T: Copy> Copy for SendError<T> {}
impl<T: Clone> Clone for SendError<T> {
    #[inline]
    fn clone(&self) -> SendError<T> {
        SendError(self.0.clone())
    }
}
impl<T: PartialEq> PartialEq for SendError<T> {
    #[inline]
    fn eq(&self, other: &SendError<T>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> Debug for TrySendError<T> {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            TrySendError::Full(_) => f.write_str("Full"),
            TrySendError::Disconnected(_) => f.write_str("Disconnected"),
        }
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl<T> Error for TrySendError<T> {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl<T: Eq> Eq for TrySendError<T> {}
impl<T> Display for TrySendError<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
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
        TrySendError::Disconnected(v.0)
    }
}
impl<T: PartialEq> PartialEq for TrySendError<T> {
    #[inline]
    fn eq(&self, other: &TrySendError<T>) -> bool {
        match (self, other) {
            (TrySendError::Full(x), TrySendError::Full(y)) => x.eq(&y),
            (TrySendError::Disconnected(x), TrySendError::Disconnected(y)) => x.eq(&y),
            _ => false,
        }
    }
}

impl Eq for RecvError {}
impl Copy for RecvError {}
impl Clone for RecvError {
    #[inline]
    fn clone(&self) -> RecvError {
        RecvError(())
    }
}
impl Debug for RecvError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("RecvError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Error for RecvError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for RecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
impl PartialEq for RecvError {
    #[inline]
    fn eq(&self, _other: &RecvError) -> bool {
        true
    }
}

impl Eq for TryRecvError {}
impl Copy for TryRecvError {}
impl Clone for TryRecvError {
    #[inline]
    fn clone(&self) -> TryRecvError {
        match self {
            TryRecvError::Empty => TryRecvError::Empty,
            TryRecvError::Disconnected => TryRecvError::Disconnected,
        }
    }
}
impl Debug for TryRecvError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            TryRecvError::Empty => f.write_str("Empty"),
            TryRecvError::Disconnected => f.write_str("Disconnected"),
        }
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Error for TryRecvError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for TryRecvError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
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
impl From<RecvError> for TryRecvError {
    #[inline]
    fn from(_v: RecvError) -> TryRecvError {
        TryRecvError::Disconnected
    }
}

impl Eq for RecvTimeoutError {}
impl Copy for RecvTimeoutError {}
impl Clone for RecvTimeoutError {
    #[inline]
    fn clone(&self) -> RecvTimeoutError {
        match self {
            RecvTimeoutError::Timeout => RecvTimeoutError::Timeout,
            RecvTimeoutError::Disconnected => RecvTimeoutError::Disconnected,
        }
    }
}
impl Debug for RecvTimeoutError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            RecvTimeoutError::Timeout => f.write_str("Timeout"),
            RecvTimeoutError::Disconnected => f.write_str("Disconnected"),
        }
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Error for RecvTimeoutError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for RecvTimeoutError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
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
impl From<RecvError> for RecvTimeoutError {
    #[inline]
    fn from(_v: RecvError) -> RecvTimeoutError {
        RecvTimeoutError::Disconnected
    }
}

impl From<TimeoutError> for RecvError {
    #[inline]
    fn from(_v: TimeoutError) -> RecvError {
        RecvError(())
    }
}
impl From<TimeoutError> for TryRecvError {
    #[inline]
    fn from(_v: TimeoutError) -> TryRecvError {
        TryRecvError::Empty
    }
}
impl<T> From<FullError<T>> for SendError<T> {
    #[inline]
    fn from(v: FullError<T>) -> SendError<T> {
        SendError(v.0)
    }
}
impl From<TimeoutError> for RecvTimeoutError {
    #[inline]
    fn from(_v: TimeoutError) -> RecvTimeoutError {
        RecvTimeoutError::Timeout
    }
}
impl<T> From<FullError<T>> for TrySendError<T> {
    #[inline]
    fn from(v: FullError<T>) -> TrySendError<T> {
        TrySendError::Full(v.0)
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

unsafe impl<T: Send> Sync for Sender<T> {}
unsafe impl<T: Send> Send for Sender<T> {}
unsafe impl<T: Send> Send for IntoIter<T> {}
unsafe impl<T: Send> Send for Receiver<T> {}

unsafe impl<T: Send> Send for SyncSender<T> {}
unsafe impl<T: Send> Sync for SyncSender<T> {}

/// Creates a new asynchronous channel, returning the sender/receiver halves.
///
/// All data sent on the [`Sender`] will become available on the [`Receiver`] in
/// the same order as it was sent, and no [`send`] will block the calling thread
/// (this channel has an "infinite buffer", unlike [`sync_channel`], which will
/// block after its buffer limit is reached). [`recv`] will block until a
/// message is available while there is at least one [`Sender`] alive (including
/// clones).
///
/// The [`Sender`] can be cloned to [`send`] to the same channel multiple times,
/// but only one [`Receiver`] is supported.
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
/// use xrmt_stx::sync::mpsc::channel;
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
    let r = Receiver {
        v:  Ref::new(Carrier::new_list()),
        _p: PhantomData,
    };
    (Sender(Ref::weak(&r.v)), r)
}
/// Creates a new synchronous, bounded channel.
///
/// All data sent on the [`SyncSender`] will become available on the
/// [`Receiver`] in the same order as it was sent. Like asynchronous
/// [`channel`]s, the [`Receiver`] will block until a message becomes available.
/// `sync_channel` differs greatly in the semantics of the sender, however.
///
/// This channel has an internal buffer on which messages will be queued.
/// `bound` specifies the buffer size. When the internal buffer becomes full,
/// future sends will *block* waiting for the buffer to open up. Note that a
/// buffer size of 0 is valid, in which case this becomes "rendezvous channel"
/// where each [`send`] will not return until a [`recv`] is paired with it.
///
/// The [`SyncSender`] can be cloned to [`send`] to the same channel multiple
/// times, but only one [`Receiver`] is supported.
///
/// Like asynchronous channels, if the [`Receiver`] is disconnected while trying
/// to [`send`] with the [`SyncSender`], the [`send`] method will return a
/// [`SendError`]. Similarly, If the [`SyncSender`] is disconnected while trying
/// to [`recv`], the [`recv`] method will return a [`RecvError`].
///
/// [`send`]: SyncSender::send
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
pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
    let r = Receiver {
        v:  Ref::new(if bound == 0 {
            Carrier::new_single()
        } else {
            Carrier::new_array(bound)
        }),
        _p: PhantomData,
    };
    (SyncSender(Ref::weak(&r.v)), r)
}
