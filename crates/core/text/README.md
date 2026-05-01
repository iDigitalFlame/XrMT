# XrMT Text Crate

The XrMT Text Crate provides fast and efficient text conversions between different
encoding formats. There are multiple functions provided that can convert text
into lossy, UTF16 or UTF8 formats easily with no allocations. Additionally, multiple
Iterators are provided for looping through valid strings or characters in multiple
encoding formats.

If the `alloc` crate is supported via the default `alloc` feature, this crate also
includes fast hex and base-10 integer to string conversions.

This crate is entirely `no_std` and can be used anywhere.

## Features

- **alloc**: Enables the ability to use the `alloc` crate to do integer to string
             conversions into `Vec` structs and Arrays.

## Default Features

- **alloc**

## Dependencies

- **None**
