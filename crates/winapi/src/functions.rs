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
#![cfg(target_family = "windows")]
#![allow(non_snake_case)]

extern crate core;

use core::arch::asm;

use crate::structs::{Handle, PEB, TEB};

mod advapi32;
mod helpers;
mod kernel32;
mod ntdll;
mod simple;
mod winsock;

pub use self::advapi32::*;
pub use self::helpers::*;
pub use self::kernel32::*;
pub use self::ntdll::*;
pub use self::simple::*;
pub use self::winsock::*;

// NOTE(dij): Inlining ASM functions here seems to break stuff.
//            No idea why... *shrug*. Works fine without it.

#[inline(never)]
pub fn SetLastError(e: u32) {
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): See ARM code in 'GetCurrentProcessPEB'.
        //
        // GetLastError
        //   2D E9 00 48     push      {r11,lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   58 6B           ldr         r0, [r3,#0x34]
        //   BD E8 00 88     pop.w     {r11,pc} // RET
        //
        // There's no equivalent 'SetLastError' ARM KernelBase.dll as it
        // does some weird shit in Ntdll.dll.
        //
        // The arm 'STR' instruction is basically the opposite of the 'LDR' instruction
        //  - LDR -> Load to Register  | LDR <DST>, [<SRC>, #<OFFSET>]
        //  - STR -> Store to Register | STR <SRC>, [<DST>, #<OFFSET>]
        //
        // So this modified example /should/ work.
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             str     r2, [r3, #0x34]", in("r2") e
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov x8, x18
             str w0, [x8, #0x68]", in("w0") e
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov                  eax, FS:[0x18]
             mov dword ptr [eax+0x34], ecx", in("ecx") e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov                  rax, qword ptr GS:[0x30]
             mov dword ptr [rax+0x68], ecx", in("ecx") e
        );
    }
}
#[inline(never)]
pub fn GetLastError() -> u32 {
    let e: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // See ARM code in 'GetCurrentProcessPEB'.
        //
        // GetLastError
        //   2D E9 00 48     push      {r11, lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   58 6B           ldr         r0, [r3,#0x34]
        //   BD E8 00 88     pop.w     {r11, pc} // <- RET, don't need
        //
        asm!(
            "push  {{r11, lr}}
             mov     r11, sp
             mrc     p15, 0x0, r3, cr13, cr0, 0x2
             ldr      {}, [r3, #0x34]", out(reg) e
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:w}, [x8, #0x68]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x34]", out(reg) e
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x68]", out(reg) e
        );
    }
    e
}
#[inline(never)]
pub fn GetProcessHeap() -> Handle {
    let mut h;
    #[cfg(target_arch = "arm")]
    unsafe {
        // See ARM code in 'GetCurrentProcessPEB'.
        //
        // GetProcessHeap
        //   2D E9 00 48     push      {r11,lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   1B 6B           ldr         r3, [r3,#0x30]
        //   98 69           ldr         r0, [r3,#0x18]
        //   BD E8 00 88     pop.w     {r11,pc} // <- RET, don't need
        //
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     r3, [r3, #0x30]
             ldr     {}, [r3, #0x18]", out(reg) h
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov x8, x18
             ldr x8, [x8, #0x60]
             ldr {}, [x8, #0x30]", out(reg) h
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov eax, dword ptr [eax+0x30]
             mov  {}, dword ptr [eax+0x18]", out(reg) h
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, qword ptr GS:[0x60]
             mov  {}, qword ptr [rax+0x30]", out(reg) h
        );
    }
    Handle::new(h)
}
#[inline(never)]
pub fn GetCurrentThreadID() -> u32 {
    let i: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // See ARM code in 'GetCurrentProcessPEB'.
        //
        // GetCurrentThreadId
        //   2D E9 00 48     push      {r11 ,lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   58 6A           ldr         r0, [r3,#0x24] // <- r0 is the return
        //   BD E8 00 88     pop.w     {r11, pc} // <- RET, don't need
        //
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     {}, [r3, #0x24]", out(reg) i
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:w}, [x8, #0x48]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x24]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x48]", out(reg) i
        );
    }
    i
}
#[inline(never)]
pub fn GetCurrentProcessID() -> u32 {
    let i: u32;
    #[cfg(target_arch = "arm")]
    unsafe {
        // See ARM code in 'GetCurrentProcessPEB'.
        //
        // GetCurrentProcessId
        //   2D E9 00 48     push      {r11, lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   18 6A           ldr         r0, [r3,#0x20] // <- r0 is the return
        //   BD E8 00 88     pop.w     {r11, pc} // <- RET, don't need
        //
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     {}, [r3, #0x20]", out(reg) i
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov    x8, x18
             ldr {0:w}, [x8, #0x40]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov   eax, FS:[0x18]
             mov {0:e}, dword ptr [eax+0x20]", out(reg) i
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov   rax, qword ptr GS:[0x30]
             mov {0:e}, dword ptr [rax+0x40]", out(reg) i
        );
    }
    i
}
#[inline(never)]
pub fn GetCurrentTEB<'a>() -> &'a TEB<'a> {
    let t: *mut TEB;
    #[cfg(target_arch = "arm")]
    unsafe {
        // See ARM code in 'GetCurrentProcessPEB'.
        // 99% sure we need to write "ldr" as "[r3, #hex]" and not "[r3]" when
        // taking offset 0.
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     {}, [r3, #0x0]", out(reg) t
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("mov {}, x18", out(reg) t);
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!("mov {}, FS:[0x18]", out(reg) t);
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("mov {}, qword ptr GS:[0x30]", out(reg) t);
    }
    unsafe { &*t }
}
#[inline(never)]
pub fn GetCurrentProcessPEB<'a>() -> &'a PEB<'a> {
    let p: *mut PEB;
    #[cfg(target_arch = "arm")]
    unsafe {
        // NOTE(dij): I'm not 100% sure if this works correctly. For the most
        //            part Windows or ARM has a very limited set of machines
        //            _currently_ supported for ARM and is focusing more on
        //            AARCH64.
        //            Also, why is ARM opcode so different than AARCH64?
        //
        // RtlGetCurrentPeb
        //   2D E9 00 48     push      {r11, lr}
        //   EB 46           mov        r11, sp
        //   1D EE 50 3F     mrc        p15, 0x0, r3, cr13, cr0, 0x2
        //   18 6B           ldr         r0, [r3,#0x30] // <- r0 is the return
        //   BD E8 00 88     pop.w     {r11, pc} // <- RET, don't need
        asm!(
            "push {{r11, lr}}
             mov    r11, sp
             mrc    p15, 0x0, r3, cr13, cr0, 0x2
             ldr     {}, [r3, #0x30]", out(reg) p
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "mov x8, x18
             ldr {}, [x8, #0x60]", out(reg) p
        );
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
            "mov eax, FS:[0x18]
             mov  {}, dword ptr [eax+0x30]", out(reg) p
        );
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
            "mov rax, qword ptr GS:[0x30]
             mov  {}, qword ptr [rax+0x60]", out(reg) p
        );
    }
    unsafe { &*p }
}
