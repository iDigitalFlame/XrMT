# XrMT Random Crate

The XrMT Random Crate is a simple pseudo-random number generator.

This crate contains many different functions that can be used to generate random
numbers or byte slices. It is **not** recommended to use the random data provided
by this crate for secure operations.

The nature of this crate allows it to be used in embedded or "non-os" situations.
Without the `sys` feature, `Random` structs can be generated but must be supplied
with a seed to work properly.

## Features

- **strip**: Removes some strings from compilation, mostly regarding error messages.
             Messages will instead reflect their error code or index. This should
             not be used while debugging. (Enables `strip` on the `xrmt-io` dependency).
- **sys**  : Enables system-depdent random generation, removing the need for setting
             seeds on new `Random` structs.
- **crypt**: Enables the `xrmt-crypt` crate for *nix systems.

## Default Features

- **sys**

## Dependencies

- **libc** (*nix only)
- **xrmt-io**
- **xrmt-crypt** (*nix only)
- **xrmt-winapi** (Windows only)
