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
use core::alloc::{AllocError, Allocator, Layout};
use core::cell::Cell;
use core::cmp;
use core::intrinsics::unlikely;
use core::mem::size_of;
use core::ptr::{self, NonNull};
use core::slice::{from_raw_parts, from_raw_parts_mut};

use crate::prelude::*;

const INITIAL_MIN: usize = 256;
const INITIAL_MULTIPLY: usize = 3;

const BLOCK_SIZE: usize = size_of::<Block>();
const HEADER_SIZE: usize = size_of::<Header>();

pub struct AllocIter<'a> {
    pos:  usize,
    root: &'a Entry,
}
pub struct AllocMutIter<'a> {
    pos:  usize,
    root: &'a Entry,
}
pub struct Manager(Box<Entry>);
pub struct Silo(NonNull<Entry>);
pub struct LeafIter<'a>(&'a Entry);

#[repr(C)]
struct Block {
    mem:  Heap,
    next: Entry,
    size: usize,
}
struct Address {
    end:    usize,
    next:   usize,
    start:  usize,
    header: Header,
}
#[repr(transparent)]
struct Header(usize);
struct BlockIter<'a> {
    ptr: &'a Block,
    pos: usize,
}
struct Heap(NonNull<u8>);
struct Entry(Cell<NonNull<Block>>);

impl Heap {
    #[inline]
    fn addr(&self) -> usize {
        self.0.as_ptr() as usize
    }
    #[inline]
    fn free(&self, layout: Layout) {
        unsafe { Global.deallocate(self.0, layout) }
    }
    #[inline]
    fn read<T>(&self, pos: usize) -> T {
        unsafe { ptr::read((self.0.as_ptr() as *mut u8).add(pos) as *mut T) }
    }
    #[inline]
    fn write<T>(&self, pos: usize, v: T) {
        unsafe { ptr::write((self.0.as_ptr() as *mut u8).add(pos) as *mut T, v) }
    }
    #[inline]
    fn addr_at(&self, pos: usize) -> usize {
        unsafe { self.0.as_ptr().add(pos) as usize }
    }
    #[inline]
    fn allocate(layout: Layout) -> Result<Heap, AllocError> {
        Ok(Heap(unsafe {
            // NOTE(dij): If it's not zero'd we'll have problems!!
            NonNull::new_unchecked(Global.allocate_zeroed(layout)?.as_ptr() as *mut u8)
        }))
    }
    #[inline]
    fn slice(&self, pos: usize, size: usize, align: usize) -> NonNull<[u8]> {
        unsafe {
            NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(
                // Take off the header.
                (self.0.as_ptr() as *mut u8).add(pos + align).add(HEADER_SIZE),
                size,
            ))
        }
    }
    #[inline]
    fn bytes<'a>(&'a self, pos: usize, size: usize) -> &'a [u8] {
        unsafe { from_raw_parts((self.0.as_ptr() as *mut u8).add(pos).add(HEADER_SIZE), size) }
    }
    #[inline]
    fn bytes_mut<'a>(&'a self, pos: usize, size: usize) -> &'a mut [u8] {
        unsafe { from_raw_parts_mut((self.0.as_ptr() as *mut u8).add(pos).add(HEADER_SIZE), size) }
    }
}
impl Silo {
    #[inline]
    fn get(&self) -> &Entry {
        unsafe { self.0.as_ref() }
    }
}
impl Entry {
    #[inline]
    const fn empty() -> Entry {
        Entry(Cell::new(NonNull::dangling()))
    }

    #[inline]
    fn free(&self) {
        if self.is_empty() {
            return;
        }
        if let Some(e) = self.next() {
            e.free()
        }
        self.as_mut().free()
    }
    fn trim(&self) {
        // Should only be called by top level.
        if self.is_empty() {
            return;
        }
        let mut n = match self.next() {
            Some(v) => v,
            None if self.as_mut().is_empty() => {
                self.free();
                // Free outselves and then clear our ref.
                self.unlink();
                return;
            },
            None => return,
        };
        let mut b = self;
        loop {
            // Sanity check, should always work.
            if unlikely(n.is_empty()) {
                break;
            }
            if n.as_mut().is_empty() {
                // Save this pointer as it will be removed one we free.
                let t = n;
                match n.next() {
                    // Swap b's next with the next value of n.
                    Some(e) => {
                        b.replace(e);
                        n = e;
                        t.as_mut().free();
                        // We don't break as this will continue as if we skipped
                        // n and will process e next.
                    },
                    None => {
                        t.as_mut().free();
                        b.unlink();
                        // It has no next, break.
                        break;
                    },
                }
                continue;
            }
            b = n;
            n = match n.next() {
                Some(v) => v,
                None => break,
            };
        }
    }
    #[inline]
    fn unlink(&self) {
        self.0.set(NonNull::dangling());
    }
    #[inline]
    fn is_empty(&self) -> bool {
        unsafe { &*self.0.as_ptr() }.as_ptr() as usize == HEADER_SIZE
    }
    #[inline]
    fn replace(&self, new: &Entry) {
        self.0.set(new.0.get());
        new.unlink();
    }
    #[inline]
    fn as_mut(&self) -> &mut Block {
        unsafe { (&mut *(self.0.as_ptr())).as_mut() }
    }
    #[inline]
    fn as_slice(&self) -> NonNull<[u8]> {
        unsafe {
            NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(
                // Take off the header.
                self.as_mut().mem.0.as_ptr() as *mut u8,
                self.as_mut().size,
            ))
        }
    }
    #[inline]
    fn next<'a>(&'a self) -> Option<&'a Entry> {
        if self.as_mut().next.is_empty() {
            None
        } else {
            Some(&self.as_mut().next)
        }
    }
    #[inline]
    fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Called by top level only.
        if self.is_empty() {
            return;
        }
        self.as_mut().deallocate(ptr.as_ptr() as usize, layout);
    }
    #[inline]
    fn init(&self, layout: Layout) -> Result<(), AllocError> {
        self.0.set(Block::new(layout)?);
        Ok(())
    }
    #[inline]
    fn bytes<'a>(&'a self, pos: usize, size: usize) -> &'a [u8] {
        self.as_mut().mem.bytes(pos, size)
    }
    #[inline]
    fn get(&self, layout: Layout) -> Result<&mut Block, AllocError> {
        if self.is_empty() {
            self.init(layout)?
        }
        Ok(self.as_mut())
    }
    #[inline]
    fn bytes_mut<'a>(&'a self, pos: usize, size: usize) -> &'a mut [u8] {
        self.as_mut().mem.bytes_mut(pos, size)
    }
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            // Return an empty pointer for ZST (Zero Size Types) as this is what
            // the std allocator does.
            return Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0));
        }
        let l = layout.pad_to_align();
        // Alignment check is preformed in the 'alloc' call and will panic if it
        // happens.
        self.get(l)?.alloc(l)
    }
}
impl Block {
    #[inline]
    fn new(layout: Layout) -> Result<NonNull<Block>, AllocError> {
        let n = cmp::max(
            if let Ok((v, _)) = layout.repeat(INITIAL_MULTIPLY) {
                v.size()
            } else {
                layout.size() * INITIAL_MULTIPLY
            } + BLOCK_SIZE
                + HEADER_SIZE,
            INITIAL_MIN,
        );
        Ok(Block::write(Block {
            mem:  Heap::allocate(unsafe { Layout::from_size_align_unchecked(n, HEADER_SIZE).pad_to_align() })?,
            next: Entry::empty(),
            size: n,
        }))
    }

    #[inline]
    fn free(&self) {
        self.mem
            .free(unsafe { Layout::from_size_align_unchecked(self.size, HEADER_SIZE) });
    }
    #[inline]
    fn is_empty(&self) -> bool {
        if self.size == 0 {
            return true;
        }
        let h: Header = self.mem.read(BLOCK_SIZE);
        if h.0 == 0 {
            // Fast path: If the first header is empty, we know there's nothing here.
            true
        } else if !h.is_empty() {
            // Fast path: If the first Silo is being used, it's not empty.
            false
        } else {
            // Check to see if any Silos are in use. If a single Silo is in use then
            // this Block isn't empty.
            self.iter().find(|a| !a.header.is_empty()).is_none()
        }
    }
    #[inline]
    fn write(b: Block) -> NonNull<Block> {
        let m: NonNull<u8> = b.mem.0;
        let p = unsafe { NonNull::new_unchecked(m.as_ptr() as *mut Block) };
        unsafe { ptr::write(m.as_ptr().add(BLOCK_SIZE) as *mut Header, Header(0)) };
        unsafe { ptr::write(m.as_ptr() as *mut Block, b) }
        p
    }
    #[inline]
    fn iter<'a>(&'a self) -> BlockIter<'a> {
        BlockIter { ptr: self, pos: 0 }
    }
    fn walk(&self, pos: usize) -> Option<Address> {
        let p = if pos > 0 { pos } else { pos.checked_add(BLOCK_SIZE)? };
        if p > self.size {
            return None;
        }
        let h: Header = self.mem.read(p);
        if h.0 == 0 {
            None
        } else {
            let e = h.size();
            Some(Address {
                end:    e,
                next:   p.checked_add(HEADER_SIZE)?.checked_add(e)?,
                start:  p,
                header: h,
            })
        }
    }
    #[inline]
    fn deallocate(&self, addr: usize, layout: Layout) -> bool {
        match self.contains(addr, layout) {
            Some(a) => {
                // Check the next Header to see if it's empty, it empty,
                // directly evict this Header and set it to zero, to allow for
                // other Headers to be made on top.
                //
                // This has the side effect to resetting a Block if it's now
                // empty.
                let h: Header = self.mem.read(a.start);
                if h.is_zero() {
                    // Evict the Header and reduce the used size.
                    self.mem.write(a.start, Header(0))
                } else {
                    // Remove the used bit, marking this Header free for use.
                    self.mem.write(a.start, h.empty())
                }
            },
            None if !self.next.is_empty() => return self.next.as_mut().deallocate(addr, layout),
            // Can't find it? This should not happen!!
            _ => {
                bugtrack!("(Block).deallocate(): Cannot find the allocation for pointer {addr:X}!");
                core::unreachable!();
            },
        }
        true
    }
    fn next(&self, layout: Layout) -> Option<(usize, Header, usize)> {
        // All addresses returned INCLUDE THE HEADER_SIZE ptr.
        // This shouldn't fail, but check it anyway.
        let n = layout.size().checked_add(layout.align() + HEADER_SIZE)?;
        if n > self.size {
            // Reject any blocks that are too small.
            return None;
        }
        // Last seen empty Header and Seek position.
        let (mut k, mut p) = (0, BLOCK_SIZE);
        loop {
            // Break if the pos is greater than this Block's total size.
            if p > self.size {
                break;
            }
            // Check space alignment with out current position + HEADER SIZE.
            // This shouldn't fail, but check it anyway.
            //
            // Skip alignment checks for anything 1 or zero in alignment.
            let a = if layout.align() > 1 {
                layout.align() - (self.mem.addr_at(p.checked_add(HEADER_SIZE)?) & (layout.align() - 1))
            } else {
                0
            };
            let r = layout.size() + a;
            // if 'a' is greater than zero, then the alignment is off, but adding
            // 'a' will properly align the value.
            // 'r' Is the "real" size we need to store the value, so we'll use
            // that from now on.
            //
            // Check if the current position from the total is big enough to fit
            // the request size plus a Header.
            //
            // We don't need to check here as we already checked above, so it's
            // fine.
            if (r + HEADER_SIZE) > self.size.checked_sub(p)? {
                return None;
            }
            // Read the Header at the current position.
            let h: Header = self.mem.read(p);
            if h.is_zero() {
                // Was the last header free and was the last one before a non
                // created Header?
                if k > 0 {
                    // If so we can use it and it can be resized to fit our needs.
                    return Some((k, Header(r), a));
                }
                // True empty header, we've validated that we have space, so
                // return this.
                return Some((p, Header(r), a));
            }
            if h.is_empty() {
                // Otherwise, this is most likely a free'd Header.
                // We minus the size as the header size is the real size.
                if h.size() >= r {
                    // This header fits or is bigger than what we need, use this.
                    return Some((p, h, a));
                }
                // This header is too small, keep going, but store it's position.
                k = p;
            } else {
                // If we find a non-empty header, clear the last empty flag.
                k = 0;
            }
            // Advance to the next Header.
            p = p.checked_add(h.size())?.checked_add(HEADER_SIZE)?;
        }
        // We ain't find shit.
        None
    }
    #[inline]
    fn contains(&self, addr: usize, layout: Layout) -> Option<Address> {
        let t = layout.size();
        // Fast path: The layout size is bigger then the Block, it can't be in
        // here.
        if t > self.size {
            return None;
        }
        // 'n' Should ALWAYS be smaller than our start address. It's just a simple
        // way to determine if this Block contains the address.
        let n = match addr.checked_sub(self.mem.addr() + BLOCK_SIZE + (layout.align())) {
            Some(n) => n,
            None => return None,
        };
        self.iter().find(|a| a.start >= n && a.matches(&self.mem, addr))
    }
    #[inline]
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() >= isize::MAX as usize || layout.size() + layout.align() >= isize::MAX as usize {
            // Due to how the signing system works, we can't allocate anything that takes
            // over the last (63rd) bit position.
            return Err(AllocError);
        }
        // The layout should already be aligned here.
        let r = if let Some((p, h, a)) = self.next(layout) {
            // 'p' Starts at the Header base addr.
            // 'h' Has the size of the Slot, h may NOT equal layout.size() in cases
            // where a Slot is reused.
            // 'a' Contains the alignment, may be zero.
            bugtrack!("(Block).alloc(): next-> p={p}, h={}, a={a}", h.size());
            Ok(self.claim(layout.size(), p, h, a))
        } else {
            self.next.get(layout)?.alloc(layout)
        }?;
        bugtrack!(
            "(Block).alloc(): mem={}, addr={layout:?}, r={:p}",
            self.mem.addr(),
            r
        );
        if unlikely(!r.is_aligned_to(layout.align())) {
            bugtrack!("(Block).alloc(): Returned a non-aligned pointer!");
            core::unreachable!();
        }
        Ok(r)
    }
    #[inline]
    fn claim(&self, size: usize, pos: usize, h: Header, align: usize) -> NonNull<[u8]> {
        // pos is the Header base.
        self.mem.write(pos, h.filled());
        // This accounts for the Header size and alignment.
        self.mem.slice(pos, size, align)
    }
}
impl Header {
    #[inline]
    fn size(&self) -> usize {
        self.0 & !(1usize << (usize::BITS - 1))
    }
    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0
    }
    #[inline]
    fn empty(&self) -> Header {
        Header(self.0 & !(1usize << (usize::BITS - 1)))
    }
    #[inline]
    fn filled(&self) -> Header {
        Header(self.0 | (1usize << (usize::BITS - 1)))
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.0 == 0 || self.0 & (1 << (usize::BITS - 1)) == 0
    }
}
impl Address {
    #[inline]
    fn matches(&self, mem: &Heap, addr: usize) -> bool {
        let r = self.start + mem.addr() + HEADER_SIZE;
        addr >= r && addr < r + self.end
    }
}
impl Manager {
    #[inline]
    pub fn new() -> Manager {
        // The root entry is in a Box to prevent it from getting moved when it's
        // on the stack and causing issues.
        Manager(Box::new(Entry::empty()))
    }

    #[inline]
    pub const fn silo(&self) -> Silo {
        Silo(unsafe { NonNull::new_unchecked((&*self.0 as *const Entry) as *mut Entry) })
    }

    #[inline]
    pub fn trim(&self) {
        if self.0.is_empty() {
            return;
        }
        self.0.trim()
    }
    #[inline]
    pub fn wrap(&self, key: impl AsRef<[u8]>) {
        let k = key.as_ref();
        let n = k.len();
        for i in self.iter_mut() {
            for x in 0..i.len() {
                i[x] = i[x] ^ k[x % n]
            }
        }
    }
    #[inline]
    pub fn iter<'a>(&'a self) -> AllocIter<'a> {
        AllocIter { pos: 0usize, root: &self.0 }
    }
    #[inline]
    pub fn leafs<'a>(&'a self) -> LeafIter<'a> {
        LeafIter(&self.0)
    }
    #[inline]
    pub fn iter_mut<'a>(&'a self) -> AllocMutIter<'a> {
        AllocMutIter { pos: 0usize, root: &self.0 }
    }
}

impl Drop for Manager {
    #[inline]
    fn drop(&mut self) {
        self.0.free()
    }
}

impl Copy for Silo {}
impl Clone for Silo {
    #[inline]
    fn clone(&self) -> Silo {
        Silo(self.0.clone())
    }
}

impl<'a> Iterator for LeafIter<'_> {
    type Item = NonNull<[u8]>;

    #[inline]
    fn next(&mut self) -> Option<NonNull<[u8]>> {
        if self.0.is_empty() {
            return None;
        }
        let r = self.0.as_slice();
        self.0 = &self.0.as_mut().next;
        Some(r)
    }
}
impl<'a> Iterator for AllocIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.root.is_empty() {
            return None;
        }
        match self.root.as_mut().walk(self.pos) {
            Some(a) => {
                self.pos = a.next;
                if a.header.is_empty() {
                    return self.next();
                }
                Some(self.root.bytes(a.start, a.end))
            },
            None => {
                if let Some(e) = self.root.next() {
                    (self.root, self.pos) = (e, 0);
                    self.next()
                } else {
                    None
                }
            },
        }
    }
}
impl<'a> Iterator for BlockIter<'_> {
    type Item = Address;

    #[inline]
    fn next(&mut self) -> Option<Address> {
        match self.ptr.walk(self.pos) {
            Some(a) => {
                self.pos = a.next;
                Some(a)
            },
            None => None,
        }
    }
}
impl<'a> Iterator for AllocMutIter<'a> {
    type Item = &'a mut [u8];

    fn next(&mut self) -> Option<&'a mut [u8]> {
        if self.root.is_empty() {
            return None;
        }
        match self.root.as_mut().walk(self.pos) {
            Some(a) => {
                self.pos = a.next;
                if a.header.is_empty() {
                    return self.next();
                }
                Some(self.root.bytes_mut(a.start, a.end))
            },
            None => {
                if let Some(e) = self.root.next() {
                    (self.root, self.pos) = (e, 0);
                    self.next()
                } else {
                    None
                }
            },
        }
    }
}

unsafe impl Allocator for Silo {
    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.get().deallocate(ptr, layout)
    }
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.get().allocate(layout)
    }
}

#[cfg(not(feature = "strip"))]
mod display {
    use core::fmt::{self, Debug, Display, Formatter};

    use crate::data::memory::Silo;
    use crate::prelude::*;

    impl Debug for Silo {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Silo").field(&self.0.as_ptr()).finish()
        }
    }
    impl Display for Silo {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("Silo")
        }
    }
}
