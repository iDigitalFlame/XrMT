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
#![cfg(all(target_family = "windows", not(feature = "std")))]

extern crate alloc;
extern crate core;

extern crate xrmt_winapi;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::sync::Arc;
use core::clone::Clone;
use core::cmp::{Eq, PartialEq};
use core::convert::{AsRef, From, Into, TryFrom};
use core::error::Error;
use core::fmt::{Debug, Display, Formatter};
use core::marker::{Copy, PhantomData, Send, Sync};
use core::mem::{transmute, ManuallyDrop};
use core::option::Option::{self, None};
use core::result::Result::{self, Err, Ok};

use xrmt_winapi::functions::{is_terminal, WSADuplicateSocket};
use xrmt_winapi::structs;

use crate::io::{FmtResult, IoResult, IsTerminal};
use crate::net::{TcpListener, TcpStream, UdpSocket};
use crate::os::windows::raw::{HANDLE, SOCKET};
use crate::os::Handle;

/// A borrowed handle.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the handle.
///
/// This uses `repr(transparent)` and has the representation of a host handle,
/// so it can be used in FFI in places where a handle is passed as an argument,
/// it is not captured or consumed.
///
/// Note that it *may* have the value `-1`, which in `BorrowedHandle` always
/// represents a valid handle value, such as [the current process handle], and
/// not `INVALID_HANDLE_VALUE`, despite the two having the same value. See
/// [here] for the full story.
///
/// And, it *may* have the value `NULL` (0), which can occur when consoles are
/// detached from processes, or when `windows_subsystem` is used.
///
/// This type's `.to_owned()` implementation returns another `BorrowedHandle`
/// rather than an `OwnedHandle`. It just makes a trivial copy of the raw
/// handle, which is then borrowed under the same lifetime.
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
/// [the current process handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
#[repr(transparent)]
pub struct BorrowedHandle<'a> {
    h:  Handle,
    _p: PhantomData<&'a Handle>,
}
/// A borrowed socket.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the socket.
///
/// This uses `repr(transparent)` and has the representation of a host socket,
/// so it can be used in FFI in places where a socket is passed as an argument,
/// it is not captured or consumed, and it never has the value
/// `INVALID_SOCKET`.
///
/// This type's `.to_owned()` implementation returns another `BorrowedSocket`
/// rather than an `OwnedSocket`. It just makes a trivial copy of the raw
/// socket, which is then borrowed under the same lifetime.
#[repr(transparent)]
pub struct BorrowedSocket<'a> {
    h:  Handle,
    _p: PhantomData<&'a Handle>,
}
/// This is the error type used by [`HandleOrNull`] when attempting to convert
/// into a handle, to indicate that the value is null.
pub struct NullHandleError(());
/// FFI type for handles in return values or out parameters, where `NULL` is
/// used as a sentry value to indicate errors, such as in the return value of
/// `CreateThread`. This uses `repr(transparent)` and has the representation of
/// a host handle, so that it can be used in such FFI declarations.
///
/// The only thing you can usefully do with a `HandleOrNull` is to convert it
/// into an `OwnedHandle` using its [`TryFrom`] implementation; this conversion
/// takes care of the check for `NULL`. This ensures that such FFI calls cannot
/// start using the handle without checking for `NULL` first.
///
/// This type may hold any handle value that [`OwnedHandle`] may hold. As with
/// `OwnedHandle`, when it holds `-1`, that value is interpreted as a valid
/// handle value, such as [the current process handle], and not
/// `INVALID_HANDLE_VALUE`.
///
/// If this holds a non-null handle, it will close the handle on drop.
///
/// [the current process handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
/// This is the error type used by [`HandleOrInvalid`] when attempting to
/// convert into a handle, to indicate that the value is
/// `INVALID_HANDLE_VALUE`.
pub struct InvalidHandleError(());
#[repr(transparent)]
pub struct HandleOrNull(OwnedHandle);
/// FFI type for handles in return values or out parameters, where
/// `INVALID_HANDLE_VALUE` is used as a sentry value to indicate errors, such as
/// in the return value of `CreateFileW`. This uses `repr(transparent)` and has
/// the representation of a host handle, so that it can be used in such
/// FFI declarations.
///
/// The only thing you can usefully do with a `HandleOrInvalid` is to convert it
/// into an `OwnedHandle` using its [`TryFrom`] implementation; this conversion
/// takes care of the check for `INVALID_HANDLE_VALUE`. This ensures that such
/// FFI calls cannot start using the handle without checking for
/// `INVALID_HANDLE_VALUE` first.
///
/// This type may hold any handle value that [`OwnedHandle`] may hold, except
/// that when it holds `-1`, that value is interpreted to mean
/// `INVALID_HANDLE_VALUE`.
///
/// If holds a handle other than `INVALID_HANDLE_VALUE`, it will close the
/// handle on drop.
#[repr(transparent)]
pub struct HandleOrInvalid(OwnedHandle);

/// A trait to borrow the handle from an underlying object.
pub trait AsHandle {
    /// Borrows the handle.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xrmt_stx::fs::File;
    /// # use xrmt_stx::io::{self, IoResult};
    /// use xrmt_stx::os::windows::io::{AsHandle, BorrowedHandle};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// let borrowed_handle: BorrowedHandle<'_> = f.as_handle();
    /// # Ok::<(), io::Error>(())
    /// ```
    fn as_handle(&self) -> BorrowedHandle<'_>;
}
/// A trait to borrow the socket from an underlying object.
pub trait AsSocket {
    /// Borrows the socket.
    fn as_socket(&self) -> BorrowedSocket<'_>;
}
/// Extracts raw handles.
pub trait AsRawHandle {
    /// Extracts the raw handle.
    ///
    /// This function is typically used to **borrow** an owned handle.
    /// When used in this way, this method does **not** pass ownership of the
    /// raw handle to the caller, and the handle is only guaranteed
    /// to be valid while the original object has not yet been destroyed.
    ///
    /// This function may return null, such as when called on [`Stdin`],
    /// [`Stdout`], or [`Stderr`] when the console is detached.
    ///
    /// However, borrowing is not strictly required. See [`AsHandle::as_handle`]
    /// for an API which strictly borrows a handle.
    ///
    /// [`Stdin`]: crate::io::Stdin
    /// [`Stdout`]: crate::io::Stdout
    /// [`Stderr`]: crate::io::Stderr
    fn as_raw_handle(&self) -> RawHandle;
}
/// Extracts raw sockets.
pub trait AsRawSocket {
    /// Extracts the raw socket.
    ///
    /// This function is typically used to **borrow** an owned socket.
    /// When used in this way, this method does **not** pass ownership of the
    /// raw socket to the caller, and the socket is only guaranteed
    /// to be valid while the original object has not yet been destroyed.
    ///
    /// However, borrowing is not strictly required. See [`AsSocket::as_socket`]
    /// for an API which strictly borrows a socket.
    fn as_raw_socket(&self) -> RawSocket;
}
/// A trait to express the ability to consume an object and acquire ownership of
/// its raw `HANDLE`.
pub trait IntoRawHandle {
    /// Consumes this object, returning the raw underlying handle.
    ///
    /// This function is typically used to **transfer ownership** of the
    /// underlying handle to the caller. When used in this way, callers are
    /// then the unique owners of the handle and must close it once it's no
    /// longer needed.
    ///
    /// However, transferring ownership is not strictly required. Use a
    /// `Into<OwnedHandle>::into` implementation for an API which strictly
    /// transfers ownership.
    fn into_raw_handle(self) -> RawHandle;
}
/// Constructs I/O objects from raw handles.
pub trait FromRawHandle {
    /// Constructs a new I/O object from the specified raw handle.
    ///
    /// This function is typically used to **consume ownership** of the handle
    /// given, passing responsibility for closing the handle to the returned
    /// object. When used in this way, the returned object
    /// will take responsibility for closing it when the object goes out of
    /// scope.
    ///
    /// However, consuming ownership is not strictly required. Use a
    /// `From<OwnedHandle>::from` implementation for an API which strictly
    /// consumes ownership.
    ///
    /// # Safety
    ///
    /// The `handle` passed in must:
    ///   - be an [owned handle][io-safety]; in particular, it must be open.
    ///   - be a handle for a resource that may be freed via [`CloseHandle`] (as
    ///     opposed to `RegCloseKey` or other close functions).
    ///
    /// Note that the handle *may* have the value `INVALID_HANDLE_VALUE` (-1),
    /// which is sometimes a valid handle value. See [here] for the full story.
    ///
    /// [`CloseHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    /// [io-safety]: crate::io#io-safety
    unsafe fn from_raw_handle(handle: RawHandle) -> Self;
}
/// A trait to express the ability to consume an object and acquire ownership of
/// its raw `SOCKET`.
pub trait IntoRawSocket {
    /// Consumes this object, returning the raw underlying socket.
    ///
    /// This function is typically used to **transfer ownership** of the
    /// underlying socket to the caller. When used in this way, callers are
    /// then the unique owners of the socket and must close it once it's no
    /// longer needed.
    ///
    /// However, transferring ownership is not strictly required. Use a
    /// `Into<OwnedSocket>::into` implementation for an API which strictly
    /// transfers ownership.
    fn into_raw_socket(self) -> RawSocket;
}
/// Creates I/O objects from raw sockets.
pub trait FromRawSocket {
    /// Constructs a new I/O object from the specified raw socket.
    ///
    /// This function is typically used to **consume ownership** of the socket
    /// given, passing responsibility for closing the socket to the returned
    /// object. When used in this way, the returned object
    /// will take responsibility for closing it when the object goes out of
    /// scope.
    ///
    /// However, consuming ownership is not strictly required. Use a
    /// `From<OwnedSocket>::from` implementation for an API which strictly
    /// consumes ownership.
    ///
    /// # Safety
    ///
    /// The `socket` passed in must:
    ///   - be an [owned socket][io-safety]; in particular, it must be open.
    ///   - be a socket that may be freed via [`closesocket`].
    ///
    /// [`closesocket`]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-closesocket
    /// [io-safety]: crate::io#io-safety
    unsafe fn from_raw_socket(sock: RawSocket) -> Self;
}

/// Raw HANDLEs.
pub type RawHandle = HANDLE;
/// Raw SOCKETs.
pub type RawSocket = SOCKET;
/// An owned handle.
///
/// This closes the handle on drop.
///
/// Note that it *may* have the value `-1`, which in `OwnedHandle` always
/// represents a valid handle value, such as [the current process handle], and
/// not `INVALID_HANDLE_VALUE`, despite the two having the same value. See
/// [here] for the full story.
///
/// And, it *may* have the value `NULL` (0), which can occur when consoles are
/// detached from processes, or when `windows_subsystem` is used.
///
/// `OwnedHandle` uses [`CloseHandle`] to close its handle on drop. As such,
/// it must not be used with handles to open registry keys which need to be
/// closed with [`RegCloseKey`] instead.
///
/// [`CloseHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
/// [`RegCloseKey`]: https://docs.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regclosekey
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
/// [the current process handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
pub type OwnedHandle = structs::OwnedHandle;
pub type OwnedSocket = structs::OwnedSocket;

impl HandleOrNull {
    /// Constructs a new instance of `Self` from the given `RawHandle` returned
    /// from a Windows API that uses null to indicate failure, such as
    /// `CreateThread`.
    ///
    /// Use `HandleOrInvalid` instead of `HandleOrNull` for APIs that
    /// use `INVALID_HANDLE_VALUE` to indicate failure.
    ///
    /// # Safety
    ///
    /// The passed `handle` value must either satisfy the safety requirements
    /// of [`FromRawHandle::from_raw_handle`], or be null. Note that not all
    /// Windows APIs use null for errors; see [here] for the full story.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub unsafe fn from_raw_handle(v: RawHandle) -> HandleOrNull {
        HandleOrNull(v.into())
    }
}
impl HandleOrInvalid {
    /// Constructs a new instance of `Self` from the given `RawHandle` returned
    /// from a Windows API that uses `INVALID_HANDLE_VALUE` to indicate
    /// failure, such as `CreateFileW`.
    ///
    /// Use `HandleOrNull` instead of `HandleOrInvalid` for APIs that
    /// use null to indicate failure.
    ///
    /// # Safety
    ///
    /// The passed `handle` value must either satisfy the safety requirements
    /// of [`FromRawHandle::from_raw_handle`], or be
    /// `INVALID_HANDLE_VALUE` (-1). Note that not all Windows APIs use
    /// `INVALID_HANDLE_VALUE` for errors; see [here] for the full story.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub unsafe fn from_raw_handle(v: RawHandle) -> HandleOrInvalid {
        HandleOrInvalid(v.into())
    }
}
impl<'a> BorrowedHandle<'a> {
    /// Returns a `BorrowedHandle` holding the given raw handle.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `handle` must be a valid open handle, it
    /// must remain open for the duration of the returned `BorrowedHandle`.
    ///
    /// Note that it *may* have the value `INVALID_HANDLE_VALUE` (-1), which is
    /// sometimes a valid handle value. See [here] for the full story.
    ///
    /// And, it *may* have the value `NULL` (0), which can occur when consoles
    /// are detached from processes, or when `windows_subsystem` is used.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub const unsafe fn borrow_raw(v: RawHandle) -> BorrowedHandle<'a> {
        BorrowedHandle { h: v, _p: PhantomData }
    }

    /// Creates a new `OwnedHandle` instance that shares the same underlying
    /// object as the existing `BorrowedHandle` instance.
    #[inline]
    pub fn try_clone_to_owned(&self) -> IoResult<OwnedHandle> {
        Ok(self.h.duplicate()?)
    }
}
impl<'a> BorrowedSocket<'a> {
    /// Returns a `BorrowedHandle` holding the given raw handle.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `handle` must be a valid open handle, it
    /// must remain open for the duration of the returned `BorrowedHandle`.
    ///
    /// Note that it *may* have the value `INVALID_HANDLE_VALUE` (-1), which is
    /// sometimes a valid handle value. See [here] for the full story.
    ///
    /// And, it *may* have the value `NULL` (0), which can occur when consoles
    /// are detached from processes, or when `windows_subsystem` is used.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub const unsafe fn borrow_raw(v: RawSocket) -> BorrowedSocket<'a> {
        BorrowedSocket { h: v, _p: PhantomData }
    }

    /// Creates a new `OwnedHandle` instance that shares the same underlying
    /// object as the existing `BorrowedHandle` instance.
    #[inline]
    pub fn try_clone_to_owned(&self) -> IoResult<OwnedSocket> {
        Ok(WSADuplicateSocket(unsafe { transmute(self) })?)
    }
}

impl Copy for BorrowedHandle<'_> {}
impl<'a> Clone for BorrowedHandle<'a> {
    #[inline]
    fn clone(&self) -> BorrowedHandle<'a> {
        BorrowedHandle { h: self.h, _p: PhantomData }
    }
}
impl IsTerminal for BorrowedHandle<'_> {
    #[inline]
    fn is_terminal(&self) -> bool {
        is_terminal(self.h)
    }
}
impl AsRef<Handle> for BorrowedHandle<'_> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.h
    }
}

impl Copy for BorrowedSocket<'_> {}
impl AsSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        *self
    }
}
impl<'a> Clone for BorrowedSocket<'a> {
    #[inline]
    fn clone(&self) -> BorrowedSocket<'a> {
        BorrowedSocket { h: self.h, _p: PhantomData }
    }
}
impl AsRawSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.h
    }
}
impl AsRef<Handle> for BorrowedSocket<'_> {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.h
    }
}

impl TryFrom<HandleOrNull> for OwnedHandle {
    type Error = NullHandleError;

    #[inline]
    fn try_from(v: HandleOrNull) -> Result<OwnedHandle, NullHandleError> {
        if !v.0.is_invalid() {
            Ok(v.0)
        } else {
            Err(NullHandleError(()))
        }
    }
}
impl TryFrom<HandleOrInvalid> for OwnedHandle {
    type Error = InvalidHandleError;

    #[inline]
    fn try_from(v: HandleOrInvalid) -> Result<OwnedHandle, InvalidHandleError> {
        if !v.0.is_invalid() {
            Ok(v.0)
        } else {
            Err(InvalidHandleError(()))
        }
    }
}

impl Eq for NullHandleError {}
impl Copy for NullHandleError {}
impl Clone for NullHandleError {
    #[inline]
    fn clone(&self) -> NullHandleError {
        NullHandleError(())
    }
}
impl Debug for NullHandleError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("NullHandleError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Error for NullHandleError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for NullHandleError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
impl PartialEq for NullHandleError {
    #[inline]
    fn eq(&self, _other: &NullHandleError) -> bool {
        true
    }
}

impl Eq for InvalidHandleError {}
impl Copy for InvalidHandleError {}
impl Clone for InvalidHandleError {
    #[inline]
    fn clone(&self) -> InvalidHandleError {
        InvalidHandleError(())
    }
}
impl Debug for InvalidHandleError {
    #[cfg(not(feature = "strip"))]
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("InvalidHandleError")
    }
    #[cfg(feature = "strip")]
    #[inline]
    fn fmt(&self, _f: &mut Formatter<'_>) -> FmtResult {
        Ok(())
    }
}
impl Error for InvalidHandleError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
impl Display for InvalidHandleError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
    }
}
impl PartialEq for InvalidHandleError {
    #[inline]
    fn eq(&self, _other: &InvalidHandleError) -> bool {
        true
    }
}

impl AsSocket for UdpSocket {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        BorrowedSocket {
            h:  **self.as_ref(),
            _p: PhantomData,
        }
    }
}
impl AsSocket for TcpStream {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        BorrowedSocket {
            h:  **self.as_ref(),
            _p: PhantomData,
        }
    }
}
impl AsSocket for OwnedSocket {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        BorrowedSocket { h: **self, _p: PhantomData }
    }
}
impl<T: AsSocket> AsSocket for &T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}
impl<T: AsSocket> AsSocket for Rc<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.as_ref().as_socket()
    }
}
impl<T: AsSocket> AsSocket for Arc<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.as_ref().as_socket()
    }
}
impl<T: AsSocket> AsSocket for Box<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.as_ref().as_socket()
    }
}
impl<T: AsSocket> AsSocket for &mut T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

impl AsRawSocket for UdpSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        **self.as_ref()
    }
}
impl AsRawSocket for TcpStream {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        **self.as_ref()
    }
}
impl AsRawSocket for OwnedSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        **self
    }
}
impl AsRawSocket for TcpListener {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        **self.as_ref()
    }
}

impl IntoRawSocket for UdpSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        // Prevent the Handle from being dropped.
        let v = ManuallyDrop::new(self);
        **v.as_ref()
    }
}
impl IntoRawSocket for TcpStream {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        // Prevent the Handle from being dropped.
        let v = ManuallyDrop::new(self);
        **v.as_ref()
    }
}
impl IntoRawSocket for OwnedSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        // Prevent the Handle from being dropped.
        let v = ManuallyDrop::new(self);
        **v
    }
}
impl IntoRawSocket for TcpListener {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        // Prevent the Handle from being dropped.
        let v = ManuallyDrop::new(self);
        **v.as_ref()
    }
}

impl FromRawSocket for UdpSocket {
    #[inline]
    unsafe fn from_raw_socket(v: RawSocket) -> UdpSocket {
        structs::OwnedSocket::from(v).into()
    }
}
impl FromRawSocket for TcpStream {
    #[inline]
    unsafe fn from_raw_socket(v: RawSocket) -> TcpStream {
        structs::OwnedSocket::from(v).into()
    }
}
impl FromRawSocket for OwnedSocket {
    #[inline]
    unsafe fn from_raw_socket(v: RawSocket) -> OwnedSocket {
        structs::OwnedSocket::from(v).into()
    }
}
impl FromRawSocket for TcpListener {
    #[inline]
    unsafe fn from_raw_socket(v: RawSocket) -> TcpListener {
        structs::OwnedSocket::from(v).into()
    }
}

impl<T: AsRef<Handle>> AsHandle for T {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        BorrowedHandle {
            h:  *self.as_ref(),
            _p: PhantomData,
        }
    }
}
impl<T: AsRef<Handle>> AsRawHandle for T {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        *self.as_ref()
    }
}
impl<T: Into<OwnedHandle>> IntoRawHandle for T {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        unsafe { Handle::take(self.into()) }
    }
}
impl<T: From<OwnedHandle>> FromRawHandle for T {
    #[inline]
    unsafe fn from_raw_handle(v: RawHandle) -> T {
        structs::OwnedHandle::from(v).into()
    }
}

unsafe impl Send for HandleOrNull {}
unsafe impl Sync for HandleOrNull {}

unsafe impl Send for HandleOrInvalid {}
unsafe impl Sync for HandleOrInvalid {}

unsafe impl Send for BorrowedHandle<'_> {}
unsafe impl Sync for BorrowedHandle<'_> {}

unsafe impl Send for BorrowedSocket<'_> {}
unsafe impl Sync for BorrowedSocket<'_> {}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Formatter, Result};
    use core::write;

    use crate::os::windows::io::{BorrowedHandle, BorrowedSocket};

    impl Debug for BorrowedHandle<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "BorrowedHandle: 0x{:X}", self.h)
        }
    }
    impl Debug for BorrowedSocket<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            write!(f, "BorrowedSocket: 0x{:X}", self.h)
        }
    }
}
