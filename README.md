# XrMT: e(X)tensiable (Rust) Malware Toolkit

**THIS IS A SUPER ALPHA VERSION!!!**

This is a Rust version of [XMT](https://github.com/iDigitalFlame/xmt) that attempts
to 1-to-1 feature match the Golang version. _(Some caveats apply)_

## State

Currently this library isn't publushed into crates.io and won't be until we reach
a quasi-stable state. This repo is currently used for building and testing, which
is why we're using `main.rs` instead of `lib.rs`. The `main.rs` is structured for
compatibility between nix and Windows (CRT and MSVC).

Feature wise, most of the Rust "std emulation" features work completely and well.
The next steps are making the "C2" part work. Right now we can just send Oneshot
packets (should be the example in the `main.rs` file).

Currently I'm planning on next:

- Profiles support
- Tasking
- Sessions!!

These will happen sometime **AFTER** DEFCON31!

## Caveats

- The Capabilities flag that XrMT implants report will carry a special "Rust" (0x40000)
  flag that will be used to differentiate implant build types.
- Windows DLL Loader
  - Loads ntdll.dll directly from PEB
  - Only uses Hash/FNV32 loading of DLL functions.
  - Funcmap support is enabled by default and cannot be disabled. (Implants always
    carry this capability flag).
  - Windows implants built with the `pie` feature flag **have no imports**.
- State of certain `const` and `var` objects from Go are no longer Globally mapped.
  - An example would be the `device.Shell` constant is now `device::shell()`.
- All function names _(minus WinAPI names)_ use the Rust **snake_case** naming schema.
- Many Windows functions that were `kernel32` or `advapi32` in Golang have been converted
  to their respective `ntdll` calls in Rust to remove more dependencies.
- Yes, we are compatible all the way back to Windows XP SP0. _(I tested it!)_
- Crypt support is not complete and will be **different** than the Golang version! (ie:
  They won't be compatible).

## Bulding

**ALL XrMT BUILDS REQUIRE THE NIGHTLY CARGO VERSIONS!!**

### Non-Windows

Just `cargo +nightly build` lol.

### Windows

The Windows builds **should** be build using the `x86_64-pc-windows-msvc` or
`i686-pc-windows-msvc` targets. This can be accomplisted on *nix by installing
`clang`, `lld` and using the [msvc-wine](https://github.com/mstorsjo/msvc-wine)
repository.

It is recommended to use `msvc` targets over `gnu` as this will prevent the Microsoft
C-Runtime library from being included. **If you build using GNU you will loose**
**Windows Xp Compatibility!**

The Windows version **does not** use the Rust standard library (std) and instead uses
`core` and `alloc`. This is combined with the "abridged" std library `stx` and `device`
which emulate _most_ of the features for `thread`, `env`, `sync`, `sync/mpsc` (I'm so
proud of this one), `fs`,
`process` and `net`. (See **Library Mappings** for more into).

The Windows version also includes it's own Heap Allocator!

## Library Mappings

Most mappings will transparently redirect on the XrMT side, meaning you can drop-in
replace code and cross-compile without any issues!

_If you really want to use std with Windows, enable it with the `std` feature flag._

-------------------------------------------------------------------
| std          | xrmt                                             |
|--------------|--------------------------------------------------|
| std/io       | xrmt::util::stx::io                              |
| std/fs       | xrmt::device::fs                                 |
| std/env      | xrmt::device::env                                |
| std/sync     | xrmt::sync                                       |
| std/thread   | xrmt::thread                                     |
| std/net      | xrmt::net                                        |
| std/process  | xrmt::process                                    |
| std/ffi      | xrmt::util::stx::ffi                             |
| std/path     | xrmt::util::stx::ffi _(path & ffi are combined)_ |
| std/preclude | xrmt::util::stx::preclude _(uses 2021 preclude)_ |

## Limitations

- Solaris
  - Can't get the Mac Address for some reason. The OS does not give it to us.
  - Not possible to determine threads for processes.
- MacOS
  - No thread support. We can do it, but you have to have a signed binary with a
    special permission set to read other processes's threads
- NetBSD
  - Thread support works, but it's not super stable as the way it indicates
    Threads is kina weird. THey don't have TIDs but can be enumerated at least
