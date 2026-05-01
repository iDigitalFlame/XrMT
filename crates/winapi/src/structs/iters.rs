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
#![cfg(target_family = "windows")]

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::string::String;
use alloc::vec::Vec;
use core::clone::Clone;
use core::convert::{From, Into};
use core::iter::{ExactSizeIterator, FusedIterator, IntoIterator, Iterator};
use core::marker::PhantomData;
use core::net::SocketAddr;
use core::option::Option::{self, None, Some};
use core::ptr::null_mut;
use core::result::Result::{Err, Ok};
use core::slice::{from_raw_parts, Iter};

use xrmt_data::text::utf16_to_string;

use crate::functions::{privilege_accquire, privilege_release, process_cmdline, process_protection, process_user, system_root, NtEnumerateKey, NtEnumerateValueKey, NtQueryKeyInfo, OpenProcess, OpenThread};
use crate::structs::{Adapter, HandleEntry, HandleEntryEx, Key, ModuleEntry, MulticastAddress, OwnedHandle, PoolTagEntry, Privilege, ProcessEntry, RegKeyBasicInfo, RegValueFullInfo, StringLike, UnicastAddress};
use crate::{iphlpapi, ntdll, syscall, Win32Error, Win32Result};

pub struct Module {
    pub path:       String,
    pub size:       u32,
    pub order:      u16,
    pub flags:      u32,
    pub address:    usize,
    pub load_count: u16,
}
pub struct Thread {
    pub tid:    u32,
    pub pid:    u32,
    pub status: u8,
}
pub struct Process {
    pub pid:     u32,
    pub ppid:    u32,
    pub user:    String,
    pub cmdline: String,
    pub threads: u32,
    pub session: u32,
}
pub struct DnsIter<'a> {
    base: &'a Adapter<'a>,
    cur:  Option<&'a MulticastAddress<'a>>,
}
pub struct ModuleIter<'a> {
    v:    Iter<'a, ModuleEntry>,
    _buf: Vec<u8>,
}
pub struct HandleIter<'a> {
    v:    Iter<'a, HandleEntry>,
    _buf: Vec<u8>,
}
pub struct RegKeyIter<'a> {
    pos: u32,
    key: Key,
    buf: Vec<u8>,
    max: u32,
    _p:  PhantomData<&'a [u8]>,
}
pub struct UnicastIter<'a> {
    base: &'a Adapter<'a>,
    cur:  Option<&'a UnicastAddress<'a>>,
}
pub struct AdapterIter<'a> {
    base: &'a Adapter<'a>,
    cur:  Option<&'a Adapter<'a>>,
    _buf: Vec<u8>,
}
pub struct PoolTagIter<'a> {
    v:    Iter<'a, PoolTagEntry>,
    _buf: Vec<u8>,
}
pub struct ProcessIter<'a> {
    buf:  Vec<u8>,
    pos:  usize,
    next: usize,
    _p:   PhantomData<&'a [u8]>,
}
pub struct HandleExIter<'a> {
    v:    Iter<'a, HandleEntryEx>,
    _buf: Vec<u8>,
}
pub struct RegValueIter<'a> {
    pos: u32,
    key: Key,
    buf: Vec<u8>,
    max: u32,
    _p:  PhantomData<&'a [u8]>,
}
pub struct AddressIter<'a>(UnicastIter<'a>);

#[repr(C)]
struct ModulesList {
    modules:      u32,
    module_array: [ModuleEntry; 1],
}
#[repr(C)]
struct HandlesList {
    handle_count: u32,
    handles:      [HandleEntry; 1],
}
#[repr(C)]
struct PoolTagList {
    tag_count: u32,
    tags:      [PoolTagEntry; 1],
}
#[repr(C)]
struct HandlesListEx {
    handle_count: usize,
    pad:          usize,
    handles:      [HandleEntryEx; 1],
}

impl Thread {
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        OpenThread(access, false, self.tid)
    }
}
impl Process {
    #[inline]
    pub fn handle(&self, access: u32) -> Win32Result<OwnedHandle> {
        OpenProcess(access, false, self.pid)
    }
}
impl<'a> Adapter<'a> {
    #[inline]
    pub fn dns(&'a self) -> DnsIter<'a> {
        DnsIter { base: self, cur: None }
    }
    #[inline]
    pub fn unicast(&'a self) -> UnicastIter<'a> {
        UnicastIter { base: self, cur: None }
    }
    #[inline]
    pub fn addresses(&'a self) -> AddressIter<'a> {
        AddressIter(self.unicast())
    }
}
impl<'a> ModuleIter<'a> {
    #[inline]
    pub fn new() -> Win32Result<ModuleIter<'a>> {
        // 0xB - SystemModuleInformation
        let b = query(0xB, true)?;
        let e = unsafe { &*(b.as_ptr() as *const ModulesList) };
        let v = unsafe { from_raw_parts(e.module_array.as_ptr(), e.modules as usize) };
        Ok(ModuleIter { v: v.iter(), _buf: b })
    }

    #[inline]
    pub fn to_vec(self) -> Vec<Module> {
        let mut v = Vec::with_capacity(self.v.len());
        let r = system_root();
        for i in self {
            v.push(i.module(&r));
        }
        v
    }
}
impl<'a> HandleIter<'a> {
    #[inline]
    pub fn new() -> Win32Result<HandleIter<'a>> {
        // 0x10 - SystemHandleInformation
        let b = query(0x10, false)?;
        let e = unsafe { &*(b.as_ptr() as *const HandlesList) };
        let v = unsafe { from_raw_parts(e.handles.as_ptr(), e.handle_count as usize) };
        Ok(HandleIter { v: v.iter(), _buf: b })
    }
    #[inline]
    pub fn new_extended() -> Win32Result<HandleExIter<'a>> {
        // 0x40 - SystemExtendedHandleInformation
        let b = query(0x40, false)?;
        let e = unsafe { &*(b.as_ptr() as *const HandlesListEx) };
        let v = unsafe { from_raw_parts(e.handles.as_ptr(), e.handle_count as usize) };
        Ok(HandleExIter { v: v.iter(), _buf: b })
    }
}
impl<'a> RegKeyIter<'a> {
    #[inline]
    pub fn new(key: Key) -> Win32Result<RegKeyIter<'a>> {
        let mut b = Vec::new();
        let i = NtQueryKeyInfo(key, &mut b)?;
        Ok(RegKeyIter {
            pos: 0u32,
            key,
            max: i.subkeys,
            buf: b,
            _p: PhantomData,
        })
    }

    #[inline]
    pub fn to_names(self) -> Win32Result<Vec<String>> {
        let mut r = Vec::with_capacity(self.max as usize);
        for i in self {
            r.push(utf16_to_string(i?.as_slice()));
        }
        Ok(r)
    }
}
impl<'a> AdapterIter<'a> {
    #[inline]
    pub fn new(family: u32, flags: u32) -> Win32Result<AdapterIter<'a>> {
        let v = adapters(family, flags)?;
        let a = unsafe { &*(v.as_ptr() as *const Adapter) };
        Ok(AdapterIter { base: a, cur: None, _buf: v })
    }
}
impl<'a> ProcessIter<'a> {
    #[inline]
    pub fn new() -> Win32Result<ProcessIter<'a>> {
        // 0x5 - SystemProcessInformation
        Ok(ProcessIter {
            buf:  query(0x5, false)?,
            pos:  0usize,
            next: 0usize,
            _p:   PhantomData,
        })
    }

    #[inline]
    pub fn to_vec(self) -> Vec<Process> {
        let mut v = Vec::with_capacity(64);
        for i in self {
            v.push(i.into());
        }
        v
    }
}
impl<'a> PoolTagIter<'a> {
    #[inline]
    pub fn new() -> Win32Result<PoolTagIter<'a>> {
        // 0x10 - SystemHandleInformation
        let b = query(0x10, false)?;
        let e = unsafe { &*(b.as_ptr() as *const PoolTagList) };
        let v = unsafe { from_raw_parts(e.tags.as_ptr(), e.tag_count as usize) };
        Ok(PoolTagIter { v: v.iter(), _buf: b })
    }
}
impl<'a> RegValueIter<'a> {
    #[inline]
    pub fn new(key: Key) -> Win32Result<RegValueIter<'a>> {
        let mut b = Vec::new();
        let i = NtQueryKeyInfo(key, &mut b)?;
        Ok(RegValueIter {
            key,
            pos: 0u32,
            max: i.values,
            buf: b,
            _p: PhantomData,
        })
    }

    #[inline]
    pub fn to_names(self) -> Win32Result<Vec<String>> {
        let mut r = Vec::with_capacity(self.max as usize);
        for i in self {
            r.push(utf16_to_string(i?.as_slice()));
        }
        Ok(r)
    }
}

impl Clone for Module {
    #[inline]
    fn clone(&self) -> Module {
        Module {
            path:       self.path.clone(),
            size:       self.size,
            order:      self.order,
            flags:      self.flags,
            address:    self.address,
            load_count: self.load_count,
        }
    }
}
impl From<&ModuleEntry> for Module {
    #[inline]
    fn from(v: &ModuleEntry) -> Module {
        v.module(&system_root())
    }
}

impl Clone for Process {
    #[inline]
    fn clone(&self) -> Process {
        Process {
            pid:     self.pid,
            ppid:    self.ppid,
            user:    self.user.clone(),
            cmdline: self.cmdline.clone(),
            threads: self.threads,
            session: self.session,
        }
    }
}
impl From<&ProcessEntry<'_>> for Process {
    #[inline]
    fn from(v: &ProcessEntry<'_>) -> Process {
        let (c, u) = details(v);
        Process {
            pid:     v.pid as u32,
            ppid:    v.ppid as u32,
            user:    u,
            cmdline: c,
            threads: v.thread_count,
            session: v.session,
        }
    }
}

impl Iterator for DnsIter<'_> {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        match self.cur {
            Some(v) => self.cur = v.next,
            None => self.cur = self.base.first_dns,
        }
        self.cur.and_then(|v| v.addr.address?.addr())
    }
}
impl FusedIterator for DnsIter<'_> {}

impl Iterator for AddressIter<'_> {
    type Item = SocketAddr;

    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        loop {
            if let Some(a) = self.0.next()?.address() {
                return Some(a);
            }
        }
    }
}
impl FusedIterator for AddressIter<'_> {}

impl<'a> Iterator for UnicastIter<'a> {
    type Item = &'a UnicastAddress<'a>;

    #[inline]
    fn next(&mut self) -> Option<&'a UnicastAddress<'a>> {
        match self.cur {
            Some(v) => self.cur = v.next,
            None => self.cur = self.base.first_unicast,
        }
        self.cur
    }
}
impl FusedIterator for UnicastIter<'_> {}

impl<'a> Iterator for ProcessIter<'a> {
    type Item = &'a ProcessEntry<'a>;

    #[inline]
    fn next(&mut self) -> Option<&'a ProcessEntry<'a>> {
        if self.next == 0 && self.pos > 0 {
            return None;
        }
        self.pos += self.next;
        let p = unsafe { (self.buf.as_ptr().add(self.pos) as *const ProcessEntry).as_ref()? };
        self.next = p.next as usize;
        if p.pid > 0 {
            Some(p)
        } else {
            self.next()
        }
    }
}
impl FusedIterator for ProcessIter<'_> {}

impl<'a> Iterator for ModuleIter<'a> {
    type Item = &'a ModuleEntry;

    #[inline]
    fn next(&mut self) -> Option<&'a ModuleEntry> {
        self.v.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.v.len(), Some(self.v.len()))
    }
}
impl FusedIterator for ModuleIter<'_> {}
impl<'a> ExactSizeIterator for ModuleIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}

impl<'a> Iterator for HandleIter<'a> {
    type Item = &'a HandleEntry;

    #[inline]
    fn next(&mut self) -> Option<&'a HandleEntry> {
        self.v.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.v.len(), Some(self.v.len()))
    }
}
impl FusedIterator for HandleIter<'_> {}
impl<'a> ExactSizeIterator for HandleIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}

impl<'a> Iterator for HandleExIter<'a> {
    type Item = &'a HandleEntryEx;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.v.len(), Some(self.v.len()))
    }
    #[inline]
    fn next(&mut self) -> Option<&'a HandleEntryEx> {
        self.v.next()
    }
}
impl FusedIterator for HandleExIter<'_> {}
impl<'a> ExactSizeIterator for HandleExIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}

impl<'a> Iterator for PoolTagIter<'a> {
    type Item = &'a PoolTagEntry;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.v.len(), Some(self.v.len()))
    }
    #[inline]
    fn next(&mut self) -> Option<&'a PoolTagEntry> {
        self.v.next()
    }
}
impl FusedIterator for PoolTagIter<'_> {}
impl<'a> ExactSizeIterator for PoolTagIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }
}

impl<'a> Iterator for AdapterIter<'a> {
    type Item = &'a Adapter<'a>;

    #[inline]
    fn next(&mut self) -> Option<&'a Adapter<'a>> {
        match self.cur {
            Some(v) => self.cur = v.next,
            None => self.cur = Some(self.base),
        }
        self.cur
    }
}
impl FusedIterator for AdapterIter<'_> {}
impl<'a> IntoIterator for &'a Adapter<'a> {
    type Item = SocketAddr;
    type IntoIter = AddressIter<'a>;

    #[inline]
    fn into_iter(self) -> AddressIter<'a> {
        self.addresses()
    }
}

impl<'a> Iterator for RegKeyIter<'a> {
    type Item = Win32Result<&'a RegKeyBasicInfo>;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        ((self.max as usize), Some(self.max as usize))
    }
    #[inline]
    fn next(&mut self) -> Option<Win32Result<&'a RegKeyBasicInfo>> {
        if self.pos >= self.max {
            return None;
        }
        let r = match NtEnumerateKey(self.key, self.pos, &mut self.buf) {
            Ok(None) => return None,
            // We don't take the direct value of the above function as the compiler
            // thinks it's not tied to the Vec this struct has, even though it's the same as below.
            Ok(_) => Ok(unsafe { (self.buf.as_ptr() as *const RegKeyBasicInfo).as_ref()? }),
            Err(e) => Err(e),
        };
        self.pos += 1;
        Some(r)
    }
}
impl FusedIterator for RegKeyIter<'_> {}
impl<'a> ExactSizeIterator for RegKeyIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.max as usize
    }
}

impl<'a> Iterator for RegValueIter<'a> {
    type Item = Win32Result<&'a RegValueFullInfo>;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        ((self.max as usize), Some(self.max as usize))
    }
    #[inline]
    fn next(&mut self) -> Option<Win32Result<&'a RegValueFullInfo>> {
        if self.pos >= self.max {
            return None;
        }
        let r = match NtEnumerateValueKey(self.key, self.pos, &mut self.buf) {
            Ok(None) => return None,
            // We don't take the direct value of the above function as the compiler
            // thinks it's not tied to the Vec this struct has, even though it's the same as below.
            Ok(_) => Ok(unsafe { (self.buf.as_ptr() as *const RegValueFullInfo).as_ref()? }),
            Err(e) => Err(e),
        };
        self.pos += 1;
        Some(r)
    }
}
impl FusedIterator for RegValueIter<'_> {}
impl<'a> ExactSizeIterator for RegValueIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.max as usize
    }
}

fn details(v: &ProcessEntry<'_>) -> (String, String) {
    // 0x1410 - PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION |
    //           PROCESS_VM_READ
    let h = match OpenProcess(0x1410, false, v.pid as u32) {
        Ok(h) => h,
        Err(_) => return (utf16_to_string(v.name.as_slice()), String::new()),
    };
    // Don't inspect PPL processes.
    if process_protection(&h).map_or(false, |v| v.is_none()) {
        let c = match process_cmdline(&h) {
            Ok(x) => x,
            Err(_) => utf16_to_string(v.name.as_slice()),
        };
        return (c, process_user(h).unwrap_or_default());
    }
    (utf16_to_string(v.name.as_slice()), String::new())
}
fn query(class: u32, debug: bool) -> Win32Result<Vec<u8>> {
    let f = syscall!(
        ntdll().NtQuerySystemInformation,
        fn(u32, *mut u8, u32, *mut u32) -> u32
    );
    // Windows 11+ requires the Debug privilege to get the Module base address.
    // if it fails, the call succeeds, but will return 0 as the base address.
    let p = if debug {
        privilege_accquire(Privilege::SeDebug).is_ok()
    } else {
        false
    };
    let mut n = 0u32;
    unsafe {
        let _ = f(class, null_mut(), 0, &mut n);
    }
    let mut b = Vec::with_capacity((n as usize) * 2);
    loop {
        b.resize((n as usize) * 2, 0);
        let r = unsafe { f(class, b.as_mut_ptr(), n, &mut n) };
        match r {
            // 0x80000005 - STATUS_BUFFER_OVERFLOW
            // 0xC0000004 - STATUS_INFO_LENGTH_MISMATCH
            // 0xC0000023 - STATUS_BUFFER_TOO_SMALL
            0x80000005 | 0xC0000004 | 0xC0000023 => continue,
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    if p {
        let _ = privilege_release(Privilege::SeDebug);
    }
    Ok(b)
}
fn adapters(family: u32, flags: u32) -> Win32Result<Vec<u8>> {
    let f = syscall!(
        iphlpapi().GetAdaptersAddresses,
        fn(u32, u32, usize, *mut u8, *mut u32) -> u32
    );
    let mut n = 15_000;
    let mut b = Vec::with_capacity((n as usize) * 2);
    loop {
        b.resize((n as usize) * 2, 0);
        let r = unsafe { f(family, flags, 0, b.as_mut_ptr(), &mut n) };
        match r {
            0x6F => continue, // 0x6F - ERROR_BUFFER_OVERFLOW
            0 => break,
            _ => return Err(Win32Error::from_status(r)),
        }
    }
    Ok(b)
}
