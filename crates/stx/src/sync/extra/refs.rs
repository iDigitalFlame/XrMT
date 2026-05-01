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

#![no_implicit_prelude]

extern crate alloc;
extern crate core;

use alloc::alloc::Global;
use alloc::boxed::Box;
use core::alloc::{Allocator, Layout};
use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::clone::Clone;
use core::convert::{AsMut, AsRef, From};
use core::default::Default;
use core::hint::{spin_loop, unlikely};
use core::marker::{Send, Sized, Sync};
use core::mem::{align_of_val_raw, drop, ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut, Drop, FnOnce};
use core::option::Option::{self, None, Some};
use core::ptr::{addr_eq, drop_in_place, read, slice_from_raw_parts_mut, NonNull};
use core::result::Result::{self, Err, Ok};
use core::sync::atomic::{fence, AtomicUsize, Ordering};

use crate::{abort, abort_unlikely};

/// `Ref` is similar to the `Ref` struct, but allows for mutation of the shared
/// value and has additional options to reference and manage references.
///
/// While lacking some functions used with `Ref`, [`Ref`] has a smaller
/// footprint and quicker access than the `stdlib` `Ref` struct.
pub struct Ref<T: ?Sized, A: Allocator = Global> {
    ptr:   NonNull<Reference<T>>,
    alloc: A,
}
/// `Weak` is a version of [`Ref`] that holds a non-owning reference to the
/// managed allocation.
///
/// The allocation is accessed by calling [`upgrade`] on the `Weak`
/// pointer, which returns an <code>[Option]<[Ref]\<T>></code>.
///
/// Since a `Weak` reference does not count towards ownership, it will not
/// prevent the value stored in the allocation from being dropped, and `Weak`
/// itself makes no guarantees about the value still being present. Thus it may
/// return [`None`] when [`upgrade`]d. Note however that a `Weak` reference
/// *does* prevent the allocation itself (the backing store) from being
/// deallocated.
///
/// A `Weak` pointer is useful for keeping a temporary reference to the
/// allocation managed by [`Ref`] without preventing its inner value from being
/// dropped. It is also used to prevent circular references between [`Ref`]
/// pointers, since mutual owning references would never allow either [`Ref`] to
/// be dropped. For example, a tree could have strong [`Ref`] pointers from
/// parent nodes to children, and `Weak` pointers from children back to their
/// parents.
///
/// The typical way to obtain a `Weak` pointer is to call [`Ref::downgrade`].
///
/// [`upgrade`]: Weak::upgrade
pub struct Weak<T: ?Sized, A: Allocator = Global> {
    ptr:   NonNull<Reference<T>>,
    alloc: A,
}

struct ReferenceWeak<'a> {
    w: &'a AtomicUsize,
    s: &'a AtomicUsize,
}
#[repr(C)]
struct Reference<T: ?Sized> {
    w: AtomicUsize,
    s: AtomicUsize,
    v: UnsafeCell<T>,
}

impl<T> Ref<T> {
    /// Constructs a new `Ref<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    /// ```
    #[inline]
    pub fn new(v: T) -> Ref<T> {
        Ref::new_in(v, Global)
    }
    /// Constructs a new `Ref` with uninitialized contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let mut five = Ref::<u32>::new_uninit();
    ///
    /// // Deferred initialization:
    /// Ref::get_mut(&mut five).unwrap().write(5);
    ///
    /// let five = unsafe { five.assume_init() };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[inline]
    pub fn new_uninit() -> Ref<MaybeUninit<T>> {
        Ref::new_uninit_in(Global)
    }
    /// Constructs a new `Ref` with uninitialized contents, with the memory
    /// being filled with `0` bytes.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and
    /// incorrect usage of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let zero = Ref::<u32>::new_zeroed();
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0)
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn new_zeroed_uninit() -> Ref<MaybeUninit<T>> {
        Ref::new_zeroed_uninit_in(Global)
    }
}
impl<T> Ref<[T]> {
    /// Constructs a new atomically reference-counted slice with uninitialized
    /// contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let mut values = Ref::<[u32]>::new_uninit_slice(3);
    ///
    /// // Deferred initialization:
    /// let data = Ref::get_mut(&mut values).unwrap();
    /// data[0].write(1);
    /// data[1].write(2);
    /// data[2].write(3);
    ///
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[inline]
    pub fn new_uninit_slice(len: usize) -> Ref<[MaybeUninit<T>]> {
        Ref::new_uninit_slice_in(len, Global)
    }
    /// Constructs a new atomically reference-counted slice with uninitialized
    /// contents, with the memory being filled with `0` bytes.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and
    /// incorrect usage of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let values = Ref::<[u32]>::new_zeroed_slice(3);
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0])
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn new_zeroed_slice(len: usize) -> Ref<[MaybeUninit<T>]> {
        Ref::new_zeroed_slice_in(len, Global)
    }
}
impl<T: ?Sized> Ref<T> {
    /// Constructs an `Ref<T>` from a raw pointer.
    ///
    /// The raw pointer must have been previously returned by a call to
    /// [`Ref<U>::into_raw`][into_raw] with the following requirements:
    ///
    /// * If `U` is sized, it must have the same size and alignment as `T`. This
    ///   is trivially true if `U` is `T`.
    /// * If `U` is unsized, its data pointer must have the same size and
    ///   alignment as `T`. This is trivially true if `Ref<U>` was constructed
    ///   through `Ref<T>` and then converted to `Ref<U>` through an [unsized
    ///   coercion].
    ///
    /// Note that if `U` or `U`'s data pointer is not `T` but has the same size
    /// and alignment, this is basically like transmuting references of
    /// different types. See [`mem::transmute`][transmute] for more information
    /// on what restrictions apply in this case.
    ///
    /// The raw pointer must point to a block of memory allocated by the global
    /// allocator.
    ///
    /// The user of `from_raw` has to make sure a specific value of `T` is only
    /// dropped once.
    ///
    /// This function is unsafe because improper use may lead to memory
    /// unsafety, even if the returned `Ref<T>` is never accessed.
    ///
    /// [into_raw]: Ref::into_raw
    /// [transmute]: core::mem::transmute
    /// [unsized coercion]: https://doc.rust-lang.org/reference/type-coercions.html#unsized-coercions
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x = Ref::new("hello".to_owned());
    /// let x_ptr = Ref::into_raw(x);
    ///
    /// unsafe {
    ///     // Convert back to an `Ref` to prevent leak.
    ///     let x = Ref::from_raw(x_ptr);
    ///     assert_eq!(&*x, "hello");
    ///
    ///     // Further calls to `Ref::from_raw(x_ptr)` would be memory-unsafe.
    /// }
    ///
    /// // The memory was freed when `x` went out of scope above, so `x_ptr` is now dangling!
    /// ```
    ///
    /// Convert a slice back into its original array:
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x: Ref<[u32]> = Ref::new([1, 2, 3]);
    /// let x_ptr: *const [u32] = Ref::into_raw(x);
    ///
    /// unsafe {
    ///     let x: Ref<[u32; 3]> = Ref::from_raw(x_ptr.cast::<[u32; 3]>());
    ///     assert_eq!(&*x, &[1, 2, 3]);
    /// }
    /// ```
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Ref<T> {
        unsafe { Ref::from_raw_in(ptr, Global) }
    }
}
impl<T: ?Sized> Weak<T> {
    /// Converts a raw pointer previously created by [`into_raw`] back into
    /// `Weak<T>`.
    ///
    /// This can be used to safely get a strong reference (by calling
    /// [`upgrade`] later) or to deallocate the weak count by dropping the
    /// `Weak<T>`.
    ///
    /// It takes ownership of one weak reference (with the exception of pointers
    /// created by [`new`], as these don't own anything; the method still
    /// works on them).
    ///
    /// # Safety
    ///
    /// The pointer must have originated from the [`into_raw`] and must still
    /// own its potential weak reference, and must point to a block of
    /// memory allocated by global allocator.
    ///
    /// It is allowed for the strong count to be 0 at the time of calling this.
    /// Nevertheless, this takes ownership of one weak reference currently
    /// represented as a raw pointer (the weak count is not modified by this
    /// operation) and therefore it must be paired with a previous
    /// call to [`into_raw`].
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    ///
    /// let strong = Ref::new("hello".to_owned());
    ///
    /// let raw_1 = Ref::downgrade(&strong).into_raw();
    /// let raw_2 = Ref::downgrade(&strong).into_raw();
    ///
    /// assert_eq!(2, Ref::weak_count(&strong));
    ///
    /// assert_eq!("hello", &*unsafe { Weak::from_raw(raw_1) }.upgrade().unwrap());
    /// assert_eq!(1, Ref::weak_count(&strong));
    ///
    /// drop(strong);
    ///
    /// // Decrement the last weak count.
    /// assert!(unsafe { Weak::from_raw(raw_2) }.upgrade().is_none());
    /// ```
    ///
    /// [`new`]: Weak::new
    /// [`into_raw`]: Weak::into_raw
    /// [`upgrade`]: Weak::upgrade
    #[inline]
    pub unsafe fn from_raw(ptr: *const T) -> Weak<T> {
        unsafe { Weak::from_raw_in(ptr, Global) }
    }
}
impl<T, A: Allocator> Ref<T, A> {
    /// Constructs a new `Ref<T>` in the provided allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let five = Ref::new_in(5, System);
    /// ```
    #[inline]
    pub fn new_in(v: T, alloc: A) -> Ref<T, A> {
        Ref {
            ptr: unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(Reference {
                    v: UnsafeCell::new(v),
                    w: AtomicUsize::new(1),
                    s: AtomicUsize::new(1),
                })))
            },
            alloc,
        }
    }
    /// Constructs a new `Ref` with uninitialized contents in the provided
    /// allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let mut five = Ref::<u32, _>::new_uninit_in(System);
    ///
    /// let five = unsafe {
    ///     // Deferred initialization:
    ///     Ref::get_mut_unchecked(&mut five).as_mut_ptr().write(5);
    ///
    ///     five.assume_init()
    /// };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[inline]
    pub fn new_uninit_in(alloc: A) -> Ref<MaybeUninit<T>, A> {
        Ref {
            ptr: unsafe { NonNull::new_unchecked(uninit(&alloc, false)) },
            alloc,
        }
    }
    /// Constructs a new `Ref` with uninitialized contents, with the memory
    /// being filled with `0` bytes, in the provided allocator.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and
    /// incorrect usage of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let zero = Ref::<u32, _>::new_zeroed_in(System);
    /// let zero = unsafe { zero.assume_init() };
    ///
    /// assert_eq!(*zero, 0)
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn new_zeroed_uninit_in(alloc: A) -> Ref<MaybeUninit<T>, A> {
        Ref {
            ptr: unsafe { NonNull::new_unchecked(uninit(&alloc, true)) },
            alloc,
        }
    }

    /// Returns the inner value, if the `Ref` has exactly one strong reference.
    ///
    /// Otherwise, [`None`] is returned and the `Ref` is dropped.
    ///
    /// This will succeed even if there are outstanding weak references.
    ///
    /// If `Ref::into_inner` is called on every clone of this `Ref`,
    /// it is guaranteed that exactly one of the calls returns the inner value.
    /// This means in particular that the inner value is not dropped.
    ///
    /// [`Ref::try_unwrap`] is conceptually similar to `Ref::into_inner`, but it
    /// is meant for different use-cases. If used as a direct replacement
    /// for `Ref::into_inner` anyway, such as with the expression
    /// <code>[Ref::try_unwrap]\(this).[ok][Result::ok]()</code>, then it does
    /// **not** give the same guarantee as described in the previous paragraph.
    /// For more information, see the examples below and read the documentation
    /// of [`Ref::try_unwrap`].
    ///
    /// # Examples
    ///
    /// Minimal example demonstrating the guarantee that `Ref::into_inner`
    /// gives. ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x = Ref::new(3);
    /// let y = Ref::clone(&x);
    ///
    /// // Two threads calling `Ref::into_inner` on both clones of an `Ref`:
    /// let x_thread = xrmt_stx::thread::spawn(|| Ref::into_inner(x));
    /// let y_thread = xrmt_stx::thread::spawn(|| Ref::into_inner(y));
    ///
    /// let x_inner_value = x_thread.join().unwrap();
    /// let y_inner_value = y_thread.join().unwrap();
    ///
    /// // One of the threads is guaranteed to receive the inner value:
    /// assert!(matches!(
    ///     (x_inner_value, y_inner_value),
    ///     (None, Some(3)) | (Some(3), None)
    /// ));
    /// // The result could also be `(None, None)` if the threads called
    /// // `Ref::try_unwrap(x).ok()` and `Ref::try_unwrap(y).ok()` instead.
    /// ```
    /// 
    /// A more practical example demonstrating the need for `Ref::into_inner`:
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// // Definition of a simple singly linked list using `Ref`:
    /// #[derive(Clone)]
    /// struct LinkedList<T>(Option<Ref<Node<T>>>);
    /// struct Node<T>(T, Option<Ref<Node<T>>>);
    ///
    /// // Dropping a long `LinkedList<T>` relying on the destructor of `Ref`
    /// // can cause a stack overflow. To prevent this, we can provide a
    /// // manual `Drop` implementation that does the destruction in a loop:
    /// impl<T> Drop for LinkedList<T> {
    ///     fn drop(&mut self) {
    ///         let mut link = self.0.take();
    ///         while let Some(arc_node) = link.take() {
    ///             if let Some(Node(_value, next)) = Ref::into_inner(arc_node)
    /// {                 link = next;
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// // Implementation of `new` and `push` omitted
    /// impl<T> LinkedList<T> {
    ///     /* ... */
    /// #   fn new() -> Self {
    /// #       LinkedList(None)
    /// #   }
    /// #   fn push(&mut self, x: T) {
    /// #       self.0 = Some(Ref::new(Node(x, self.0.take())));
    /// #   }
    /// }
    ///
    /// // The following code could have still caused a stack overflow
    /// // despite the manual `Drop` impl if that `Drop` impl had used
    /// // `Ref::try_unwrap(arc).ok()` instead of `Ref::into_inner(arc)`.
    ///
    /// // Create a long list and clone it
    /// let mut x = LinkedList::new();
    /// let size = 100000;
    /// # let size = if cfg!(miri) { 100 } else { size };
    /// for i in 0..size {
    ///     x.push(i); // Adds i to the front of x
    /// }
    /// let y = x.clone();
    ///
    /// // Drop the clones in parallel
    /// let x_thread = xrmt_stx::thread::spawn(|| drop(x));
    /// let y_thread = xrmt_stx::thread::spawn(|| drop(y));
    /// x_thread.join().unwrap();
    /// y_thread.join().unwrap();
    /// ```
    #[inline]
    pub fn into_inner(r: Ref<T, A>) -> Option<T> {
        match Ref::try_unwrap(r) {
            Ok(v) => Some(v),
            Err(v) => {
                // Prevent dropping the value incase or race conditions.
                let _ = ManuallyDrop::new(v);
                None
            },
        }
    }
    /// Returns the inner value, if the `Ref` has exactly one strong reference.
    ///
    /// Otherwise, an [`Err`] is returned with the same `Ref` that was
    /// passed in.
    ///
    /// This will succeed even if there are outstanding weak references.
    ///
    /// It is strongly recommended to use [`Ref::into_inner`] instead if you
    /// don't keep the `Ref` in the [`Err`] case.
    /// Immediately dropping the [`Err`]-value, as the expression
    /// `Ref::try_unwrap(this).ok()` does, can cause the strong count to
    /// drop to zero and the inner value of the `Ref` to be dropped.
    /// For instance, if two threads execute such an expression in parallel,
    /// there is a race condition without the possibility of unsafety:
    /// The threads could first both check whether they own the last instance
    /// in `Ref::try_unwrap`, determine that they both do not, and then both
    /// discard and drop their instance in the call to [`ok`][`Result::ok`].
    /// In this scenario, the value inside the `Ref` is safely destroyed
    /// by exactly one of the threads, but neither thread will ever be able
    /// to use the value.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x = Ref::new(3);
    /// assert_eq!(Ref::try_unwrap(x), Ok(3));
    ///
    /// let x = Ref::new(4);
    /// let _y = Ref::clone(&x);
    /// assert_eq!(*Ref::try_unwrap(x).unwrap_err(), 4);
    /// ```
    #[inline]
    pub fn try_unwrap(r: Ref<T, A>) -> Result<T, Ref<T, A>> {
        if r.ptr()
            .s
            .compare_exchange(1, 0, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return Err(r);
        }
        fence(Ordering::Acquire);
        let v = ManuallyDrop::new(r);
        let d = unsafe { read(&*v.ptr().v.get()) };
        drop(Weak { ptr: v.ptr, alloc: &v.alloc });
        Ok(d)
    }
}
impl<T, A: Allocator> Ref<[T], A> {
    /// Constructs a new atomically reference-counted slice with uninitialized
    /// contents in the provided allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let mut values = Ref::<[u32], _>::new_uninit_slice_in(3, System);
    ///
    /// let values = unsafe {
    ///     // Deferred initialization:
    ///     Ref::get_mut_unchecked(&mut values)[0].as_mut_ptr().write(1);
    ///     Ref::get_mut_unchecked(&mut values)[1].as_mut_ptr().write(2);
    ///     Ref::get_mut_unchecked(&mut values)[2].as_mut_ptr().write(3);
    ///
    ///     values.assume_init()
    /// };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[inline]
    pub fn new_uninit_slice_in(len: usize, alloc: A) -> Ref<[MaybeUninit<T>], A> {
        Ref {
            ptr: unsafe { NonNull::new_unchecked(uninit_slice(len, &alloc, false)) },
            alloc,
        }
    }
    /// Constructs a new atomically reference-counted slice with uninitialized
    /// contents, with the memory being filled with `0` bytes, in the
    /// provided allocator.
    ///
    /// See [`MaybeUninit::zeroed`][zeroed] for examples of correct and
    /// incorrect usage of this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let values = Ref::<[u32], _>::new_zeroed_slice_in(3, System);
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [0, 0, 0])
    /// ```
    ///
    /// [zeroed]: mem::MaybeUninit::zeroed
    #[inline]
    pub fn new_zeroed_slice_in(len: usize, alloc: A) -> Ref<[MaybeUninit<T>], A> {
        Ref {
            ptr: unsafe { NonNull::new_unchecked(uninit_slice(len, &alloc, false)) },
            alloc,
        }
    }
}
impl<T: ?Sized, A: Allocator> Ref<T, A> {
    /// Constructs an `Ref<T, A>` from a raw pointer.
    ///
    /// The raw pointer must have been previously returned by a call to [`Ref<U,
    /// A>::into_raw`][into_raw] with the following requirements:
    ///
    /// * If `U` is sized, it must have the same size and alignment as `T`. This
    ///   is trivially true if `U` is `T`.
    /// * If `U` is unsized, its data pointer must have the same size and
    ///   alignment as `T`. This is trivially true if `Ref<U>` was constructed
    ///   through `Ref<T>` and then converted to `Ref<U>` through an [unsized
    ///   coercion].
    ///
    /// Note that if `U` or `U`'s data pointer is not `T` but has the same size
    /// and alignment, this is basically like transmuting references of
    /// different types. See [`mem::transmute`][transmute] for more information
    /// on what restrictions apply in this case.
    ///
    /// The raw pointer must point to a block of memory allocated by `alloc`
    ///
    /// The user of `from_raw` has to make sure a specific value of `T` is only
    /// dropped once.
    ///
    /// This function is unsafe because improper use may lead to memory
    /// unsafety, even if the returned `Ref<T>` is never accessed.
    ///
    /// [into_raw]: Ref::into_raw
    /// [transmute]: core::mem::transmute
    /// [unsized coercion]: https://doc.rust-lang.org/reference/type-coercions.html#unsized-coercions
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let x = Ref::new_in("hello".to_owned(), System);
    /// let x_ptr = Ref::into_raw(x);
    ///
    /// unsafe {
    ///     // Convert back to an `Ref` to prevent leak.
    ///     let x = Ref::from_raw_in(x_ptr, System);
    ///     assert_eq!(&*x, "hello");
    ///
    ///     // Further calls to `Ref::from_raw(x_ptr)` would be memory-unsafe.
    /// }
    ///
    /// // The memory was freed when `x` went out of scope above, so `x_ptr` is now dangling!
    /// ```
    ///
    /// Convert a slice back into its original array:
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let x: Ref<[u32], _> = Ref::new_in([1, 2, 3], System);
    /// let x_ptr: *const [u32] = Ref::into_raw(x);
    ///
    /// unsafe {
    ///     let x: Ref<[u32; 3], _> = Ref::from_raw_in(x_ptr.cast::<[u32; 3]>(), System);
    ///     assert_eq!(&*x, &[1, 2, 3]);
    /// }
    /// ```
    #[inline]
    pub unsafe fn from_raw_in(ptr: *const T, alloc: A) -> Ref<T, A> {
        let v = Layout::new::<Reference<()>>();
        Ref {
            alloc,
            ptr: unsafe { NonNull::new_unchecked(ptr.byte_sub(v.size() + v.padding_needed_for(align_of_val_raw(ptr))) as *mut Reference<T>) },
        }
    }

    /// Provides a reference to the data.
    #[inline]
    pub fn as_ref(r: &Ref<T, A>) -> &T {
        unsafe { &*r.ptr().v.get() }
    }
    /// Returns a reference to the underlying allocator.
    ///
    /// Note: this is an associated function, which means that you have
    /// to call it as `Ref::allocator(&a)` instead of `a.allocator()`. This
    /// is so that there is no conflict with a method on the inner type.
    #[inline]
    pub fn allocator(r: &Ref<T, A>) -> &A {
        &r.alloc
    }
    /// Provides a mutable reference to the data.
    #[inline]
    pub fn as_mut(r: &Ref<T, A>) -> &mut T {
        unsafe { &mut *r.ptr().v.get() }
    }
    /// Provides a raw pointer to the data.
    ///
    /// The counts are not affected in any way and the `Ref` is not consumed.
    /// The pointer is valid for as long as there are strong counts in the
    /// `Ref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x = Ref::new("hello".to_owned());
    /// let y = Ref::clone(&x);
    /// let x_ptr = Ref::as_ptr(&x);
    /// assert_eq!(x_ptr, Ref::as_ptr(&y));
    /// assert_eq!(unsafe { &*x_ptr }, "hello");
    /// ```
    #[inline]
    pub fn as_ptr(r: &Ref<T, A>) -> *const T {
        unsafe { r.ptr.as_ref().v.get() }
    }
    /// Consumes the `Ref`, returning the wrapped pointer.
    ///
    /// To avoid a memory leak the pointer must be converted back to an `Ref`
    /// using [`Ref::from_raw`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let x = Ref::new("hello".to_owned());
    /// let x_ptr = Ref::into_raw(x);
    /// assert_eq!(unsafe { &*x_ptr }, "hello");
    /// # // Prevent leaks for Miri.
    /// # drop(unsafe { Ref::from_raw(x_ptr) });
    /// ```
    #[inline]
    pub fn into_raw(r: Ref<T, A>) -> *const T {
        Ref::as_ptr(&ManuallyDrop::new(r))
    }
    /// Gets the number of [`Weak`] pointers to this allocation.
    ///
    /// # Safety
    ///
    /// This method by itself is safe, but using it correctly requires extra
    /// care. Another thread can change the weak count at any time,
    /// including potentially between calling this method and acting on the
    /// result.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    /// let _weak_five = Ref::downgrade(&five);
    ///
    /// // This assertion is deterministic because we haven't shared
    /// // the `Ref` or `Weak` between threads.
    /// assert_eq!(1, Ref::weak_count(&five));
    /// ```
    #[inline]
    pub fn weak_count(r: &Ref<T, A>) -> usize {
        r.ptr().w.load(Ordering::Relaxed)
    }
    /// Gets the number of strong (`Ref`) pointers to this allocation.
    ///
    /// # Safety
    ///
    /// This method by itself is safe, but using it correctly requires extra
    /// care. Another thread can change the strong count at any time,
    /// including potentially between calling this method and acting on the
    /// result.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    /// let _also_five = Ref::clone(&five);
    ///
    /// // This assertion is deterministic because we haven't shared
    /// // the `Ref` between threads.
    /// assert_eq!(2, Ref::strong_count(&five));
    /// ```
    #[inline]
    pub fn strong_count(r: &Ref<T, A>) -> usize {
        r.ptr().s.load(Ordering::Relaxed)
    }
    /// Returns `true` if the two `Ref`s point to the same allocation in a vein
    /// similar to [`ptr::eq`]. This function ignores the metadata of  `dyn
    /// Trait` pointers.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    /// let same_five = Ref::clone(&five);
    /// let other_five = Ref::new(5);
    ///
    /// assert!(Ref::ptr_eq(&five, &same_five));
    /// assert!(!Ref::ptr_eq(&five, &other_five));
    /// ```
    ///
    /// [`ptr::eq`]: core::ptr::eq "ptr::eq"
    #[inline]
    pub fn ptr_eq(r: &Ref<T, A>, other: &Ref<T, A>) -> bool {
        addr_eq(r.ptr.as_ptr(), other.ptr.as_ptr())
    }
    /// Consumes the `Ref`, returning the wrapped pointer and allocator.
    ///
    /// To avoid a memory leak the pointer must be converted back to an `Ref`
    /// using [`Ref::from_raw_in`].
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use xrmt_stx::alloc::System;
    ///
    /// let x = Ref::new_in("hello".to_owned(), System);
    /// let (ptr, alloc) = Ref::into_raw_with_allocator(x);
    /// assert_eq!(unsafe { &*ptr }, "hello");
    /// let x = unsafe { Ref::from_raw_in(ptr, alloc) };
    /// assert_eq!(&*x, "hello");
    /// ```
    #[inline]
    pub fn into_raw_with_allocator(r: Ref<T, A>) -> (*const T, A) {
        let v = ManuallyDrop::new(r);
        (Ref::as_ptr(&v), unsafe { read(&v.alloc) })
    }

    #[inline]
    fn extract(r: Ref<T, A>) -> (NonNull<Reference<T>>, A) {
        let v = ManuallyDrop::new(r);
        (v.ptr, unsafe { read(&v.alloc) })
    }

    #[inline]
    fn ptr(&self) -> &Reference<T> {
        unsafe { self.ptr.as_ref() }
    }
}
impl<T: ?Sized, A: Allocator> Weak<T, A> {
    /// Converts a raw pointer previously created by [`into_raw`] back into
    /// `Weak<T>` in the provided allocator.
    ///
    /// This can be used to safely get a strong reference (by calling
    /// [`upgrade`] later) or to deallocate the weak count by dropping the
    /// `Weak<T>`.
    ///
    /// It takes ownership of one weak reference (with the exception of pointers
    /// created by [`new`], as these don't own anything; the method still
    /// works on them).
    ///
    /// # Safety
    ///
    /// The pointer must have originated from the [`into_raw`] and must still
    /// own its potential weak reference, and must point to a block of
    /// memory allocated by `alloc`.
    ///
    /// It is allowed for the strong count to be 0 at the time of calling this.
    /// Nevertheless, this takes ownership of one weak reference currently
    /// represented as a raw pointer (the weak count is not modified by this
    /// operation) and therefore it must be paired with a previous
    /// call to [`into_raw`].
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    ///
    /// let strong = Ref::new("hello".to_owned());
    ///
    /// let raw_1 = Ref::downgrade(&strong).into_raw();
    /// let raw_2 = Ref::downgrade(&strong).into_raw();
    ///
    /// assert_eq!(2, Ref::weak_count(&strong));
    ///
    /// assert_eq!("hello", &*unsafe { Weak::from_raw(raw_1) }.upgrade().unwrap());
    /// assert_eq!(1, Ref::weak_count(&strong));
    ///
    /// drop(strong);
    ///
    /// // Decrement the last weak count.
    /// assert!(unsafe { Weak::from_raw(raw_2) }.upgrade().is_none());
    /// ```
    ///
    /// [`new`]: Weak::new
    /// [`into_raw`]: Weak::into_raw
    /// [`upgrade`]: Weak::upgrade
    #[inline]
    pub unsafe fn from_raw_in(ptr: *const T, alloc: A) -> Weak<T, A> {
        let v = Layout::new::<Reference<()>>();
        Weak {
            alloc,
            ptr: unsafe { NonNull::new_unchecked(ptr.byte_sub(v.size() + v.padding_needed_for(align_of_val_raw(ptr))) as *mut Reference<T>) },
        }
    }

    /// Returns a reference to the underlying allocator.
    #[inline]
    pub fn allocator(&self) -> &A {
        &self.alloc
    }
    /// Returns a raw pointer to the object `T` pointed to by this `Weak<T>`.
    ///
    /// The pointer is valid only if there are some strong references. The
    /// pointer may be dangling, unaligned or even [`null`] otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    /// use core::ptr;
    ///
    /// let strong = Ref::new("hello".to_owned());
    /// let weak = Ref::downgrade(&strong);
    /// // Both point to the same object
    /// assert!(ptr::eq(&*strong, weak.as_ptr()));
    /// // The strong here keeps it alive, so we can still access the object.
    /// assert_eq!("hello", unsafe { &*weak.as_ptr() });
    ///
    /// drop(strong);
    /// // But not any more. We can do weak.as_ptr(), but accessing the pointer would lead to
    /// // undefined behavior.
    /// // assert_eq!("hello", unsafe { &*weak.as_ptr() });
    /// ```
    ///
    /// [`null`]: core::ptr::null "ptr::null"
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        unsafe { self.ptr.as_ref().v.get() }
    }
    /// Consumes the `Weak<T>` and turns it into a raw pointer.
    ///
    /// This converts the weak pointer into a raw pointer, while still
    /// preserving the ownership of one weak reference (the weak count is
    /// not modified by this operation). It can be turned back into the
    /// `Weak<T>` with [`from_raw`].
    ///
    /// The same restrictions of accessing the target of the pointer as with
    /// [`as_ptr`] apply.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    ///
    /// let strong = Ref::new("hello".to_owned());
    /// let weak = Ref::downgrade(&strong);
    /// let raw = weak.into_raw();
    ///
    /// assert_eq!(1, Ref::weak_count(&strong));
    /// assert_eq!("hello", unsafe { &*raw });
    ///
    /// drop(unsafe { Weak::from_raw(raw) });
    /// assert_eq!(0, Ref::weak_count(&strong));
    /// ```
    ///
    /// [`from_raw`]: Weak::from_raw
    /// [`as_ptr`]: Weak::as_ptr
    #[inline]
    pub fn into_raw(self) -> *const T {
        ManuallyDrop::new(self).as_ptr()
    }
    /// Gets an approximation of the number of `Weak` pointers pointing to this
    /// allocation.
    #[inline]
    pub fn weak_count(&self) -> usize {
        self.ptr().w.load(Ordering::Relaxed)
    }
    /// Gets the number of strong (`Ref`) pointers pointing to this allocation.
    #[inline]
    pub fn strong_count(&self) -> usize {
        self.ptr().s.load(Ordering::Relaxed)
    }
    /// Returns `true` if the two `Weak`s point to the same allocation similar
    /// to [`ptr::eq`], or if both don't point to any allocation (because
    /// they were created with `Weak::new()`). However, this function
    /// ignores the metadata of  `dyn Trait` pointers.
    ///
    /// # Notes
    ///
    /// Since this compares pointers it means that `Weak::new()` will equal each
    /// other, even though they don't point to any allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let first_rc = Ref::new(5);
    /// let first = Ref::downgrade(&first_rc);
    /// let second = Ref::downgrade(&first_rc);
    ///
    /// assert!(first.ptr_eq(&second));
    ///
    /// let third_rc = Ref::new(5);
    /// let third = Ref::downgrade(&third_rc);
    ///
    /// assert!(!first.ptr_eq(&third));
    /// ```
    ///
    /// Comparing `Weak::new`.
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    ///
    /// let first = Weak::new();
    /// let second = Weak::new();
    /// assert!(first.ptr_eq(&second));
    ///
    /// let third_rc = Ref::new(());
    /// let third = Ref::downgrade(&third_rc);
    /// assert!(!first.ptr_eq(&third));
    /// ```
    ///
    /// [`ptr::eq`]: core::ptr::eq "ptr::eq"
    #[inline]
    pub fn ptr_eq(&self, other: &Weak<T, A>) -> bool {
        addr_eq(self.ptr.as_ptr(), other.ptr.as_ptr())
    }
    /// Consumes the `Weak<T>`, returning the wrapped pointer and allocator.
    ///
    /// This converts the weak pointer into a raw pointer, while still
    /// preserving the ownership of one weak reference (the weak count is
    /// not modified by this operation). It can be turned back into the
    /// `Weak<T>` with [`from_raw_in`].
    ///
    /// The same restrictions of accessing the target of the pointer as with
    /// [`as_ptr`] apply.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    /// use xrmt_stx::alloc::System;
    ///
    /// let strong = Ref::new_in("hello".to_owned(), System);
    /// let weak = Ref::downgrade(&strong);
    /// let (raw, alloc) = weak.into_raw_with_allocator();
    ///
    /// assert_eq!(1, Ref::weak_count(&strong));
    /// assert_eq!("hello", unsafe { &*raw });
    ///
    /// drop(unsafe { Weak::from_raw_in(raw, alloc) });
    /// assert_eq!(0, Ref::weak_count(&strong));
    /// ```
    ///
    /// [`from_raw_in`]: Weak::from_raw_in
    /// [`as_ptr`]: Weak::as_ptr
    #[inline]
    pub fn into_raw_with_allocator(self) -> (*const T, A) {
        let v = ManuallyDrop::new(self);
        (v.as_ptr(), unsafe { read(&v.alloc) })
    }
    /// Attempt to gain access to the value contained in the [`Ref`] if it still
    /// exists. If it exists, the function in `f` will be called with the stored
    /// data value.
    ///
    /// This function temporarily increases the reference count to prevent
    /// deallocation during operation of `f`. Once complete, the reference count
    /// will be decremented. If no other strong references exist, this may cause
    /// the data to be deallocated.
    ///
    /// This function is perferred over doing a clone-than-use as this prevents
    /// an additional clone and allocation of the underlying allocator and will
    /// automatically clean up, even if a [`panic!`] occurs.
    ///
    /// [`panic!`]: core::panic!
    pub fn access<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        match self.ptr().s.fetch_update(Ordering::Acquire, Ordering::Relaxed, check) {
            Err(_) => None,
            Ok(_) => {
                // Decrement the counter even if a panic happens
                let mut v = Ref {
                    ptr:   self.ptr,
                    alloc: &self.alloc,
                };
                let r = f(v.as_mut());
                drop(v);
                Some(r)
            },
        }
    }

    #[inline]
    fn ptr(&self) -> ReferenceWeak<'_> {
        unsafe {
            ReferenceWeak {
                w: &self.ptr.as_ref().w,
                s: &self.ptr.as_ref().s,
            }
        }
    }
}
impl<T, A: Allocator> Ref<MaybeUninit<T>, A> {
    /// Converts to `Ref<T>`.
    ///
    /// # Safety
    ///
    /// As with [`MaybeUninit::assume_init`],
    /// it is up to the caller to guarantee that the inner value
    /// really is in an initialized state.
    /// Calling this when the content is not yet fully initialized
    /// causes immediate undefined behavior.
    ///
    /// [`MaybeUninit::assume_init`]: mem::MaybeUninit::assume_init
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let mut five = Ref::<u32>::new_uninit();
    ///
    /// // Deferred initialization:
    /// Ref::get_mut(&mut five).unwrap().write(5);
    ///
    /// let five = unsafe { five.assume_init() };
    ///
    /// assert_eq!(*five, 5)
    /// ```
    #[inline]
    pub unsafe fn assume_init(self) -> Ref<T, A> {
        let (v, a) = Ref::extract(self);
        Ref { ptr: v.cast(), alloc: a }
    }
}
impl<T, A: Allocator> Ref<[MaybeUninit<T>], A> {
    /// Converts to `Ref<[T]>`.
    ///
    /// # Safety
    ///
    /// As with [`MaybeUninit::assume_init`],
    /// it is up to the caller to guarantee that the inner value
    /// really is in an initialized state.
    /// Calling this when the content is not yet fully initialized
    /// causes immediate undefined behavior.
    ///
    /// [`MaybeUninit::assume_init`]: mem::MaybeUninit::assume_init
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let mut values = Ref::<[u32]>::new_uninit_slice(3);
    ///
    /// // Deferred initialization:
    /// let data = Ref::get_mut(&mut values).unwrap();
    /// data[0].write(1);
    /// data[1].write(2);
    /// data[2].write(3);
    ///
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    #[inline]
    pub unsafe fn assume_init(self) -> Ref<[T], A> {
        let (v, a) = Ref::extract(self);
        Ref {
            ptr:   unsafe { NonNull::new_unchecked(v.as_ptr() as _) },
            alloc: a,
        }
    }
}
impl<T: ?Sized, A: Allocator + Clone> Ref<T, A> {
    /// Creates a new [`Weak`] pointer to this allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    ///
    /// let weak_five = Ref::downgrade(&five);
    /// ```
    pub fn weak(r: &Ref<T, A>) -> Weak<T, A> {
        let i = unsafe { r.ptr.as_ref() };
        let mut v = i.w.load(Ordering::Relaxed);
        loop {
            v = match i.w.compare_exchange_weak(v, v + 1, Ordering::Acquire, Ordering::Relaxed) {
                Err(x) => x,
                Ok(_) => {
                    return Weak {
                        ptr:   r.ptr,
                        alloc: r.alloc.clone(),
                    }
                },
            };
            spin_loop();
        }
    }
    /// Creates a new [`Weak`] pointer to this allocation. But allows for Direct
    /// access instead of a "named" call.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    ///
    /// let weak_five = Ref::downgrade(&five);
    /// ```
    #[inline]
    pub fn _weak(&self) -> Weak<T, A> {
        Ref::weak(self)
    }
}
impl<T: ?Sized, A: Allocator + Clone> Weak<T, A> {
    /// Attempts to upgrade the `Weak` pointer to an [`Ref`], delaying
    /// dropping of the inner value if successful.
    ///
    /// Returns [`None`] if the inner value has since been dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    ///
    /// let weak_five = Ref::downgrade(&five);
    ///
    /// let strong_five: Option<Ref<_>> = weak_five.upgrade();
    /// assert!(strong_five.is_some());
    ///
    /// // Destroy all strong pointers.
    /// drop(strong_five);
    /// drop(five);
    ///
    /// assert!(weak_five.upgrade().is_none());
    /// ```
    #[inline]
    pub fn upgrade(&self) -> Option<Ref<T, A>> {
        self.ptr()
            .s
            .fetch_update(Ordering::Acquire, Ordering::Relaxed, check)
            .ok()
            .map(|_| Ref {
                ptr:   self.ptr,
                alloc: self.alloc.clone(),
            })
    }
}

impl<T: ?Sized + Default> Default for Ref<T> {
    #[inline]
    fn default() -> Ref<T> {
        Ref::new(T::default())
    }
}
impl<T: ?Sized, A: Allocator> Drop for Ref<T, A> {
    #[inline]
    fn drop(&mut self) {
        if self.ptr().s.fetch_sub(1, Ordering::Release) == 1 {
            drop_ref(self)
        }
    }
}
impl<T: ?Sized, A: Allocator> Deref for Ref<T, A> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.ptr().v.get() }
    }
}
impl<T: ?Sized, A: Allocator> DerefMut for Ref<T, A> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr().v.get() }
    }
}
impl<T: ?Sized, A: Allocator> AsRef<T> for Ref<T, A> {
    #[inline]
    fn as_ref(&self) -> &T {
        unsafe { &*self.ptr().v.get() }
    }
}
impl<T: ?Sized, A: Allocator> AsMut<T> for Ref<T, A> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr().v.get() }
    }
}
impl<T: ?Sized, A: Allocator> Borrow<T> for Ref<T, A> {
    #[inline]
    fn borrow(&self) -> &T {
        unsafe { &*self.ptr().v.get() }
    }
}
impl<T: ?Sized, A: Allocator + Clone> Clone for Ref<T, A> {
    /// Makes a clone of the `Ref` pointer.
    ///
    /// This creates another pointer to the same allocation, increasing the
    /// strong reference count.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::Ref;
    ///
    /// let five = Ref::new(5);
    ///
    /// let _ = Ref::clone(&five);
    /// ```
    #[inline]
    fn clone(&self) -> Ref<T, A> {
        let v = self.ptr().s.fetch_add(1, Ordering::Relaxed);
        if unlikely(v > isize::MAX as usize) {
            abort!();
        }
        Ref {
            ptr:   self.ptr,
            alloc: self.alloc.clone(),
        }
    }
}

impl<T: ?Sized, A: Allocator> Drop for Weak<T, A> {
    #[inline]
    fn drop(&mut self) {
        if self.ptr().w.fetch_sub(1, Ordering::Release) == 1 {
            unsafe {
                self.alloc
                    .deallocate(self.ptr.cast(), Layout::for_value_raw(self.ptr.as_ptr()))
            }
        }
    }
}
impl<T: ?Sized, A: Allocator + Clone> Clone for Weak<T, A> {
    /// Makes a clone of the `Weak` pointer that points to the same allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use xrmt_stx::sync::extra::{Ref, Weak};
    ///
    /// let weak_five = Ref::downgrade(&Ref::new(5));
    ///
    /// let _ = Weak::clone(&weak_five);
    /// ```
    #[inline]
    fn clone(&self) -> Weak<T, A> {
        let v = self.ptr().w.fetch_add(1, Ordering::Relaxed);
        if unlikely(v > isize::MAX as usize) {
            abort!();
        }
        Weak {
            ptr:   self.ptr,
            alloc: self.alloc.clone(),
        }
    }
}

impl<T> From<T> for Ref<T> {
    #[inline]
    fn from(v: T) -> Ref<T> {
        Ref::new(v)
    }
}

unsafe impl<T: ?Sized + Sync + Send, A: Allocator + Send> Send for Ref<T, A> {}
unsafe impl<T: ?Sized + Sync + Send, A: Allocator + Sync> Sync for Ref<T, A> {}

#[inline]
fn check(v: usize) -> Option<usize> {
    if v == 0 {
        None
    } else {
        Some(v + 1)
    }
}
#[cold]
#[inline(never)]
fn drop_ref<T: ?Sized, A: Allocator>(r: &mut Ref<T, A>) {
    let v = Weak { ptr: r.ptr, alloc: &r.alloc };
    unsafe { drop_in_place(&mut (*r.ptr.as_ptr()).v) };
    drop(v)
}
fn uninit<T, A: Allocator>(alloc: &A, zero: bool) -> *mut Reference<T> {
    let v = abort_unlikely!(Layout::new::<Reference<()>>().extend(Layout::new::<T>()))
        .0
        .pad_to_align();
    let e: *mut Reference<T> = abort_unlikely!(if zero {
        alloc.allocate_zeroed(v)
    } else {
        alloc.allocate(v)
    })
    .cast()
    .as_ptr();
    unsafe {
        (&raw mut (*e).s).write(AtomicUsize::new(1));
        (&raw mut (*e).w).write(AtomicUsize::new(1));
    }
    e
}
fn uninit_slice<T, A: Allocator>(len: usize, alloc: A, zero: bool) -> *mut Reference<[T]> {
    let v = abort_unlikely!(Layout::new::<Reference<()>>().extend(abort_unlikely!(Layout::array::<T>(len))))
        .0
        .pad_to_align();
    let e = slice_from_raw_parts_mut(
        abort_unlikely!(if zero {
            alloc.allocate_zeroed(v)
        } else {
            alloc.allocate(v)
        })
        .cast::<T>()
        .as_ptr(),
        len,
    ) as *mut Reference<[T]>;
    unsafe {
        (&raw mut (*e).s).write(AtomicUsize::new(1));
        (&raw mut (*e).w).write(AtomicUsize::new(1));
    }
    e
}
