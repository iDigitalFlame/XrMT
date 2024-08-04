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

use crate::io::{self, Write};
use crate::prelude::*;

#[derive(Debug)]
pub enum Base64Error {
    TooSmall,
}

#[inline]
pub fn encode(src: &[u8], dst: &mut [u8]) -> Result<usize, Base64Error> {
    encode_inner(b'=', false, src, dst)
}
#[inline]
pub fn encode_write(src: &[u8], dst: &mut impl Write) -> io::Result<usize> {
    encode_write_inner(b'=', false, src, dst)
}

fn encode_inner(pad: u8, url: bool, src: &[u8], dst: &mut [u8]) -> Result<usize, Base64Error> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < ((src.len() + 2) / 3) * 4 {
        return Err(Base64Error::TooSmall);
    }
    let t = (src.len() / 3) * 3;
    let (mut s, mut d) = (0, 0);
    while s < t {
        let v = (src[s] as u32) << 16 | (src[s + 1] as u32) << 8 | src[s + 2] as u32;
        dst[d] = encode_char(((v >> 18) & 0x3F) as u8, url);
        dst[d + 1] = encode_char(((v >> 12) & 0x3F) as u8, url);
        dst[d + 2] = encode_char(((v >> 6) & 0x3F) as u8, url);
        dst[d + 3] = encode_char((v & 0x3F) as u8, url);
        (s, d) = (s + 3, d + 4);
    }
    let r = src.len() - s;
    if r == 0 {
        return Ok(d);
    }
    let v = (src[s] as u32) << 16 | if r == 2 { (src[s + 1] as u32) << 8 } else { 0 };
    dst[d] = encode_char(((v >> 18) & 0x3F) as u8, url);
    dst[d + 1] = encode_char(((v >> 12) & 0x3F) as u8, url);
    match r {
        2 => {
            dst[d + 2] = encode_char(((v >> 6) & 0x3F) as u8, url);
            if pad > 0 {
                dst[d + 3] = pad;
            }
            d += 4;
        },
        1 if pad > 0 => {
            (dst[d + 2], dst[d + 3]) = (pad, pad);
            d += 4;
        },
        _ => d += 2,
    }
    Ok(d)
}
fn encode_write_inner(pad: u8, url: bool, src: &[u8], dst: &mut impl Write) -> io::Result<usize> {
    if src.is_empty() {
        return Ok(0);
    }
    let t = (src.len() / 3) * 3;
    let (mut s, mut d) = (0, 0);
    while s < t {
        let v = (src[s] as u32) << 16 | (src[s + 1] as u32) << 8 | src[s + 2] as u32;
        dst.write(&[
            encode_char(((v >> 18) & 0x3F) as u8, url),
            encode_char(((v >> 12) & 0x3F) as u8, url),
            encode_char(((v >> 6) & 0x3F) as u8, url),
            encode_char((v & 0x3F) as u8, url),
        ])?;
        (s, d) = (s + 3, d + 4);
    }
    let r = src.len() - s;
    if r == 0 {
        return Ok(d);
    }
    let v = (src[s] as u32) << 16 | if r == 2 { (src[s + 1] as u32) << 8 } else { 0 };
    dst.write(&[
        encode_char(((v >> 18) & 0x3F) as u8, url),
        encode_char(((v >> 12) & 0x3F) as u8, url),
    ])?;
    match r {
        2 => {
            dst.write(&[encode_char(((v >> 6) & 0x3F) as u8, url)])?;
            if pad > 0 {
                dst.write(&[pad])?;
            }
            d += 4;
        },
        1 if pad > 0 => {
            dst.write(&[pad, pad])?;
            d += 4;
        },
        _ => d += 2,
    }
    Ok(d)
}

fn encode_char(v: u8, url: bool) -> u8 {
    // A-Za-z +/ | -_
    match v {
        0..=26 => b'A' + v,
        27..=52 => b'a' + (v - 26),
        53..=61 => b'0' + (v - 52),
        62 if url => b'-',
        62 => b'+',
        63 if url => b'_',
        63 => b'/',
        _ => b'=',
    }
}
/*fn decode_char(c: u8, url: bool) -> u8 {
    // A-Za-z +/ | -_
    match c {
        b'A'..=b'Z' => (),
        b'a'..=b'z' => (),
        b'0'..=b'9' => (),
        b'-' if url => (),
        b'+' => (),
        b'_' if url => (),
        b'/' => (),
        _ => (),
    }
    0
}
*/
