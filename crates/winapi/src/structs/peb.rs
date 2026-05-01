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

extern crate core;

use core::default::Default;
use core::iter::{FusedIterator, Iterator};
use core::marker::PhantomData;
use core::option::Option::{self, None, Some};
use core::ptr::null_mut;

use crate::info::{is_min_windows_10, is_min_windows_7};
use crate::structs::{ApiSet, EnvironmentBlock, Handle, UnicodeString, GUID};
use crate::PTR_SIZE;

#[repr(C)]
pub struct PEB<'a> {
    pub inheritied_address_space: u8,
    pub read_image_file_exec:     u8,
    pub being_debugged:           u8,
    pub bitflags:                 u8,
    // bitflags has the following
    //  0 - ImageUsesLargePages
    //  1 - IsProtectedProcess
    //  2 - IsImageDynamicallyRelocated
    //  3 - SkipPatchingUser32Forwarders
    //  4 - IsPackagedProcess
    //  5 - IsAppContainer
    //  6 - IsProtectedProcessLight
    //  7 - IsLongPathAwareProcess
    pub mutant:                   usize,
    pub image_base_address:       Handle,
    pub ldr:                      *mut LoaderTable,
    pub process_parameters:       *mut ProcessParams<'a>,
    pub sub_system_data:          usize,
    pub process_heap:             Handle,
    pad1:                         usize,
    pub alt_thunk_list_ptr:       usize,
    pad2:                         usize,
    pub cross_process_flags:      u32,
    pub kernel_callback_table:    usize, // INVESTIGATE
    pad3:                         u32,
    pub alt_thunk_list_ptr32:     u32,
    pub api_map:                  *mut ApiSet, // API_MAP SET
    pub tls_ex_count:             u32,
    pub tls_bitmap:               usize,
    pub tls_bitmap_bits:          [u32; 2],
    pub readonly_shared_base:     usize,
    pub hotpatch_into:            usize,
    pad4:                         usize,
    pub ansi_code_page:           usize,
    pub oem_code_page:            usize,
    pub unicode_case_table:       usize,
    pub number_of_processors:     u32,
    pub nt_global_flag:           u32,
    pad5:                         u64,
    pad6:                         [usize; 4],
    pub number_of_heaps:          u32,
    pub max_heaps:                u32,
    pub process_heaps:            usize,
    pad7:                         [usize; 2],
    pad8:                         u32,
    pub loader_lock:              usize,
    pub os_major_version:         u32,
    pub os_minor_version:         u32,
    pub os_build:                 u16,
    pub os_csd_version:           u16, // AKA (SP Major << 8) | (SP Minor)
    pub os_platform_id:           u32,
    pub image_subsystem:          u32,
    pub image_subsystem_major:    u32,
    pub image_subsystem_minor:    u32,
    pub process_affinity_mask:    usize,
    pad9:                         [u32; if PTR_SIZE == 8 { 60 } else { 34 }],
    pad10:                        usize,
    pub tls_ex_bitmap:            usize,
    pub tls_ex_bitmap_bits:       [u32; 32],
    pub session_id:               u32,
    pub appcompat_flags:          u64,
    pub appcompat_flags_user:     u64,
    pub shim_data:                usize,
    pub app_compat:               usize,
    pub csd_version:              UnicodeString<'a>,
    pad12:                        [usize; 4],
    pub min_stack_commit:         usize,
    pad13:                        [usize; 3],
    pad14:                        [u32; 4],
    pad15:                        u32,
    pad16:                        [usize; 3],
    pub image_header_hash:        usize,
}
#[repr(C)]
pub struct TEB<'a> {
    pub exception_list:      usize,
    pub stack_base:          usize,
    pub stack_limit:         usize,
    pub subsystem_teb:       usize,
    pub fiber_data_or_ver:   usize,
    pub user_ptr:            usize,
    pub self_ptr:            *mut TEB<'a>,
    pad1:                    usize,
    pub client_id:           ClientID,
    pub active_rpc_handle:   usize,
    pub tls_storage:         usize,
    pub peb:                 *mut PEB<'a>,
    pub last_error:          u32,
    pad2:                    u32,
    pub csr_client_thread:   usize,
    pub win32_thread_info:   usize, // Is a Kernel Pointer??
    pub user32_reserved:     [u32; 26],
    pub user_reserved:       [u32; 5],
    pub wow32_reserved:      usize,
    pub current_locale:      u32,
    pad3:                    u32,
    pub system_reserved:     [usize; 54],
    pub exception_code:      u32,
    pub activation_stack:    usize,
    pad4:                    [u8; 32],
    pad5:                    [u8; 8 - ((PTR_SIZE / 8) * 8)],
    pub gdi_teb_batch:       [u32; 312], // 0x01D4 | 0x02F0
    pad6:                    [u8; (PTR_SIZE / 8) * 8],
    pub real_client_id:      ClientID, // 0x06B4 | 0x07D8
    pub gdi_client_handle:   Handle,
    pub gdi_client_pid:      u32,
    pub gdi_client_tid:      u32,
    pub gdi_client_local:    usize,
    pub win32_client_info:   [usize; 62],
    pub gdi_dispatch:        [usize; 233],
    pad7:                    [usize; 29],
    pad8:                    usize,
    pub gl_section_info:     usize,
    pub gl_section:          usize,
    pub gl_table:            usize,
    pub gl_current_rc:       usize,
    pub gl_context:          usize,
    pub last_nt_status:      u32,
    pub static_str:          UnicodeString<'a>,
    pub static_str_buffer:   [u16; 261],
    pub deallocation_stack:  usize,
    pub tls_slots:           [usize; 64],
    pub tls_links:           LoaderList,
    pub vdm:                 usize,
    pub reserved_nt_rpc:     usize,
    pub dbg_ss_reserved:     [Handle; 2],
    pub hard_error_mode:     u32,
    pad10:                   [u8; (PTR_SIZE / 8) * 4],
    pub instrumentation:     [usize; 9],
    pad11:                   [usize; (PTR_SIZE / 8) * 2],
    pub activity_id:         GUID,
    pub sub_process_tag:     usize,
    pub etw_local_data:      usize,
    pub etw_trace_data:      usize,
    pub winsock_data:        usize,
    pub gdi_batch_count:     u32,
    pub ideal_process:       u32,
    pub guaranteed_stack:    u32,
    pub reserved_for_pref:   usize,
    pub reserved_for_ole:    usize,
    pub waiting_on_loader:   u32,
    pad12:                   [usize; 2],
    pub thread_pool_data:    usize,
    pub tls_expansion_slots: usize,
    pad13:                   [usize; (PTR_SIZE / 8) * 2],
    pub mui_generation:      u32,
    pub is_impersonating:    u32,
    pub nls_cache:           usize,
    pub shim_data:           usize,
    pub heap_data:           u32,
    pad14:                   u16,
    pad15:                   usize,
    pub active_frame:        usize,
    pub fls_data:            usize,
    pad16:                   u32,
    pad17:                   [usize; 3],
    pad18:                   u32,
    pub cross_teb_flags:     u16,
    pub same_teb_flags:      u16,
    pad19:                   [usize; 3],
    pub lock_count:          u32,
    pub wow_teb_offset:      u32,
}
#[repr(C)]
pub struct ClientID {
    pub process: usize,
    pub thread:  usize,
}
#[repr(C)]
pub struct LoaderList {
    pub f_link: *mut LoaderList,
    pub b_link: *mut LoaderList,
}
#[repr(C)]
pub struct LoaderNode {
    pub left:   *const LoaderNode,
    pub right:  *const LoaderNode,
    pub parent: usize,
}
#[repr(C)]
pub struct LoaderTable {
    pub length:               u32,
    pub initialized:          u8,
    pad:                      usize,
    pub modules_load_order:   LoaderList, // InLoadOrderModuleList
    pub modules_memory_order: LoaderList, // InMemoryOrderModuleList
    pub modules_init_order:   LoaderList, // InInitializationOrderModuleList
}
#[repr(C)]
pub struct LoaderDiagNode {
    pub modules:              LoaderList,
    pub list_service_tag:     usize,
    pub count_load:           u32,
    pub count_load_unloading: u32,
    pub lowest_link:          u32,
    pub dependencies:         usize,
    pub incoming:             usize,
    pub state:                u32, // might be an enum
    pub condense:             usize,
    pub preorder:             u32,
}
pub struct LoaderIter<'a> {
    cur:   *mut LoaderList,
    head:  *mut LoaderList,
    order: ModuleOrder,
    _p:    PhantomData<&'a LoaderEntry<'a>>,
}
#[repr(C)]
pub struct ThreadBasicInfo {
    pub exit_status: u32,
    pub teb_base:    usize,
    pub client_id:   ClientID,
    pad1:            u64,
    pad2:            u32,
}
#[repr(C)]
pub struct LoaderEntry<'a> {
    pub modules_load_order:   LoaderList, // InLoadOrderModuleList
    pub modules_memory_order: LoaderList, // InMemoryOrderModuleList
    pub modules_init_order:   LoaderList, // InInitializationOrderModuleList
    pub dll_base:             Handle,
    pub entry_point:          usize,
    pub size_image:           u32,
    pub name_full:            UnicodeString<'a>,
    pub name_base:            UnicodeString<'a>,
    pub flags:                u32,
    pub count_load:           i16, // Used < Windows10
    pub tls_index:            u16,
    pub links_hash:           LoaderList,
    pub time_stamp:           u32,
    pub entry_point_ctx:      usize,
    pub patch_info:           usize,
    // Vista - Win10: Adding these to be init'd so we don't crash, but we'll
    // keep 'em empty.
    //
    // The extra size shouldn't matter.
    //
    // WARNING: These may be NULL depending on Windows version.
    pub diag_node:            *mut LoaderDiagNode,
    pub links_modules:        LoaderList,
    pub load_ctx:             usize,
    pub parent_base:          usize,
    pub switch_back_ctx:      usize,
    pub node_base_ptr:        LoaderNode,
    pub node_mapping_info:    LoaderNode,
    pub original_base:        usize,
    pub load_time:            i64,
    pub name_base_hash:       u32,
    /// Load Reason Windows 8+ (>= 6.2)
    ///
    /// - 0:  LoadReasonStaticDependency;
    /// - 1:  LoadReasonStaticForwarderDependency;
    /// - 2:  LoadReasonDynamicForwarderDependency;
    /// - 3:  LoadReasonDelayloadDependency;
    /// - 4:  LoadReasonDynamicLoad;
    /// - 5:  LoadReasonAsImageLoad;
    /// - 6:  LoadReasonAsDataLoad;
    /// - 7:  LoadReasonEnclavePrimary (1709 and higher);
    /// - 8:  LoadReasonEnclaveDependency (1709 and higher);
    /// - -1: LoadReasonUnknown.
    ///
    /// See:
    ///  https://www.geoffchappell.com/studies/windows/km/ntoskrnl/inc/api/ntldr/ldr_data_table_entry.htm
    pub load_reason:          i32,
    pub implicit_path_opts:   u32,
    pub count_reference:      i32,
    pub dependent_flags:      u32,
    pub signing_level:        u32,
}
#[repr(C)]
pub struct ProcessParams<'a> {
    pub max_length:        u32,
    pub length:            u32,
    pub flags:             u32,
    pub debug_flags:       u32,
    pub console:           Handle,
    pub console_flags:     u32,
    pub standard_input:    Handle,
    pub standard_output:   Handle,
    pub standard_error:    Handle,
    pub current_directory: CurrentDirectory<'a>,
    pub dll_path:          UnicodeString<'a>,
    pub image_name:        UnicodeString<'a>,
    pub command_line:      UnicodeString<'a>,
    pub environment:       EnvironmentBlock<'a>,
    pub start_x:           u32,
    pub start_y:           u32,
    pub count_x:           u32,
    pub count_y:           u32,
    pub count_chars_x:     u32,
    pub count_chars_y:     u32,
    pub fill_attribute:    u32,
    pub window_flags:      u32,
    pub show_window_flags: u32,
    pub window_title:      UnicodeString<'a>,
    pub desktop_info:      UnicodeString<'a>,
    pub shell_info:        UnicodeString<'a>,
    pub runtime_data:      UnicodeString<'a>,
    pub directories:       [u8; (12 + PTR_SIZE) * 32],
    pub environment_size:  usize,
    pub package_dep_data:  usize,
    pub process_group_id:  u32,
    pub loader_threads:    u32,
}
#[repr(C)]
pub struct ProcessBasicInfo<'a> {
    pub exit_status:       u32,
    pub peb_base:          *mut PEB<'a>,
    pad1:                  usize,
    pad2:                  u32,
    pub process_id:        usize,
    pub parent_process_id: usize,
}
#[repr(C)]
pub struct CurrentDirectory<'a> {
    pub dos_path: UnicodeString<'a>,
    pub handle:   Handle,
}

enum ModuleOrder {
    Load,
    Memory,
    Init,
}

impl TEB<'_> {
    #[inline]
    pub fn wow_teb(&self) -> Option<usize> {
        if !is_min_windows_7() {
            return None;
        }
        if self.wow_teb_offset == 0 {
            None
        } else {
            Some((self as *const TEB as usize) + (self.wow_teb_offset as usize))
        }
    }
}
impl ClientID {
    #[inline]
    pub const fn thread(i: u32) -> ClientID {
        ClientID {
            thread:  i as usize,
            process: 0usize,
        }
    }
    #[inline]
    pub const fn process(i: u32) -> ClientID {
        ClientID {
            thread:  0usize,
            process: i as usize,
        }
    }
}
impl LoaderList {
    #[inline]
    fn as_load<'a>(&mut self) -> &'a mut LoaderEntry<'a> {
        unsafe { &mut *(self as *mut LoaderList as *mut LoaderEntry) }
    }
    #[inline]
    fn as_init<'a>(&mut self) -> &'a mut LoaderEntry<'a> {
        unsafe { &mut *(((self as *mut LoaderList as usize) - (PTR_SIZE * 4)) as *mut LoaderEntry) }
    }
    #[inline]
    fn as_memory<'a>(&mut self) -> &'a mut LoaderEntry<'a> {
        unsafe { &mut *(((self as *mut LoaderList as usize) - (PTR_SIZE * 2)) as *mut LoaderEntry) }
    }
}
impl LoaderTable {
    #[inline]
    pub fn iter<'a>(&self) -> LoaderIter<'a> {
        self.iter_load()
    }
    #[inline]
    pub fn iter_init<'a>(&self) -> LoaderIter<'a> {
        LoaderIter {
            cur:   self.modules_init_order.f_link,
            head:  self.modules_init_order.f_link,
            order: ModuleOrder::Init,
            _p:    PhantomData,
        }
    }
    #[inline]
    pub fn iter_load<'a>(&self) -> LoaderIter<'a> {
        LoaderIter {
            cur:   self.modules_load_order.f_link,
            head:  self.modules_load_order.f_link,
            order: ModuleOrder::Load,
            _p:    PhantomData,
        }
    }
    #[inline]
    pub fn iter_memory<'a>(&self) -> LoaderIter<'a> {
        LoaderIter {
            cur:   self.modules_memory_order.f_link,
            head:  self.modules_memory_order.f_link,
            order: ModuleOrder::Memory,
            _p:    PhantomData,
        }
    }
}
impl<'a> PEB<'a> {
    #[inline]
    pub fn modules(&self) -> LoaderIter<'a> {
        self.loader_entries().iter_load()
    }
    #[inline]
    pub fn loader_entries(&self) -> &'a LoaderTable {
        unsafe { &*self.ldr }
    }
    #[inline]
    pub fn process_params(&self) -> &'a ProcessParams<'a> {
        unsafe { &*self.process_parameters }
    }

    #[inline]
    pub unsafe fn loader_entries_mut(&self) -> &'a mut LoaderTable {
        unsafe { &mut *self.ldr }
    }
}
impl LoaderEntry<'_> {
    #[inline]
    pub fn is_static(&self) -> bool {
        // This is ok to call here, as it does NOT use any syscalls.
        if is_min_windows_10() {
            // Check both just to be sure.
            self.load_reason < 2
        } else {
            self.count_load == -1
        }
    }
    #[inline]
    pub fn reference_increase(&mut self) {
        if is_min_windows_10() {
            self.count_reference = self.count_reference.saturating_add(1);
        } else {
            self.count_load = self.count_load.saturating_add(1)
        }
    }
    #[inline]
    pub fn reference_decrease(&mut self) {
        if is_min_windows_10() {
            self.count_reference = self.count_reference.saturating_sub(1);
        } else {
            self.count_load = self.count_load.saturating_sub(1)
        }
    }
}

impl Default for ThreadBasicInfo {
    #[inline]
    fn default() -> ThreadBasicInfo {
        ThreadBasicInfo {
            pad1:        0u64,
            pad2:        0u32,
            teb_base:    0usize,
            client_id:   ClientID { process: 0usize, thread: 0usize },
            exit_status: 0u32,
        }
    }
}
impl<'a> Default for ProcessBasicInfo<'a> {
    #[inline]
    fn default() -> ProcessBasicInfo<'a> {
        ProcessBasicInfo {
            pad1:              0usize,
            pad2:              0u32,
            peb_base:          null_mut(),
            process_id:        0usize,
            exit_status:       0u32,
            parent_process_id: 0usize,
        }
    }
}

impl<'a> Iterator for LoaderIter<'a> {
    type Item = &'a mut LoaderEntry<'a>;

    fn next(&mut self) -> Option<&'a mut LoaderEntry<'a>> {
        let x = unsafe { self.cur.as_mut()? };
        if x.b_link == x.f_link && x.f_link == self.head {
            return None;
        }
        let r = match self.order {
            ModuleOrder::Load => {
                let v = x.as_load();
                self.cur = v.modules_load_order.f_link;
                v
            },
            ModuleOrder::Memory => {
                let v = x.as_memory();
                self.cur = v.modules_memory_order.f_link;
                v
            },
            ModuleOrder::Init => {
                let v = x.as_init();
                self.cur = v.modules_init_order.f_link;
                v
            },
        };
        if self.cur == self.head {
            return None;
        }
        Some(r)
    }
}
impl FusedIterator for LoaderIter<'_> {}
