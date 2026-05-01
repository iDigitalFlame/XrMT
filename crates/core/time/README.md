# XrMT Time Crate

The XrMT Time Crate provides structs and functions to deal with "wall clock" time.
This crate provides a `Time` struct that can be used to describe the time and do
"time math" between `Time` and `Duration` structs. This crate also provides
system-specific methods to get the current time.

Other `std::time` and `core::time` structs and constants are exported with this
crate, allowing it to serve as a drop-in replacement for `std::time`

## Features

- **strip**: Removes some strings from compilation, mostly regarding error messages.
             Messages will instead reflect their error code or index. This should
             not be used while debugging. This has no effect when "std" is used.

## Default Features

- **None**

## Dependencies

- **libc** (*nix only)
