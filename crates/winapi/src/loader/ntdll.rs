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

extern crate xrmt_bugtrack;
extern crate xrmt_winapi_fnv;

crate::dll!(
    Ntdll,
    NTDLL,
    ntdll,
    DbgBreakPoint, // TODO(dij):
    EtwEventRegister,
    EtwEventWrite,
    EtwEventWriteFull,
    EtwNotificationRegister,
    LdrGetDllHandleEx,
    LdrGetProcedureAddress,
    LdrLoadDll,
    LdrUnloadDll,
    NtAdjustPrivilegesToken,
    NtAllocateVirtualMemory,
    NtCancelIoFile,
    NtCancelIoFileEx,
    NtCancelTimer,
    NtClose,
    NtCreateEvent,
    NtCreateFile,
    NtCreateIoCompletion,
    NtCreateKey,
    NtCreateKeyedEvent,
    NtCreateMailslotFile,
    NtCreateMutant,
    NtCreateNamedPipeFile,
    NtCreateSection,
    NtCreateSemaphore,
    NtCreateThreadEx,
    NtCreateTimer,
    NtDelayExecution,
    NtDeleteKey,
    NtDeleteValueKey,
    NtDeviceIoControlFile,
    NtDuplicateObject,
    NtDuplicateToken,
    NtEnumerateKey,
    NtEnumerateValueKey,
    NtFlushBuffersFile,
    NtFlushInstructionCache,
    NtFlushKey,
    NtFreeVirtualMemory,
    NtFsControlFile,
    NtImpersonateThread,
    NtLockFile,
    NtMapViewOfSection,
    NtOpenEvent,
    NtOpenKey,
    NtOpenKeyedEvent,
    NtOpenIoCompletion,
    NtOpenMutant,
    NtOpenProcess,
    NtOpenProcessToken,
    NtOpenSemaphore,
    NtOpenThread,
    NtOpenThreadToken,
    NtOpenTimer,
    NtProtectVirtualMemory,
    NtPulseEvent,
    NtQueryDirectoryFile,
    NtQueryEvent,
    NtQueryInformationFile,
    NtQueryInformationProcess,
    NtQueryInformationThread,
    NtQueryInformationToken,
    NtQueryIoCompletion,
    NtQueryVirtualMemory,
    NtQueryKey,
    NtQueryMutant,
    NtQueryObject,
    NtQuerySemaphore,
    NtQuerySystemInformation,
    NtQuerySystemInformationEx,
    NtQueryTimer,
    NtQueryValueKey,
    NtReadFile,
    NtReadVirtualMemory,
    NtReleaseKeyedEvent,
    NtReleaseMutant,
    NtReleaseSemaphore,
    NtRemoveIoCompletion,
    NtResetEvent,
    NtResumeProcess,
    NtResumeThread,
    NtSetEvent,
    NtSetInformationFile,
    NtSetInformationObject,
    NtSetInformationProcess,
    NtSetInformationThread,
    NtSetInformationToken,
    NtSetTimer,
    NtSetValueKey,
    NtSuspendProcess,
    NtSuspendThread,
    NtTerminateProcess,
    NtTerminateThread,
    NtTraceControl, // TODO(dij):
    NtTraceEvent,
    NtUnlockFile,
    NtUnmapViewOfSection,
    NtWaitForKeyedEvent,
    NtWaitForMultipleObjects,
    NtWaitForSingleObject,
    NtWow64AllocateVirtualMemory64,
    NtWow64QueryInformationProcess64,
    NtWow64ReadVirtualMemory64,
    NtWow64WriteVirtualMemory64,
    NtWriteFile,
    NtWriteVirtualMemory,
    NtYieldExecution,
    RtlAllocateHeap,
    RtlCreateHeap,
    RtlDestroyHeap,
    RtlFindMessage,
    RtlFreeHeap,
    RtlReAllocateHeap,
    RtlSetCurrentDirectory_U,
    RtlSetEnvironmentVar
);

// TODO(dij): Do we need these anymore?
//  RtlCopyMappedMemory
//  RtlLengthSecurityDescriptor
//  RtlMoveMemory
