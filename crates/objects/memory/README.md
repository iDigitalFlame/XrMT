# XrMT Memory Crate

The XrMT Memory Crate is a special crate that provides memory "Containers".

The provided "Containers" can be used to control how Rust manages and stores
memory.

## Features

- **bugs** : Enables the `xrmt-bugtrack` crate to log detailed allocation data for
             debugging purposes.
- **strip**: Removes some strings from compilation, mostly regarding error messages.
             Messages will instead reflect their error code or index. This should
             not be used while debugging. This has no effect when "std" is used.

## Default Features

- **None**

## Dependencies

- **xrmt-bugtrack**
