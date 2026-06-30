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

//! Native threads.
//!
//! ## The threading model
//!
//! An executing Rust program consists of a collection of native OS threads,
//! each with their own stack and local state. Threads can be named, and
//! provide some built-in support for low-level synchronization.
//!
//! Communication between threads can be done through
//! [channels], Rust's message-passing types, along with [other forms of thread
//! synchronization](../../std/sync/index.html) and shared-memory data
//! structures. In particular, types that are guaranteed to be
//! threadsafe are easily shared between threads using the
//! atomically-reference-counted container, [`Arc`].
//!
//! Fatal logic errors in Rust cause *thread panic*, during which
//! a thread will unwind the stack, running destructors and freeing
//! owned resources. While not meant as a 'try/catch' mechanism, panics
//! in Rust can nonetheless be caught (unless compiling with `panic=abort`) with
//! [`catch_unwind`](../../std/panic/fn.catch_unwind.html) and recovered
//! from, or alternatively be resumed with
//! [`resume_unwind`](../../std/panic/fn.resume_unwind.html). If the panic
//! is not caught the thread will exit, but the panic may optionally be
//! detected from a different thread with [`join`]. If the main thread panics
//! without the panic being caught, the application will exit with a
//! non-zero exit code.
//!
//! When the main thread of a Rust program terminates, the entire program shuts
//! down, even if other threads are still running. However, this module provides
//! convenient facilities for automatically waiting for the termination of a
//! thread (i.e., join).
//!
//! ## Spawning a thread
//!
//! A new thread can be spawned using the [`thread::spawn`][`spawn`] function:
//!
//! ```rust
//! use xrmt_stx::thread;
//!
//! thread::spawn(move || {
//!     // some work here
//! });
//! ```
//!
//! In this example, the spawned thread is "detached," which means that there is
//! no way for the program to learn when the spawned thread completes or
//! otherwise terminates.
//!
//! To learn when a thread completes, it is necessary to capture the
//! [`JoinHandle`] object that is returned by the call to [`spawn`], which
//! provides a `join` method that allows the caller to wait for the completion
//! of the spawned thread:
//!
//! ```rust
//! use xrmt_stx::thread;
//!
//! let thread_join_handle = thread::spawn(move || {
//!     // some work here
//! });
//! // some work here
//! let res = thread_join_handle.join();
//! ```
//!
//! The [`join`] method returns a [`thread::Result`] containing [`Ok`] of the
//! final value produced by the spawned thread, or [`Err`] of the value given to
//! a call to [`panic!`] if the thread panicked.
//!
//! Note that there is no parent/child relationship between a thread that spawns
//! a new thread and the thread being spawned.  In particular, the spawned
//! thread may or may not outlive the spawning thread, unless the spawning
//! thread is the main thread.
//!
//! ## Configuring threads
//!
//! A new thread can be configured before it is spawned via the [`Builder`]
//! type, which currently allows you to set the name and stack size for the
//! thread:
//!
//! ```rust
//! # #![allow(unused_must_use)]
//! use xrmt_stx::thread;
//!
//! thread::Builder::new().name("thread1".to_string()).spawn(move || {
//!     println!("Hello, world!");
//! });
//! ```
//!
//! ## The `Thread` type
//!
//! Threads are represented via the [`Thread`] type, which you can get in one of
//! two ways:
//!
//! * By spawning a new thread, e.g., using the [`thread::spawn`][`spawn`]
//!   function, and calling [`thread`][`JoinHandle::thread`] on the
//!   [`JoinHandle`].
//! * By requesting the current thread, using the [`thread::current`] function.
//!
//! The [`thread::current`] function is available even for threads not spawned
//! by the APIs of this module.
//!
//! ## Thread-local storage
//!
//! This module also provides an implementation of thread-local storage for Rust
//! programs. Thread-local storage is a method of storing data into a global
//! variable that each thread in the program will have its own copy of.
//! Threads do not share this data, so accesses do not need to be synchronized.
//!
//! A thread-local key owns the value it contains and will destroy the value
//! when the thread exits. It is created with the `thread_local!` macro and
//! can contain any value that is `'static` (no borrowed pointers). It provides
//! an accessor function, `with`, that yields a shared reference to the value
//! to the specified closure. Thread-local keys allow only shared access to
//! values, as there would be no way to guarantee uniqueness if mutable borrows
//! were allowed. Most values will want to make use of some form of **interior
//! mutability** through the [`Cell`] or [`RefCell`] types.
//!
//! ## Naming threads
//!
//! Threads are able to have associated names for identification purposes. By
//! default, spawned threads are unnamed. To specify a name for a thread, build
//! the thread with [`Builder`] and pass the desired thread name to
//! [`Builder::name`]. To retrieve the thread name from within the thread, use
//! [`Thread::name`]. A couple of examples where the name of a thread gets used:
//!
//! * If a panic occurs in a named thread, the thread name will be printed in
//!   the panic message.
//! * The thread name is provided to the OS where applicable (e.g.,
//!   `pthread_setname_np` in unix-like platforms).
//!
//! ## Stack size
//!
//! The default stack size is platform-dependent and subject to change.
//! Currently, it is 2 MiB on all Tier-1 platforms.
//!
//! There are two ways to manually specify the stack size for spawned threads:
//!
//! * Build the thread with [`Builder`] and pass the desired stack size to
//!   [`Builder::stack_size`].
//! * Set the `RUST_MIN_STACK` environment variable to an integer representing
//!   the desired stack size (in bytes). Note that setting
//!   [`Builder::stack_size`] will override this. Be aware that changes to
//!   `RUST_MIN_STACK` may be ignored after program start.
//!
//! Note that the stack size of the main thread is *not* determined by Rust.
//!
//! [channels]: crate::sync::mpsc
//! [`join`]: JoinHandle::join
//! [`Result`]: core::result::Result
//! [`Ok`]: core::result::Result::Ok
//! [`Err`]: core::result::Result::Err
//! [`thread::current`]: current
//! [`thread::Result`]: Result
//! [`Cell`]: core::cell::Cell
//! [`RefCell`]: core::cell::RefCell
//! [`panic!`]: core::panic!

#![no_implicit_prelude]
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_bugtrack;
extern crate xrmt_winapi;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Into};
use core::hash::{Hash, Hasher};
use core::marker::{Copy, Send, Sync};
use core::mem::{drop, transmute, ManuallyDrop, MaybeUninit};
use core::num::{NonZero, NonZeroU32, NonZeroU64};
use core::ops::{Deref, Drop, FnOnce};
use core::option::Option::{self, None, Some};
use core::result::Result::{Err, Ok};
use core::time::Duration;
use core::{result, u32};

use xrmt_bugtrack::bugtrack;
use xrmt_winapi::functions::{duration_to_micros, CreateThreadEx, GetCurrentProcessPEB, GetCurrentThreadID, GetThreadID, NtYieldExecution, OpenThread, SleepEx, WaitForSingleObject};
use xrmt_winapi::structs::OwnedHandle;
use xrmt_winapi::{CURRENT_PROCESS, INFINITE};

use crate::abort_unlikely;
use crate::io::{ErrorKind, IoError, IoResult};
use crate::os::Handle;
use crate::time::Instant;

const STACK_SIZE: usize = 0x200000usize;

/// Thread factory, which can be used in order to configure the properties of
/// a new thread.
///
/// Methods can be chained on it in order to configure it.
///
/// The two configurations available are:
///
/// - `name`: specifies an [associated name for the thread][naming-threads]
/// - [`stack_size`]: specifies the [desired stack size for the
///   thread][stack-size]
///
/// The [`spawn`] method will take ownership of the builder and create an
/// [`IoResult`] to the thread handle with the given configuration.
///
/// The [`thread::spawn`] free function uses a `Builder` with default
/// configuration and [`unwrap`]s its return value.
///
/// You may want to use [`spawn`] instead of [`thread::spawn`], when you want
/// to recover from a failure to launch a thread, indeed the free function will
/// panic where the `Builder` method will return a [`IoResult`].
///
/// # Examples
///
/// ```
/// use xrmt_stx::thread;
///
/// let builder = thread::Builder::new();
///
/// let handler = builder.spawn(|| {
///     // thread code
/// }).unwrap();
///
/// handler.join().unwrap();
/// ```
///
/// [`stack_size`]: Builder::stack_size
/// [`spawn`]: Builder::spawn
/// [`thread::spawn`]: spawn
/// [`IoResult`]: crate::IoResult
/// [`unwrap`]: core::result::Result::unwrap
/// [naming-threads]: ./index.html#naming-threads
/// [stack-size]: ./index.html#stack-size
pub struct Builder {
    stack_size: Option<usize>,
}
/// An owned permission to join on a thread (block on its termination).
///
/// A `JoinHandle` *detaches* the associated thread when it is dropped, which
/// means that there is no longer any handle to the thread and no way to `join`
/// on it.
///
/// Due to platform restrictions, it is not possible to [`Clone`] this
/// handle: the ability to join a thread is a uniquely-owned permission.
///
/// This `struct` is created by the [`thread::spawn`] function and the
/// [`thread::Builder::spawn`] method.
///
/// # Examples
///
/// Creation from [`thread::spawn`]:
///
/// ```
/// use xrmt_stx::thread;
///
/// let join_handle: thread::JoinHandle<_> = thread::spawn(|| {
///     // some work here
/// });
/// ```
///
/// Creation from [`thread::Builder::spawn`]:
///
/// ```
/// use xrmt_stx::thread;
///
/// let builder = thread::Builder::new();
///
/// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
///     // some work here
/// }).unwrap();
/// ```
///
/// A thread being detached and outliving the thread that spawned it:
///
/// ```no_run
/// use xrmt_stx::thread;
/// use xrmt_stx::time::Duration;
///
/// let original_thread = thread::spawn(|| {
///     let _detached_thread = thread::spawn(|| {
///         // Here we sleep to make sure that the first thread returns before.
///         thread::sleep(Duration::from_millis(10));
///         // This will be called, even though the JoinHandle is dropped.
///         println!("♫ Still alive ♫");
///     });
/// });
///
/// original_thread.join().expect("The thread being joined has panicked");
/// println!("Original thread is joined.");
///
/// // We make sure that the new thread has time to run, before the main
/// // thread returns.
///
/// thread::sleep(Duration::from_millis(1000));
/// ```
///
/// [`thread::Builder::spawn`]: Builder::spawn
/// [`thread::spawn`]: spawn
pub struct JoinHandle<T> {
    thread: Thread,
    result: Arc<UnsafeCell<Option<T>>>,
}
/// A handle to a thread.
///
/// Threads are represented via the `Thread` type, which you can get in one of
/// two ways:
///
/// * By spawning a new thread, e.g., using the [`thread::spawn`][`spawn`]
///   function, and calling [`thread`][`JoinHandle::thread`] on the
///   [`JoinHandle`].
/// * By requesting the current thread, using the [`thread::current`] function.
///
/// The [`thread::current`] function is available even for threads not spawned
/// by the APIs of this module.
///
/// There is usually no need to create a `Thread` struct yourself, one
/// should instead use a function like `spawn` to create new threads, see the
/// docs of [`Builder`] and [`spawn`] for more details.
///
/// [`thread::current`]: current
#[repr(transparent)]
pub struct Thread(OwnedHandle);
/// A unique identifier for a running thread.
///
/// A `ThreadId` is an opaque object that uniquely identifies each thread
/// created during the lifetime of a process. `ThreadId`s are guaranteed not to
/// be reused, even when a thread terminates. `ThreadId`s are under the control
/// of Rust's standard library and there may not be any relationship between
/// `ThreadId` and the underlying platform's notion of a thread identifier --
/// the two concepts cannot, therefore, be used interchangeably. A `ThreadId`
/// can be retrieved from the [`id`] method on a [`Thread`].
///
/// # Examples
///
/// ```
/// use xrmt_stx::thread;
///
/// let other_thread = thread::spawn(|| {
///     thread::current().id()
/// });
///
/// let other_thread_id = other_thread.join().unwrap();
/// assert!(thread::current().id() != other_thread_id);
/// ```
///
/// [`id`]: Thread::id
#[repr(transparent)]
pub struct ThreadId(NonZeroU32);

/// A specialized [`Result`] type for threads.
///
/// Indicates the manner in which a thread exited.
///
/// The value contained in the `Result::Err` variant
/// is the value the thread panicked with;
/// that is, the argument the `panic!` macro was called with.
/// Unlike with normal errors, this value doesn't implement
/// the [`Error`](core::error::Error) trait.
///
/// Thus, a sensible way to handle a thread panic is to either:
///
/// 1. propagate the panic with `xrmt_stx::panic::resume_unwind`
/// 2. or in case the thread is intended to be a subsystem boundary
/// that is supposed to isolate system-level failures,
/// match on the `Err` variant and handle the panic in an appropriate way
///
/// A thread that completes without panicking is considered to exit
/// successfully.
///
/// # Examples
///
/// Matching on the result of a joined thread:
///
/// ```no_run
/// use xrmt_stx::{fs, thread, panic};
///
/// fn copy_in_thread() -> thread::Result<()> {
///     thread::spawn(|| {
///         fs::copy("foo.txt", "bar.txt").unwrap();
///     }).join()
/// }
///
/// fn main() {
///     match copy_in_thread() {
///         Ok(_) => println!("copy succeeded"),
///         Err(e) => panic::resume_unwind(e),
///     }
/// }
/// ```
///
/// [`Result`]: core::result::Result
pub type Result<T> = result::Result<T, IoError>;

struct MaybeDangling<T>(MaybeUninit<T>);

impl Thread {
    /// Constructs a `Thread` from a raw pointer.
    ///
    /// The raw pointer must have been previously returned
    /// by a call to [`Thread::into_raw`].
    ///
    /// # Safety
    ///
    /// This function is unsafe because improper use may lead
    /// to memory unsafety, even if the returned `Thread` is never
    /// accessed.
    ///
    /// Creating a `Thread` from a pointer other than one returned
    /// from [`Thread::into_raw`] is **undefined behavior**.
    ///
    /// Calling this function twice on the same raw pointer can lead
    /// to a double-free if both `Thread` instances are dropped.
    #[inline]
    pub unsafe fn from_raw(ptr: *const ()) -> Thread {
        Thread(unsafe { *(Box::from_raw(ptr as *mut OwnedHandle)) })
    }

    /// Gets the thread's unique identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let other_thread = thread::spawn(|| {
    ///     thread::current().id()
    /// });
    ///
    /// let other_thread_id = other_thread.join().unwrap();
    /// assert!(thread::current().id() != other_thread_id);
    /// ```
    #[inline]
    pub fn id(&self) -> ThreadId {
        GetThreadID(&self.0).map_or(
            ThreadId(unsafe { NonZeroU32::new_unchecked(0xFFFFFFFF) }),
            |v| ThreadId(unsafe { NonZeroU32::new_unchecked(v) }),
        )
    }
    /// Gets the thread's name.
    ///
    /// For more information about named threads, see
    /// [this module-level documentation][naming-threads].
    ///
    /// # Examples
    ///
    /// Threads by default have no name specified:
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     assert!(thread::current().name().is_none());
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// Thread with a specified name:
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[inline]
    pub fn name(&self) -> Option<&str> {
        None
    }
    /// Consumes the `Thread`, returning a raw pointer.
    ///
    /// To avoid a memory leak the pointer must be converted
    /// back into a `Thread` using [`Thread::from_raw`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread::{self, Thread};
    ///
    /// let thread = thread::current();
    /// let id = thread.id();
    /// let ptr = Thread::into_raw(thread);
    /// unsafe {
    ///     assert_eq!(Thread::from_raw(ptr).id(), id);
    /// }
    /// ```
    #[inline]
    pub fn into_raw(self) -> *const () {
        Box::into_raw(Box::new(self.0)) as *const ()
    }
}
impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new()
    ///                               .name("foo".into())
    ///                               .stack_size(32 * 1024);
    ///
    /// let handler = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    #[inline]
    pub fn new() -> Builder {
        Builder { stack_size: None }
    }

    /// Names the thread-to-be. Currently the name is used for identification
    /// only in panic messages.
    ///
    /// The name must not contain null bytes (`\0`).
    ///
    /// For more information about named threads, see
    /// [this module-level documentation][naming-threads].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[inline]
    pub fn name(self, _name: String) -> Builder {
        self
    }
    /// Sets the size of the stack (in bytes) for the new thread.
    ///
    /// The actual stack size may be greater than this value if
    /// the platform specifies a minimal stack size.
    ///
    /// For more information about the stack size for threads, see
    /// [this module-level documentation][stack-size].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new().stack_size(32 * 1024);
    /// ```
    ///
    /// [stack-size]: ./index.html#stack-size
    #[inline]
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Some(size);
        self
    }
    /// Spawns a new thread by taking ownership of the `Builder`, and returns an
    /// [`IoResult`] to its [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller (unless the caller thread
    /// is the main thread; the whole process is terminated when the main
    /// thread finishes). The join handle can be used to block on
    /// termination of the spawned thread, including recovering its panics.
    ///
    /// For a more complete documentation see [`thread::spawn`][`spawn`].
    ///
    /// # Errors
    ///
    /// Unlike the [`spawn`] free function, this method yields an
    /// [`IoResult`] to capture any failure to create the thread at
    /// the OS level.
    ///
    /// [`IoResult`]: crate::IoResult
    ///
    /// # Panics
    ///
    /// Panics if a thread name was set and it contained null bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    #[inline]
    pub fn spawn<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(self, f: F) -> IoResult<JoinHandle<T>> {
        unsafe { self.spawn_unchecked(f) }
    }

    /// Spawns a new thread without any lifetime restrictions by taking
    /// ownership of the `Builder`, and returns an [`IoResult`] to its
    /// [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller (unless the caller thread
    /// is the main thread; the whole process is terminated when the main
    /// thread finishes). The join handle can be used to block on
    /// termination of the spawned thread, including recovering its panics.
    ///
    /// This method is identical to
    /// [`thread::Builder::spawn`][`Builder::spawn`], except for the relaxed
    /// lifetime bounds, which render it unsafe. For a more complete
    /// documentation see [`thread::spawn`][`spawn`].
    ///
    /// # Errors
    ///
    /// Unlike the [`spawn`] free function, this method yields an
    /// [`IoResult`] to capture any failure to create the thread at
    /// the OS level.
    ///
    /// # Panics
    ///
    /// Panics if a thread name was set and it contained null bytes.
    ///
    /// # Safety
    ///
    /// The caller has to ensure that the spawned thread does not outlive any
    /// references in the supplied thread closure and its return type.
    /// This can be guaranteed in two ways:
    ///
    /// - ensure that [`join`][`JoinHandle::join`] is called before any
    ///   referenced
    /// data is dropped
    /// - use only types with `'static` lifetime bounds, i.e., those with no or
    ///   only
    /// `'static` references (both [`thread::Builder::spawn`][`Builder::spawn`]
    /// and [`thread::spawn`][`spawn`] enforce this property statically)
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let x = 1;
    /// let thread_x = &x;
    ///
    /// let handler = unsafe {
    ///     builder.spawn_unchecked(move || {
    ///         println!("x = {}", *thread_x);
    ///     }).unwrap()
    /// };
    ///
    /// // caller has to ensure `join()` is called, otherwise
    /// // it is possible to access freed memory if `x` gets
    /// // dropped before the thread closure is executed!
    /// handler.join().unwrap();
    /// ```
    ///
    /// [`IoResult`]: crate::IoResult
    #[inline]
    pub unsafe fn spawn_unchecked<'a, F: FnOnce() -> T + Send + 'a, T: Send + 'a>(self, f: F) -> IoResult<JoinHandle<T>> {
        let x: Arc<UnsafeCell<Option<T>>> = Arc::new(UnsafeCell::new(None));
        let i = x.clone();
        let m = MaybeDangling::new(f);
        let func = move || {
            let r = (m.into_inner())();
            unsafe { *i.get() = Some(r) };
            drop(i);
        };
        let a = Box::into_raw(Box::new(unsafe {
            Box::from_raw(Box::into_raw(Box::new(func)) as *mut (dyn FnOnce() + 'a))
        }));
        match CreateThreadEx(
            CURRENT_PROCESS,
            self.stack_size.unwrap_or(STACK_SIZE),
            thread_main as *const () as usize,
            a as *mut Box<dyn FnOnce()> as usize,
            false,
        ) {
            Err(e) => {
                drop(unsafe { Box::from_raw(a) });
                Err(e.into())
            },
            Ok(h) => {
                bugtrack!("spawn_unchecked(): Created a new thread 0x{h:X}!");
                Ok(JoinHandle { result: x, thread: Thread(h) })
            },
        }
    }
}
impl ThreadId {
    /// This returns a numeric identifier for the thread identified by this
    /// `ThreadId`.
    ///
    /// As noted in the documentation for the type itself, it is essentially an
    /// opaque ID, but is guaranteed to be unique for each thread. The returned
    /// value is entirely opaque -- only equality testing is stable. Note that
    /// it is not guaranteed which values new threads will return, and this may
    /// change across Rust versions.
    #[inline]
    pub fn as_u64(&self) -> NonZero<u64> {
        unsafe { NonZeroU64::new_unchecked(self.0.get() as u64) }
    }
}
impl<T> JoinHandle<T> {
    /// Waits for the associated thread to finish.
    ///
    /// This function will return immediately if the associated thread has
    /// already finished.
    ///
    /// In terms of [atomic memory orderings],  the completion of the associated
    /// thread synchronizes with this function returning. In other words, all
    /// operations performed by that thread [happen
    /// before](https://doc.rust-lang.org/nomicon/atomics.html#data-accesses) all
    /// operations that happen after `join` returns.
    ///
    /// If the associated thread panics, [`Err`] is returned with the parameter
    /// given to [`panic!`] (though see the Notes below).
    ///
    /// [atomic memory orderings]: core::sync::atomic
    ///
    /// # Panics
    ///
    /// This function may panic on some platforms if a thread attempts to join
    /// itself or otherwise may create a deadlock with joining threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // some work here
    /// }).unwrap();
    /// join_handle.join().expect("Couldn't join on the associated thread");
    /// ```
    ///
    /// # Notes
    ///
    /// If a "foreign" unwinding operation (e.g. an exception thrown from C++
    /// code, or a `panic!` in Rust code compiled or linked with a different
    /// runtime) unwinds all the way to the thread root, the process may be
    /// aborted; see the Notes on [`thread::spawn`]. If the process is not
    /// aborted, this function will return a `Result::Err` containing an opaque
    /// type.
    ///
    /// [`panic!`]: core::panic!
    /// [`thread::spawn`]: spawn
    #[inline]
    pub fn join(self) -> Result<T> {
        self.done()
    }
    /// Extracts a handle to the underlying thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // some work here
    /// }).unwrap();
    ///
    /// let thread = join_handle.thread();
    /// println!("thread id: {:?}", thread.id());
    /// ```
    #[inline]
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
    /// Checks if the associated thread has finished running its main function.
    ///
    /// `is_finished` supports implementing a non-blocking join operation, by
    /// checking `is_finished`, and calling `join` if it returns `true`.
    /// This function does not block. To block while waiting on the thread
    /// to finish, use [`join`][Self::join].
    ///
    /// This might return `true` for a brief moment after the thread's main
    /// function has returned, but before the thread itself has stopped running.
    /// However, once this returns `true`, [`join`][Self::join] can be expected
    /// to return quickly, without blocking for any significant amount of time.
    #[inline]
    pub fn is_finished(&self) -> bool {
        Arc::strong_count(&self.result) == 1
    }

    fn done(mut self) -> Result<T> {
        WaitForSingleObject(self.thread, INFINITE, false)?;
        match Arc::get_mut(&mut self.result) {
            Some(v) => v.get_mut().take().ok_or_else(|| IoError::from(ErrorKind::InProgress)),
            None => Err(IoError::from(ErrorKind::BrokenPipe)),
        }
    }
}
impl<T> MaybeDangling<T> {
    #[inline]
    fn new(x: T) -> MaybeDangling<T> {
        MaybeDangling(MaybeUninit::new(x))
    }

    #[inline]
    fn into_inner(self) -> T {
        unsafe { ManuallyDrop::new(self).0.assume_init_read() }
    }
}

impl Clone for Thread {
    #[inline]
    fn clone(&self) -> Thread {
        Thread(abort_unlikely!(self.0.duplicate()))
    }
}
impl AsRef<Handle> for Thread {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

impl Eq for ThreadId {}
impl Copy for ThreadId {}
impl Hash for ThreadId {
    #[inline]
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}
impl Clone for ThreadId {
    #[inline]
    fn clone(&self) -> ThreadId {
        ThreadId(self.0)
    }
}
impl Deref for ThreadId {
    type Target = u32;

    #[inline]
    fn deref(&self) -> &u32 {
        unsafe { transmute(self) }
    }
}
impl PartialEq for ThreadId {
    #[inline]
    fn eq(&self, other: &ThreadId) -> bool {
        self.0 == other.0
    }
}

impl<T> AsRef<Handle> for JoinHandle<T> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        self.thread.as_ref()
    }
}

impl<T> Drop for MaybeDangling<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.0.assume_init_drop() };
    }
}

impl From<Thread> for OwnedHandle {
    #[inline]
    fn from(v: Thread) -> OwnedHandle {
        v.0
    }
}
impl<T> From<JoinHandle<T>> for OwnedHandle {
    #[inline]
    fn from(v: JoinHandle<T>) -> OwnedHandle {
        v.thread.0
    }
}

unsafe impl<T> Send for JoinHandle<T> {}
unsafe impl<T> Sync for JoinHandle<T> {}

/// Cooperatively gives up a timeslice to the OS scheduler.
///
/// This calls the underlying OS scheduler's yield primitive, signaling
/// that the calling thread is willing to give up its remaining timeslice
/// so that the OS may schedule other threads on the CPU.
///
/// A drawback of yielding in a loop is that if the OS does not have any
/// other ready threads to run on the current CPU, the thread will effectively
/// busy-wait, which wastes CPU time and energy.
///
/// Therefore, when waiting for events of interest, a programmer's first
/// choice should be to use synchronization devices such as [`channel`]s,
/// [`Condvar`]s, [`Mutex`]es or [`join`] since these primitives are
/// implemented in a blocking manner, giving up the CPU until the event
/// of interest has occurred which avoids repeated yielding.
///
/// `yield_now` should thus be used only rarely, mostly in situations where
/// repeated polling is required because there is no other suitable way to
/// learn when an event of interest has occurred.
///
/// # Examples
///
/// ```
/// use xrmt_stx::thread;
///
/// thread::yield_now();
/// ```
///
/// [`channel`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`Condvar`]: crate::sync::Condvar
/// [`Mutex`]: crate::sync::Mutex
#[inline]
pub fn yield_now() {
    let _ = NtYieldExecution();
}
#[inline]
pub fn current() -> Thread {
    // 0x1FFFFF - ALL_ACCESS
    let h = match OpenThread(0x1FFFFF, false, GetCurrentThreadID()) {
        Ok(v) => v,
        Err(_) => unsafe { OwnedHandle::empty() },
    };
    Thread(h)
}
/// Puts the current thread to sleep for at least the specified amount of time.
///
/// The thread may sleep longer than the duration specified due to scheduling
/// specifics or platform-dependent functionality. It will never sleep less.
///
/// This function is blocking, and should not be used in `async` functions.
///
/// # Platform-specific behavior
///
/// On Unix platforms, the underlying syscall may be interrupted by a
/// spurious wakeup or signal handler. To ensure the sleep occurs for at least
/// the specified duration, this function may invoke that system call multiple
/// times.
/// Platforms which do not support nanosecond precision for sleeping will
/// have `dur` rounded up to the nearest granularity of time they can sleep for.
///
/// Currently, specifying a zero duration on Unix platforms returns immediately
/// without invoking the underlying [`nanosleep`] syscall, whereas on Windows
/// platforms the underlying [`Sleep`] syscall is always invoked.
/// If the intention is to yield the current time-slice you may want to use
/// [`yield_now`] instead.
///
/// [`nanosleep`]: https://linux.die.net/man/2/nanosleep
/// [`Sleep`]: https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-sleep
///
/// # Examples
///
/// ```no_run
/// use xrmt_stx::{thread, time};
///
/// let ten_millis = time::Duration::from_millis(10);
/// let now = time::Instant::now();
///
/// thread::sleep(ten_millis);
///
/// assert!(now.elapsed() >= ten_millis);
/// ```
#[inline]
pub fn sleep(dur: Duration) {
    let _ = SleepEx(duration_to_micros(dur), false);
}
/// Puts the current thread to sleep until the specified deadline has passed.
///
/// The thread may still be asleep after the deadline specified due to
/// scheduling specifics or platform-dependent functionality. It will never
/// wake before.
///
/// This function is blocking, and should not be used in `async` functions.
///
/// # Platform-specific behavior
///
/// This function uses [`sleep`] internally, see its platform-specific behavior.
///
///
/// # Examples
///
/// A simple game loop that limits the game to 60 frames per second.
///
/// ```no_run
/// let max_fps = 60.0;
/// let frame_time = Duration::from_secs_f32(1.0/max_fps);
/// let mut next_frame = Instant::now();
/// loop {
///     thread::sleep_until(next_frame);
///     next_frame += frame_time;
///     update();
///     render();
/// }
/// ```
///
/// A slow api we must not call too fast and which takes a few
/// tries before succeeding. By using `sleep_until` the time the
/// api call takes does not influence when we retry or when we give up
///
/// ```no_run
/// let deadline = Instant::now() + MAX_DURATION;
/// let delay = Duration::from_millis(250);
/// let mut next_attempt = Instant::now();
/// loop {
///     if Instant::now() > deadline {
///         break Err(());
///     }
///     if let Status::Ready(data) = slow_web_api_call() {
///         break Ok(data);
///     }
///
///     next_attempt = deadline.min(next_attempt + delay);
///     thread::sleep_until(next_attempt);
/// }
/// ```
#[inline]
pub fn sleep_until(deadline: Instant) {
    let d = deadline - Instant::now();
    if !d.is_zero() {
        sleep(d)
    }
}
/// Returns an estimate of the default amount of parallelism a program should
/// use.
///
/// Parallelism is a resource. A given machine provides a certain capacity for
/// parallelism, i.e., a bound on the number of computations it can perform
/// simultaneously. This number often corresponds to the amount of CPUs a
/// computer has, but it may diverge in various cases.
///
/// Host environments such as VMs or container orchestrators may want to
/// restrict the amount of parallelism made available to programs in them. This
/// is often done to limit the potential impact of (unintentionally)
/// resource-intensive programs on other programs running on the same machine.
///
/// # Limitations
///
/// The purpose of this API is to provide an easy and portable way to query
/// the default amount of parallelism the program should use. Among other things
/// it does not expose information on NUMA regions, does not account for
/// differences in (co)processor capabilities or current system load,
/// and will not modify the program's global state in order to more accurately
/// query the amount of available parallelism.
///
/// Where both fixed steady-state and burst limits are available the
/// steady-state capacity will be used to ensure more predictable latencies.
///
/// Resource limits can be changed during the runtime of a program, therefore
/// the value is not cached and instead recomputed every time this function is
/// called. It should not be called from hot code.
///
/// The value returned by this function should be considered a simplified
/// approximation of the actual amount of parallelism available at any given
/// time. To get a more detailed or precise overview of the amount of
/// parallelism available to the program, you may wish to use
/// platform-specific APIs as well. The following platform limitations currently
/// apply to `available_parallelism`:
///
/// On Windows:
/// - It may undercount the amount of parallelism available on systems with more
///   than 64 logical CPUs. However, programs typically need specific support to
///   take advantage of more than 64 logical CPUs, and in the absence of such
///   support, the number returned by this function accurately reflects the
///   number of logical CPUs the program can use by default.
/// - It may overcount the amount of parallelism available on systems limited by
///   process-wide affinity masks, or job object limitations.
///
/// On Linux:
/// - It may overcount the amount of parallelism available when limited by a
///   process-wide affinity mask or cgroup quotas and `sched_getaffinity()` or
///   cgroup fs can't be queried, e.g. due to sandboxing.
/// - It may undercount the amount of parallelism if the current thread's
///   affinity mask does not reflect the process' cpuset, e.g. due to pinned
///   threads.
/// - If the process is in a cgroup v1 cpu controller, this may need to scan
///   mountpoints to find the corresponding cgroup v1 controller, which may take
///   time on systems with large numbers of mountpoints. (This does not apply to
///   cgroup v2, or to processes not in a cgroup.)
///
/// On all targets:
/// - It may overcount the amount of parallelism available when running in a VM
/// with CPU usage limits (e.g. an overcommitted host).
///
/// # Errors
///
/// This function will, but is not limited to, return errors in the following
/// cases:
///
/// - If the amount of parallelism is not known for the target platform.
/// - If the program lacks permission to query the amount of parallelism made
///   available to it.
///
/// # Examples
///
/// ```
/// # #![allow(dead_code)]
/// use xrmt_stx::{io, thread};
///
/// fn main() -> IoResult<()> {
///     let count = thread::available_parallelism()?.get();
///     assert!(count >= 1_usize);
///     Ok(())
/// }
/// ```
#[inline]
pub fn available_parallelism() -> Result<NonZero<usize>> {
    NonZero::new(GetCurrentProcessPEB().number_of_processors as usize).ok_or_else(|| IoError::from(ErrorKind::InvalidData))
}
/// Spawns a new thread, returning a [`JoinHandle`] for it.
///
/// The join handle provides a [`join`] method that can be used to join the
/// spawned thread. If the spawned thread panics, [`join`] will return an
/// [`Err`] containing the argument given to [`panic!`].
///
/// If the join handle is dropped, the spawned thread will implicitly be
/// *detached*. In this case, the spawned thread may no longer be joined.
/// (It is the responsibility of the program to either eventually join threads
/// it creates or detach them; otherwise, a resource leak will result.)
///
/// This call will create a thread using default parameters of [`Builder`], if
/// you want to specify the stack size or the name of the thread, use this API
/// instead.
///
/// As you can see in the signature of `spawn` there are two constraints on
/// both the closure given to `spawn` and its return value, let's explain them:
///
/// - The `'static` constraint means that the closure and its return value must
///   have a lifetime of the whole program execution. The reason for this is
///   that threads can outlive the lifetime they have been created in.
///
///   Indeed if the thread, and by extension its return value, can outlive their
///   caller, we need to make sure that they will be valid afterwards, and since
///   we *can't* know when it will return we need to have them valid as long as
///   possible, that is until the end of the program, hence the `'static`
///   lifetime.
/// - The [`Send`] constraint is because the closure will need to be passed *by
///   value* from the thread where it is spawned to the new thread. Its return
///   value will need to be passed from the new thread to the thread where it is
///   `join`ed. As a reminder, the [`Send`] marker trait expresses that it is
///   safe to be passed from thread to thread. [`Sync`] expresses that it is
///   safe to have a reference be passed from thread to thread.
///
/// # Panics
///
/// Panics if the OS fails to create a thread; use [`Builder::spawn`]
/// to recover from such errors.
///
/// # Examples
///
/// Creating a thread.
///
/// ```
/// use xrmt_stx::thread;
///
/// let handler = thread::spawn(|| {
///     // thread code
/// });
///
/// handler.join().unwrap();
/// ```
///
/// As mentioned in the module documentation, threads are usually made to
/// communicate using [`channels`], here is how it usually looks.
///
/// This example also shows how to use `move`, in order to give ownership
/// of values to a thread.
///
/// ```
/// use xrmt_stx::thread;
/// use xrmt_stx::sync::mpsc::channel;
///
/// let (tx, rx) = channel();
///
/// let sender = thread::spawn(move || {
///     tx.send("Hello, thread".to_owned())
///         .expect("Unable to send on channel");
/// });
///
/// let receiver = thread::spawn(move || {
///     let value = rx.recv().expect("Unable to receive from channel");
///     println!("{value}");
/// });
///
/// sender.join().expect("The sender thread has panicked");
/// receiver.join().expect("The receiver thread has panicked");
/// ```
///
/// A thread can also return a value through its [`JoinHandle`], you can use
/// this to make asynchronous computations (futures might be more appropriate
/// though).
///
/// ```
/// use xrmt_stx::thread;
///
/// let computation = thread::spawn(|| {
///     // Some expensive computation.
///     42
/// });
///
/// let result = computation.join().unwrap();
/// println!("{result}");
/// ```
///
/// # Notes
///
/// This function has the same minimal guarantee regarding "foreign" unwinding
/// operations (e.g. an exception thrown from C++ code, or a `panic!` in Rust
/// code compiled or linked with a different runtime) as [`catch_unwind`];
/// namely, if the thread created with `thread::spawn` unwinds all the way to
/// the root with such an exception, one of two behaviors are possible,
/// and it is unspecified which will occur:
///
/// * The process aborts.
/// * The process does not abort, and [`join`] will return a `Result::Err`
///   containing an opaque type.
///
/// [`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
/// [`channels`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`panic!`]: core::panic!
#[inline]
pub fn spawn<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) -> JoinHandle<T> {
    Builder::new().spawn(f).unwrap()
}

unsafe extern "system" fn thread_main(func: usize) -> u32 {
    unsafe { Box::from_raw(func as *mut Box<dyn FnOnce()>)() };
    0
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};

    use crate::thread::{Thread, ThreadId};

    impl Debug for Thread {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_tuple("Thread").field(&self.0).finish()
        }
    }
    impl Debug for ThreadId {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_tuple("ThreadId").field(&self.0).finish()
        }
    }
}
