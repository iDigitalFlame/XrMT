#!/usr/bin/bash -e

printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "armv7-linux-androideabi"
cargo check --target armv7-linux-androideabi || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-unknown-linux-ohos"
cargo check --target aarch64-unknown-linux-ohos || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-unknown-linux-musl"
cargo check --target aarch64-unknown-linux-musl || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-unknown-linux-gnu"
cargo check --target aarch64-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "armv7-unknown-linux-gnueabi"
cargo check --target armv7-unknown-linux-gnueabi || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "arm-unknown-linux-musleabi"
cargo check --target arm-unknown-linux-musleabi || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "arm-unknown-linux-gnueabi"
cargo check --target arm-unknown-linux-gnueabi || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "arm-linux-androideabi"
cargo check --target arm-linux-androideabi || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-apple-ios-sim"
cargo check --target aarch64-apple-ios-sim || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-apple-ios"
cargo check --target aarch64-apple-ios || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-unknown-fuchsia"
cargo check --target aarch64-unknown-fuchsia || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "aarch64-apple-darwin"
cargo check --target aarch64-apple-darwin || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "riscv64gc-unknown-linux-gnu"
cargo check --target riscv64gc-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i686-unknown-linux-musl"
cargo check --target i686-unknown-linux-musl || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i686-unknown-linux-gnu"
cargo check --target i686-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i686-unknown-freebsd"
cargo check --target i686-unknown-freebsd || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i686-linux-android"
cargo check --target i686-linux-android || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i586-unknown-linux-musl"
cargo check --target i586-unknown-linux-musl || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "i586-unknown-linux-gnu"
cargo check --target i586-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "powerpc64le-unknown-linux-gnu"
cargo check --target powerpc64le-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "powerpc64-unknown-linux-gnu"
cargo check --target powerpc64-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "powerpc-unknown-linux-gnu"
cargo check --target powerpc-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "loongarch64-unknown-linux-gnu"
cargo check --target loongarch64-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "sparc64-unknown-linux-gnu"
cargo check --target sparc64-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "s390x-unknown-linux-gnu"
cargo check --target s390x-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-apple-ios"
cargo check --target x86_64-apple-ios || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-apple-darwin"
cargo check --target x86_64-apple-darwin || exit 1
# printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-fortanix-unknown-sgx"
# cargo check --target x86_64-fortanix-unknown-sgx || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-linux-android"
cargo check --target x86_64-linux-android || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-freebsd"
cargo check --target x86_64-unknown-freebsd || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-fuchsia"
cargo check --target x86_64-unknown-fuchsia || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-illumos"
cargo check --target x86_64-unknown-illumos || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-linux-gnux32"
cargo check --target x86_64-unknown-linux-gnux32 || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-linux-musl"
cargo check --target x86_64-unknown-linux-musl || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-linux-ohos"
cargo check --target x86_64-unknown-linux-ohos || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-netbsd"
cargo check --target x86_64-unknown-netbsd || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-pc-solaris"
cargo check --target x86_64-pc-solaris || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-apple-darwin"
cargo check --target x86_64-apple-darwin || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-freebsd"
cargo check --target x86_64-unknown-freebsd || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-unknown-linux-gnu"
cargo check --target x86_64-unknown-linux-gnu || exit 1
printf '\e[1;34;40mChecking "%s"\e[1;34;40m..\n' "x86_64-pc-windows-gnu"
cargo check --target x86_64-pc-windows-gnu || exit 1