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

//! Useful synchronization primitives.
//!
//! ## The need for synchronization
//!
//! Conceptually, a Rust program is a series of operations which will
//! be executed on a computer. The timeline of events happening in the
//! program is consistent with the order of the operations in the code.
//!
//! Consider the following code, operating on some global static variables:
//!
//! ```rust
//! // FIXME(static_mut_refs): Do not allow `static_mut_refs` lint
//! #![allow(static_mut_refs)]
//!
//! static mut A: u32 = 0;
//! static mut B: u32 = 0;
//! static mut C: u32 = 0;
//!
//! fn main() {
//!     unsafe {
//!         A = 3;
//!         B = 4;
//!         A = A + B;
//!         C = B;
//!         println!("{A} {B} {C}");
//!         C = A;
//!     }
//! }
//! ```
//!
//! It appears as if some variables stored in memory are changed, an addition
//! is performed, result is stored in `A` and the variable `C` is
//! modified twice.
//!
//! When only a single thread is involved, the results are as expected:
//! the line `7 4 4` gets printed.
//!
//! As for what happens behind the scenes, when optimizations are enabled the
//! final generated machine code might look very different from the code:
//!
//! - The first store to `C` might be moved before the store to `A` or `B`, _as
//!   if_ we had written `C = 4; A = 3; B = 4`.
//!
//! - Assignment of `A + B` to `A` might be removed, since the sum can be stored
//!   in a temporary location until it gets printed, with the global variable
//!   never getting updated.
//!
//! - The final result could be determined just by looking at the code at
//!   compile time, so [constant folding] might turn the whole block into a
//!   simple `println!("7 4 4")`.
//!
//! The compiler is allowed to perform any combination of these
//! optimizations, as long as the final optimized code, when executed,
//! produces the same results as the one without optimizations.
//!
//! Due to the [concurrency] involved in modern computers, assumptions
//! about the program's execution order are often wrong. Access to
//! global variables can lead to nondeterministic results, **even if**
//! compiler optimizations are disabled, and it is **still possible**
//! to introduce synchronization bugs.
//!
//! Note that thanks to Rust's safety guarantees, accessing global (static)
//! variables requires `unsafe` code, assuming we don't use any of the
//! synchronization primitives in this module.
//!
//! [constant folding]: https://en.wikipedia.org/wiki/Constant_folding
//! [concurrency]: https://en.wikipedia.org/wiki/Concurrency_(computer_science)
//!
//! ## Out-of-order execution
//!
//! Instructions can execute in a different order from the one we define, due to
//! various reasons:
//!
//! - The **compiler** reordering instructions: If the compiler can issue an
//!   instruction at an earlier point, it will try to do so. For example, it
//!   might hoist memory loads at the top of a code block, so that the CPU can
//!   start [prefetching] the values from memory.
//!
//!   In single-threaded scenarios, this can cause issues when writing
//!   signal handlers or certain kinds of low-level code.
//!   Use [compiler fences] to prevent this reordering.
//!
//! - A **single processor** executing instructions [out-of-order]: Modern CPUs
//!   are capable of [superscalar] execution, i.e., multiple instructions might
//!   be executing at the same time, even though the machine code describes a
//!   sequential process.
//!
//!   This kind of reordering is handled transparently by the CPU.
//!
//! - A **multiprocessor** system executing multiple hardware threads at the
//!   same time: In multi-threaded scenarios, you can use two kinds of
//!   primitives to deal with synchronization:
//!   - [memory fences] to ensure memory accesses are made visible to
//!   other CPUs in the right order.
//!   - [atomic operations] to ensure simultaneous access to the same
//!   memory location doesn't lead to undefined behavior.
//!
//! [prefetching]: https://en.wikipedia.org/wiki/Cache_prefetching
//! [compiler fences]: crate::sync::atomic::compiler_fence
//! [out-of-order]: https://en.wikipedia.org/wiki/Out-of-order_execution
//! [superscalar]: https://en.wikipedia.org/wiki/Superscalar_processor
//! [memory fences]: crate::sync::atomic::fence
//! [atomic operations]: crate::sync::atomic
//!
//! ## Higher-level synchronization objects
//!
//! Most of the low-level synchronization primitives are quite error-prone and
//! inconvenient to use, which is why the standard library also exposes some
//! higher-level synchronization objects.
//!
//! These abstractions can be built out of lower-level primitives.
//! For efficiency, the sync objects in the standard library are usually
//! implemented with help from the operating system's kernel, which is
//! able to reschedule the threads while they are blocked on acquiring
//! a lock.
//!
//! The following is an overview of the available synchronization
//! objects:
//!
//! - [`Arc`]: Atomically Reference-Counted pointer, which can be used in
//!   multithreaded environments to prolong the lifetime of some data until all
//!   the threads have finished using it.
//!
//! - [`Barrier`]: Ensures multiple threads will wait for each other to reach a
//!   point in the program, before continuing execution all together.
//!
//! - [`Condvar`]: Condition Variable, providing the ability to block a thread
//!   while waiting for an event to occur.
//!
//! - [`mpsc`]: Multi-producer, single-consumer queues, used for message-based
//!   communication. Can provide a lightweight inter-thread synchronisation
//!   mechanism, at the cost of some extra memory.
//!
//! - [`mpmc`]: Multi-producer, multi-consumer queues, used for message-based
//!   communication. Can provide a lightweight inter-thread synchronisation
//!   mechanism, at the cost of some extra memory.
//!
//! - [`Mutex`]: Mutual Exclusion mechanism, which ensures that at most one
//!   thread at a time is able to access some data.
//!
//! - [`Once`]: Used for a thread-safe, one-time global initialization routine.
//!   Mostly useful for implementing other types like `OnceLock`.
//!
//! - [`OnceLock`]: Used for thread-safe, one-time initialization of a variable,
//!   with potentially different initializers based on the caller.
//!
//! - [`LazyLock`]: Used for thread-safe, one-time initialization of a variable,
//!   using one nullary initializer function provided at creation.
//!
//! - [`RwLock`]: Provides a mutual exclusion mechanism which allows multiple
//!   readers at the same time, while allowing only one writer at a time. In
//!   some cases, this can be more efficient than a mutex.
//!
//! [`Arc`]: crate::sync::Arc
//! [`Barrier`]: crate::sync::Barrier
//! [`Condvar`]: crate::sync::Condvar
//! [`mpmc`]: crate::sync::mpmc
//! [`mpsc`]: crate::sync::mpsc
//! [`Mutex`]: crate::sync::Mutex
//! [`Once`]: crate::sync::Once
//! [`OnceLock`]: crate::sync::OnceLock
//! [`RwLock`]: crate::sync::RwLock

#![no_implicit_prelude]

extern crate alloc;
extern crate core;

pub use alloc::sync::*;
pub use core::sync::*;

pub mod extra;

pub use self::inner::*;

#[cfg(all(target_family = "windows", not(feature = "std")))]
#[path = "sync"]
mod inner {
    extern crate alloc;
    extern crate core;

    mod barrier;
    mod cond;
    mod lazy;
    pub mod mpmc;
    pub mod mpsc;
    mod mutex;
    mod once;
    mod once_lock;
    mod rwlock;

    pub use self::barrier::*;
    pub use self::cond::*;
    pub use self::lazy::*;
    pub use self::mutex::*;
    pub use self::once::*;
    pub use self::once_lock::*;
    pub use self::rwlock::*;
}
#[cfg(any(not(target_family = "windows"), feature = "std"))]
#[path = "."]
mod inner {
    extern crate std;
    pub use std::sync::*;
}
