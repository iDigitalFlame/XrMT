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

extern crate core;

extern crate xrmt_stx;

use core::clone::Clone;
use core::convert::From;
use core::future::Future;
use core::iter::Iterator;
use core::marker::{Copy, PhantomData, Unpin};
use core::ops::{Deref, DerefMut, FnMut};
use core::option::Option::{self, Some};
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use xrmt_stx::time::Instant;

use crate::link;

pub enum Status<'a, 'b> {
    Setup(&'b mut State<'a>),
    Ready(&'b mut State<'a>),
}

pub struct Sleep(Instant);
pub struct Wrapper<T: FutureExt>(T);
pub struct Iter<T: AsyncIterator>(T);
pub struct FutureMap<R, S: FutureSetup<R>, P: FuturePoll<R>> {
    s:  S,
    p:  P,
    _p: PhantomData<R>,
}

pub trait FutureExt {
    type Output;

    fn poll(&mut self, state: &mut State<'_>) -> Poll<Self::Output>;

    #[inline]
    #[allow(unused_variables)]
    fn setup(&mut self, state: &mut State<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
pub trait AsyncIterator {
    type Item;

    async fn next(&mut self) -> Option<Self::Item>;
}
pub trait FuturePoll<R> = FnMut(&mut State) -> Poll<R>;
pub trait FutureSetup<R> = FnMut(&mut State) -> Poll<R>;

pub type I<T> = Iter<T>;
pub type F<T> = Wrapper<T>;
pub type State<'a> = crate::runtime::State<'a>;

impl Sleep {
    #[inline]
    pub fn sleep(dur: Duration) -> Sleep {
        Sleep(Instant::now() + dur)
    }
    #[inline]
    pub fn deadline(when: Instant) -> Sleep {
        Sleep(when)
    }
}
impl Status<'_, '_> {
    #[inline]
    pub fn is_ready(&self) -> bool {
        match self {
            Status::Ready(_) => true,
            Status::Setup(_) => false,
        }
    }
}
impl<T: FutureExt> Wrapper<T> {
    #[inline]
    pub const fn new(v: T) -> Wrapper<T> {
        Wrapper(v)
    }
}
impl<T: AsyncIterator> Iter<T> {
    #[inline]
    pub const fn new(v: T) -> Iter<T> {
        Iter(v)
    }
}

impl<T: FutureExt> FutureExt for &mut T {
    type Output = T::Output;

    #[inline]
    fn poll(&mut self, state: &mut State<'_>) -> Poll<Self::Output> {
        (&mut **self).poll(state)
    }
    #[inline]
    fn setup(&mut self, state: &mut State<'_>) -> Poll<Self::Output> {
        (&mut **self).setup(state)
    }
}

impl<T: FutureExt> Deref for Wrapper<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: FutureExt> Unpin for Wrapper<T> {}
impl<T: FutureExt> Future for Wrapper<T> {
    type Output = T::Output;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T::Output> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) => self.0.poll(v),
            Status::Setup(v) => self.0.setup(v),
        }
    }
}
impl<T: FutureExt> From<T> for Wrapper<T> {
    #[inline]
    fn from(v: T) -> Wrapper<T> {
        Wrapper::new(v)
    }
}
impl<T: FutureExt> DerefMut for Wrapper<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<R, S: FutureSetup<R>, P: FuturePoll<R>> Unpin for FutureMap<R, S, P> {}
impl<R, S: FutureSetup<R>, P: FuturePoll<R>> Future for FutureMap<R, S, P> {
    type Output = R;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<R> {
        match State::from_context(cx).status(&self) {
            Status::Ready(v) => (self.p)(v),
            Status::Setup(v) => (self.s)(v),
        }
    }
}
impl<R, S: FutureSetup<R>, P: FuturePoll<R>> FutureExt for FutureMap<R, S, P> {
    type Output = R;

    #[inline]
    fn poll(&mut self, state: &mut State<'_>) -> Poll<R> {
        (&mut self.p)(state)
    }
    #[inline]
    fn setup(&mut self, state: &mut State<'_>) -> Poll<R> {
        (&mut self.s)(state)
    }
}

impl Copy for Sleep {}
impl Clone for Sleep {
    #[inline]
    fn clone(&self) -> Sleep {
        Sleep(self.0.clone())
    }
}
impl Future for Sleep {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match State::from_context(cx).status(&self) {
            Status::Ready(_) => Poll::Ready(()),
            Status::Setup(v) => {
                v.set_deadline(Some(self.0));
                Poll::Pending
            },
        }
    }
}

impl<T: AsyncIterator> Iterator for Iter<T> {
    type Item = T::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        link(self.0.next())
    }
}

#[inline]
pub fn sleep(dur: Duration) -> Sleep {
    Sleep::sleep(dur)
}
#[inline]
pub fn deadline(when: Instant) -> Sleep {
    Sleep::deadline(when)
}
