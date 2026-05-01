# XrMT I/O Crate

The XrMT I/O Crate is a abstraction of the Rust `std::io` crate. This can be
used directly as a drop-in replacement for `std::io`.

This crate also functions as a "detached io" crate, allowing for `no_std`
crates to use some IO features without re-implementing them.

The nature of this crate allows it to be used in embedded or "non-os" situations.
The additional `alloc` feature allows for useage of features that require the
`alloc` crate, such as complex `io::Error` objects.

## Features

- **std**  : Redirects all the internal crate functions to the `std::io` crate.
             This is the default for any *nix device.
- **alloc**: Enables the ability to use the `alloc` crate to create some structs
             and `Error` structs. This is **not** supported on embedded devices or
             devices that do not have an allocator. This is enabled when the `std`
             feature is enabled.
- **strip**: Removes some strings from compilation, mostly regarding error messages.
             Messages will instead reflect their error code or index. This should
             not be used while debugging. This has no effect when "std" is used.

## Default Features

- *alloc* (non *nix)
- *std* (*nix)

## Dependencies

- **None**
