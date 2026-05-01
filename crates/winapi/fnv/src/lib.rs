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
extern crate proc_macro;

use alloc::string::ToString;
use core::iter::{FromIterator, IntoIterator, Iterator};
use core::option::Option::{None, Some};

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

#[proc_macro]
pub fn fnv(v: TokenStream) -> TokenStream {
    if v.is_empty() {
        return error(
            Span::call_site(),
            "function name string or literal must be provided",
        );
    }
    let mut e = v.into_iter();
    let n = match e.nth(0) {
        None => {
            return error(
                Span::call_site(),
                "function name string or literal must be provided",
            )
        },
        Some(i) => i,
    };
    if e.next().is_some() {
        return error(
            Span::call_site(),
            "function argument must only be a single value",
        );
    }
    let d = match n {
        TokenTree::Ident(i) => i.to_string(),
        TokenTree::Literal(i) => i.to_string(),
        _ => {
            return error(
                Span::call_site(),
                "function name must be a string or literal",
            )
        },
    };
    let mut h: u32 = 0x811C9DC5;
    for (i, x) in d.as_bytes().iter().enumerate() {
        match *x {
            b'"' if i == 0 => continue,
            b'"' if i + 1 == d.len() => continue,
            b'_' | b'-' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => (),
            _ => {
                return error(
                    Span::call_site(),
                    "function name contains invalid characters",
                )
            },
        }
        h = h.wrapping_mul(0x1000193);
        h ^= *x as u32;
    }
    TokenStream::from_iter([TokenTree::Literal(Literal::u32_suffixed(h))])
}

fn error(loc: Span, v: &'static str) -> TokenStream {
    let mut p = Punct::new('!', Spacing::Alone);
    p.set_span(loc);
    let mut m = Literal::string(v);
    m.set_span(loc);
    let mut g = Group::new(
        Delimiter::Brace,
        TokenStream::from_iter([TokenTree::Literal(m)]),
    );
    g.set_span(loc);
    TokenStream::from_iter([
        TokenTree::Ident(Ident::new("compile_error", loc)),
        TokenTree::Punct(p),
        TokenTree::Group(g),
    ])
}
