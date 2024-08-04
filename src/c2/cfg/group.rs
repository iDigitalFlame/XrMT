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

use core::alloc::Allocator;
use core::cmp;

use crate::c2::cfg::{OwnedConfig, Setting, SEPARATOR};
use crate::prelude::*;

pub const SELECTOR_LAST_VALID: u8 = 0xAAu8;
pub const SELECTOR_ROUND_ROBIN: u8 = 0xABu8;
pub const SELECTOR_RANDOM: u8 = 0xACu8;
pub const SELECTOR_SEMI_ROUND_ROBIN: u8 = 0xADu8;
pub const SELECTOR_SEMI_RANDOM: u8 = 0xAEu8;
pub const SELECTOR_SEMI_LAST_VALID: u8 = 0xA7u8;

pub(super) const SELECTOR_PERCENT: u8 = 0xA8u8;
pub(super) const SELECTOR_PERCENT_ROUND_ROBIN: u8 = 0xA9u8;

impl<A: Allocator> OwnedConfig<A> {
    #[inline]
    pub fn select_random(mut self) -> OwnedConfig<A> {
        self.0.push(SELECTOR_RANDOM);
        self
    }
    #[inline]
    pub fn select_last_valid(mut self) -> OwnedConfig<A> {
        self.0.push(SELECTOR_LAST_VALID);
        self
    }
    #[inline]
    pub fn select_round_robin(mut self) -> OwnedConfig<A> {
        self.0.push(SELECTOR_ROUND_ROBIN);
        self
    }
    #[inline]
    pub fn select_semi_random(mut self) -> OwnedConfig<A> {
        self.0.push(SELECTOR_SEMI_RANDOM);
        self
    }
    #[inline]
    pub fn select_semi_round_robin(mut self) -> OwnedConfig<A> {
        self.0.push(SELECTOR_SEMI_ROUND_ROBIN);
        self
    }
    #[inline]
    pub fn select_random_percent(mut self, per: u8) -> OwnedConfig<A> {
        self.0.push(SELECTOR_PERCENT);
        self.0.push(cmp::max(per, 100));
        self
    }
    pub fn add_group(mut self, new: impl Setting<A>) -> OwnedConfig<A> {
        if new.is_empty() {
            return self;
        }
        if self.is_empty() {
            new.write(&mut self.0);
            return self;
        }
        self.0.reserve(new.len() + 1);
        self.0.push(SEPARATOR);
        new.write(&mut self.0);
        self
    }
    #[inline]
    pub fn select_round_robin_percent(mut self, per: u8) -> OwnedConfig<A> {
        self.0.push(SELECTOR_PERCENT_ROUND_ROBIN);
        self.0.push(cmp::max(per, 100));
        self
    }
}
