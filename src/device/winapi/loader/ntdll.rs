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
#![cfg(target_family = "windows")]
#![allow(non_snake_case, non_upper_case_globals)]

use crate::device::winapi::loader::{Function, Loader};

pub(crate) static LdrLoadDll: Function = Function::new();
pub(crate) static LdrUnloadDll: Function = Function::new();
pub(crate) static LdrGetDllHandleEx: Function = Function::new();
pub(crate) static LdrGetProcedureAddress: Function = Function::new();

pub(crate) static DbgBreakPoint: Function = Function::new();

pub(crate) static RtlFreeHeap: Function = Function::new();
pub(crate) static RtlCreateHeap: Function = Function::new();
pub(crate) static RtlDestroyHeap: Function = Function::new();
pub(crate) static RtlAllocateHeap: Function = Function::new();
pub(crate) static RtlReAllocateHeap: Function = Function::new();

pub(crate) static RtlMoveMemory: Function = Function::new();
pub(crate) static RtlCopyMappedMemory: Function = Function::new();

pub(crate) static RtlSetEnvironmentVar: Function = Function::new();
pub(crate) static RtlSetCurrentDirectory: Function = Function::new();

pub(crate) static RtlGetNtVersionNumbers: Function = Function::new();
pub(crate) static RtlWow64GetProcessMachines: Function = Function::new();
pub(crate) static RtlLengthSecurityDescriptor: Function = Function::new();

pub(crate) static EtwEventWrite: Function = Function::new();
pub(crate) static EtwEventRegister: Function = Function::new();
pub(crate) static EtwEventWriteFull: Function = Function::new();
pub(crate) static EtwNotificationRegister: Function = Function::new();

pub(crate) static NtOpenKey: Function = Function::new();
pub(crate) static NtFlushKey: Function = Function::new();
pub(crate) static NtQueryKey: Function = Function::new();
pub(crate) static NtCreateKey: Function = Function::new();
pub(crate) static NtDeleteKey: Function = Function::new();
pub(crate) static NtSetValueKey: Function = Function::new();
pub(crate) static NtEnumerateKey: Function = Function::new();
pub(crate) static NtQueryValueKey: Function = Function::new();
pub(crate) static NtDeleteValueKey: Function = Function::new();
pub(crate) static NtEnumerateValueKey: Function = Function::new();

pub(crate) static NtClose: Function = Function::new();
pub(crate) static NtReadFile: Function = Function::new();
pub(crate) static NtWriteFile: Function = Function::new();
pub(crate) static NtCreateFile: Function = Function::new();
pub(crate) static NtCancelIoFile: Function = Function::new();
pub(crate) static NtCancelIoFileEx: Function = Function::new();
pub(crate) static NtFlushBuffersFile: Function = Function::new();
pub(crate) static NtSetInformationFile: Function = Function::new();
pub(crate) static NtQueryDirectoryFile: Function = Function::new();
pub(crate) static NtQueryInformationFile: Function = Function::new();

pub(crate) static NtWaitForSingleObject: Function = Function::new();
pub(crate) static NtWaitForMultipleObjects: Function = Function::new();

pub(crate) static NtCreateSection: Function = Function::new();
pub(crate) static NtMapViewOfSection: Function = Function::new();
pub(crate) static NtUnmapViewOfSection: Function = Function::new();

pub(crate) static NtFreeVirtualMemory: Function = Function::new();
pub(crate) static NtReadVirtualMemory: Function = Function::new();
pub(crate) static NtWriteVirtualMemory: Function = Function::new();
pub(crate) static NtProtectVirtualMemory: Function = Function::new();
pub(crate) static NtFlushInstructionCache: Function = Function::new();
pub(crate) static NtAllocateVirtualMemory: Function = Function::new();

pub(crate) static NtSetEvent: Function = Function::new();
pub(crate) static NtOpenEvent: Function = Function::new();
pub(crate) static NtResetEvent: Function = Function::new();
pub(crate) static NtQueryEvent: Function = Function::new();
pub(crate) static NtCreateEvent: Function = Function::new();

pub(crate) static NtOpenMutant: Function = Function::new();
pub(crate) static NtQueryMutant: Function = Function::new();
pub(crate) static NtCreateMutant: Function = Function::new();
pub(crate) static NtReleaseMutant: Function = Function::new();

pub(crate) static NtOpenSemaphore: Function = Function::new();
pub(crate) static NtQuerySemaphore: Function = Function::new();
pub(crate) static NtCreateSemaphore: Function = Function::new();
pub(crate) static NtReleaseSemaphore: Function = Function::new();

pub(crate) static NtSetTimer: Function = Function::new();
pub(crate) static NtOpenTimer: Function = Function::new();
pub(crate) static NtQueryTimer: Function = Function::new();
pub(crate) static NtCancelTimer: Function = Function::new();
pub(crate) static NtCreateTimer: Function = Function::new();

pub(crate) static NtFsControlFile: Function = Function::new();
pub(crate) static NtCreateMailslotFile: Function = Function::new();
pub(crate) static NtDeviceIoControlFile: Function = Function::new();
pub(crate) static NtCreateNamedPipeFile: Function = Function::new();

pub(crate) static NtQueryObject: Function = Function::new();
pub(crate) static NtDuplicateObject: Function = Function::new();
pub(crate) static NtSetInformationObject: Function = Function::new();

pub(crate) static NtTraceEvent: Function = Function::new();
pub(crate) static NtTraceControl: Function = Function::new();

pub(crate) static NtYieldExecution: Function = Function::new();
pub(crate) static NtDelayExecution: Function = Function::new();
pub(crate) static NtQuerySystemInformation: Function = Function::new();

pub(crate) static NtOpenThread: Function = Function::new();
pub(crate) static NtResumeThread: Function = Function::new();
pub(crate) static NtSuspendThread: Function = Function::new();
pub(crate) static NtCreateThreadEx: Function = Function::new();
pub(crate) static NtTerminateThread: Function = Function::new();
pub(crate) static NtImpersonateThread: Function = Function::new();
pub(crate) static NtSetInformationThread: Function = Function::new();
pub(crate) static NtQueryInformationThread: Function = Function::new();

pub(crate) static NtOpenProcess: Function = Function::new();
pub(crate) static NtResumeProcess: Function = Function::new();
pub(crate) static NtSuspendProcess: Function = Function::new();
pub(crate) static NtTerminateProcess: Function = Function::new();
pub(crate) static NtSetInformationProcess: Function = Function::new();
pub(crate) static NtQueryInformationProcess: Function = Function::new();

pub(crate) static NtDuplicateToken: Function = Function::new();
pub(crate) static NtOpenThreadToken: Function = Function::new();
pub(crate) static NtOpenProcessToken: Function = Function::new();
pub(crate) static NtSetInformationToken: Function = Function::new();
pub(crate) static NtAdjustTokenPrivileges: Function = Function::new();
pub(crate) static NtQueryInformationToken: Function = Function::new();

#[cfg(not(target_pointer_width = "64"))]
mod wow {
    use crate::device::winapi::loader::Function;

    pub(crate) static NtWow64ReadVirtualMemory64: Function = Function::new();
    pub(crate) static NtWow64WriteVirtualMemory64: Function = Function::new();
    pub(crate) static NtWow64AllocateVirtualMemory64: Function = Function::new();
    pub(crate) static NtWow64QueryInformationProcess64: Function = Function::new();
}

#[cfg(not(target_pointer_width = "64"))]
pub(crate) use self::wow::*;

pub(super) static DLL: Loader = Loader::new(|ntdll| {
    ntdll.proc(&LdrLoadDll, 0xB6936493);
    ntdll.proc(&LdrUnloadDll, 0x630D7790);
    ntdll.proc(&LdrGetDllHandleEx, 0xA0B1C41C);
    ntdll.proc(&LdrGetProcedureAddress, 0x448176AE);

    ntdll.proc(&DbgBreakPoint, 0x6861210F);

    ntdll.proc(&RtlFreeHeap, 0xBC880A2D);
    ntdll.proc(&RtlCreateHeap, 0xA1846AB);
    ntdll.proc(&RtlDestroyHeap, 0x167E8613);
    ntdll.proc(&RtlAllocateHeap, 0x50AA445E);
    ntdll.proc(&RtlReAllocateHeap, 0xA51D1975);

    ntdll.proc(&RtlMoveMemory, 0xA0CE107B);
    ntdll.proc(&RtlCopyMappedMemory, 0x381752E6);

    ntdll.proc(&RtlSetEnvironmentVar, 0xE8474F1D);
    ntdll.proc(&RtlSetCurrentDirectory, 0x366CCFB7);

    ntdll.proc(&RtlGetNtVersionNumbers, 0xD476F98B);
    ntdll.proc(&RtlWow64GetProcessMachines, 0x982D219D);
    ntdll.proc(&RtlLengthSecurityDescriptor, 0xF5677F7C);

    ntdll.proc(&EtwEventWrite, 0xD32A6690);
    ntdll.proc(&EtwEventRegister, 0xC0B4D94C);
    ntdll.proc(&EtwEventWriteFull, 0xAC8A097);
    ntdll.proc(&EtwNotificationRegister, 0x7B7F821F);

    ntdll.proc(&NtOpenKey, 0x8AB6D330);
    ntdll.proc(&NtFlushKey, 0x617ECEC);
    ntdll.proc(&NtQueryKey, 0x8FDAE50E);
    ntdll.proc(&NtCreateKey, 0xFCBDD5CC);
    ntdll.proc(&NtDeleteKey, 0x902F876B);
    ntdll.proc(&NtSetValueKey, 0x4719A915);
    ntdll.proc(&NtEnumerateKey, 0xB5BB2E94);
    ntdll.proc(&NtQueryValueKey, 0x5BCE7235);
    ntdll.proc(&NtDeleteValueKey, 0x6E2425FE);
    ntdll.proc(&NtEnumerateValueKey, 0xAC63F74B);

    ntdll.proc(&NtClose, 0x36291E41);
    ntdll.proc(&NtReadFile, 0xB9FEE621);
    ntdll.proc(&NtWriteFile, 0x465736BE);
    ntdll.proc(&NtCreateFile, 0xB6FCA7E9);
    ntdll.proc(&NtCancelIoFile, 0xF402EB27);
    ntdll.proc(&NtCancelIoFileEx, 0xD4909C18);
    ntdll.proc(&NtFlushBuffersFile, 0x78D6E042);
    ntdll.proc(&NtSetInformationFile, 0xA87A5F13);
    ntdll.proc(&NtQueryDirectoryFile, 0xE0DF29FC);
    ntdll.proc(&NtQueryInformationFile, 0x645779B3);

    ntdll.proc(&NtWaitForSingleObject, 0x46D9033C);
    ntdll.proc(&NtWaitForMultipleObjects, 0x5DF74043);

    ntdll.proc(&NtCreateSection, 0x40A2511C);
    ntdll.proc(&NtMapViewOfSection, 0x704A2F2C);
    ntdll.proc(&NtUnmapViewOfSection, 0x19B022D);

    ntdll.proc(&NtFreeVirtualMemory, 0x8C399853);
    ntdll.proc(&NtReadVirtualMemory, 0xB572F955);
    ntdll.proc(&NtWriteVirtualMemory, 0x2012F428);
    ntdll.proc(&NtProtectVirtualMemory, 0xD86AFCB8);
    ntdll.proc(&NtFlushInstructionCache, 0xEFB80179);
    ntdll.proc(&NtAllocateVirtualMemory, 0x46D22D36);

    ntdll.proc(&NtSetEvent, 0x5E5D5E5B);
    ntdll.proc(&NtOpenEvent, 0x9378E521);
    ntdll.proc(&NtResetEvent, 0x9D48A168);
    ntdll.proc(&NtQueryEvent, 0xF9C5AFBB);
    ntdll.proc(&NtCreateEvent, 0x1D54CD3D);

    ntdll.proc(&NtOpenMutant, 0xC225442A);
    ntdll.proc(&NtQueryMutant, 0x8A77D844);
    ntdll.proc(&NtCreateMutant, 0xB20542FE);
    ntdll.proc(&NtReleaseMutant, 0xAE4E52E3);

    ntdll.proc(&NtOpenSemaphore, 0x638EBB93);
    ntdll.proc(&NtQuerySemaphore, 0x2F9E591);
    ntdll.proc(&NtCreateSemaphore, 0x545848C7);
    ntdll.proc(&NtReleaseSemaphore, 0x2D0F8080);

    ntdll.proc(&NtSetTimer, 0xEB3A7C98);
    ntdll.proc(&NtOpenTimer, 0x95FFA4C2);
    ntdll.proc(&NtQueryTimer, 0x86A2CEF8);
    ntdll.proc(&NtCancelTimer, 0x41CD8D70);
    ntdll.proc(&NtCreateTimer, 0x48D21FDE);

    ntdll.proc(&NtFsControlFile, 0x6EDA0D4F);
    ntdll.proc(&NtCreateMailslotFile, 0xA7BD74CC);
    ntdll.proc(&NtDeviceIoControlFile, 0x5D0C9026);
    ntdll.proc(&NtCreateNamedPipeFile, 0x1273C7E4);

    ntdll.proc(&NtQueryObject, 0x2D946558);
    ntdll.proc(&NtDuplicateObject, 0xAD2BC047);
    ntdll.proc(&NtSetInformationObject, 0xF604613C);

    ntdll.proc(&NtTraceEvent, 0x89F984CE);
    ntdll.proc(&NtTraceControl, 0x3DD363A1);

    ntdll.proc(&NtYieldExecution, 0xD4C349DC);
    ntdll.proc(&NtDelayExecution, 0xF931557A);
    ntdll.proc(&NtQuerySystemInformation, 0x337C7C64);

    ntdll.proc(&NtOpenThread, 0x7319665F);
    ntdll.proc(&NtResumeThread, 0xA6F798EA);
    ntdll.proc(&NtSuspendThread, 0x9D419019);
    ntdll.proc(&NtCreateThreadEx, 0x8E6261C);
    ntdll.proc(&NtTerminateThread, 0x18157A24);
    ntdll.proc(&NtImpersonateThread, 0x12724B12);
    ntdll.proc(&NtSetInformationThread, 0x5F74B08D);
    ntdll.proc(&NtQueryInformationThread, 0x115412D);

    ntdll.proc(&NtOpenProcess, 0x57367582);
    ntdll.proc(&NtResumeProcess, 0xB5333DBD);
    ntdll.proc(&NtSuspendProcess, 0x8BD95BF8);
    ntdll.proc(&NtTerminateProcess, 0xB3AC5173);
    ntdll.proc(&NtSetInformationProcess, 0x77CDB26C);
    ntdll.proc(&NtQueryInformationProcess, 0xC88AB8C);

    ntdll.proc(&NtDuplicateToken, 0x7A75D3A1);
    ntdll.proc(&NtOpenThreadToken, 0x82EEAAFE);
    ntdll.proc(&NtOpenProcessToken, 0xB2CA3641);
    ntdll.proc(&NtSetInformationToken, 0x43623A4);
    ntdll.proc(&NtAdjustTokenPrivileges, 0x6CCF6931);
    ntdll.proc(&NtQueryInformationToken, 0x63C176C4);

    #[cfg(not(target_pointer_width = "64"))]
    {
        ntdll.proc(&NtWow64ReadVirtualMemory64, 0x24CC52C4);
        ntdll.proc(&NtWow64WriteVirtualMemory64, 0x7626B3AB);
        ntdll.proc(&NtWow64AllocateVirtualMemory64, 0xEB1033F);
        ntdll.proc(&NtWow64QueryInformationProcess64, 0x77A90561);
    }
});

#[inline]
pub(crate) fn address() -> usize {
    DLL.address()
}
