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

extern crate alloc;
extern crate core;

extern crate xrmt_data;

use alloc::string::String;
use core::clone::Clone;
use core::cmp::{Eq, Ord, PartialEq};
use core::convert::{AsRef, From, TryFrom};
use core::default::Default;
use core::marker::Copy;
use core::mem::{size_of, MaybeUninit};
use core::ops::{Deref, DerefMut, Drop};
use core::option::Option::{self, None, Some};
use core::ptr::{null, null_mut};
use core::result::Result::{self, Err, Ok};
use core::slice::{from_raw_parts, from_raw_parts_mut};

use xrmt_data::text::{utf16_to_string, ToStr};
use xrmt_data::{read_u32, Blob, Slice, VecLike};

use crate::functions::{lsa_close, privilege_accquire, privilege_raw_release, LocalFree};
use crate::structs::{Handle, StringWritable, UnicodeString, WChars, GUID};
use crate::utils::{write_hex, write_hex_u16, write_u32_u16};
use crate::{advapi32, syscall, Win32Error, Win32Result};

#[repr(u8)]
pub enum Privilege {
    SeCreateToken                    = 2,
    SeAssignPrimaryToken             = 3,
    SeLockMemory                     = 4,
    SeIncreaseQuota                  = 5,
    SeMachineAccount                 = 6,
    SeTcb                            = 7,
    SeSecurity                       = 8,
    SeTakeOwnership                  = 9,
    SeLoadDriver                     = 10,
    SeSystemProfile                  = 11,
    SeSystemTime                     = 12,
    SeProfileSingleProcess           = 13,
    SeIncreaseBasePriority           = 14,
    SeCreatePagefile                 = 15,
    SeCreatePermanent                = 16,
    SeBackup                         = 17,
    SeRestore                        = 18,
    SeShutdown                       = 19,
    SeDebug                          = 20,
    SeAudit                          = 21,
    SeSystemEnvironment              = 22,
    SeChangeNotify                   = 23,
    SeRemoteShutdown                 = 24,
    SeUndock                         = 25,
    SeSyncAgent                      = 26,
    SeEnableDelegation               = 27,
    SeManageVolume                   = 28,
    SeImpersonate                    = 29,
    SeCreateGlobal                   = 30,
    SeTrustedCredManAccess           = 31,
    SeRelabel                        = 32,
    SeIncreaseWorkingSet             = 33,
    SeTimeZone                       = 34,
    SeCreateSymbolicLink             = 35,
    SeDelegateSessionUserImpersonate = 36, // Win10+
}
#[repr(u8)]
pub enum JoinState {
    Unknown   = 0,
    Unjoined  = 1,
    Workgroup = 2,
    Domain    = 3,
}
#[repr(u8)]
pub enum WellKnownSID {
    /// S-1-0
    Null                             = 0,
    /// S-1-1-0
    Everyone                         = 1,
    /// S-1-2
    Local                            = 2,
    /// S-1-3-0
    CreatorOwner                     = 3,
    /// S-1-3-1
    CreatorGroup                     = 4,
    /// S-1-3-2
    CreatorOwnerServer               = 5,
    /// S-1-3-3
    CreatorGroupServer               = 6,
    /// S-1-5
    NtAuthority                      = 7,
    /// S-1-5-1
    DialUp                           = 8,
    /// S-1-5-2
    Network                          = 9,
    /// S-1-5-3
    Batch                            = 10,
    /// S-1-5-4
    Interactive                      = 11,
    /// S-1-5-6
    Service                          = 12,
    /// S-1-5-7
    Anonymous                        = 13,
    /// S-1-5-8
    Proxy                            = 14,
    /// S-1-5-9
    EnterpriseControllers            = 15,
    /// S-1-5-10
    ///
    /// Also known as "Self", not named this since it
    /// competes with a Rust keyword.
    PrincipalSelf                    = 16,
    /// S-1-5-11
    AuthenticatedUsers               = 17,
    /// S-1-5-12
    Restricted                       = 18,
    /// S-1-5-13
    TerminalServer                   = 19,
    /// S-1-5-14
    RemoteLogon                      = 20,
    /// S-1-5-18
    LocalSystem                      = 22,
    /// S-1-5-19
    LocalService                     = 23,
    /// S-1-5-20
    NetworkService                   = 24,
    /// S-1-5-32
    Builtin                          = 25,
    /// S-1-5-32-544
    BuiltinAdministrators            = 26,
    /// S-1-5-32-545
    BuiltinUsers                     = 27,
    /// S-1-5-32-546
    BuiltinGuests                    = 28,
    /// S-1-5-32-547
    BuiltinPowerUsers                = 29,
    /// S-1-5-32-548
    BuiltinAccountOperators          = 30,
    /// S-1-5-32-549
    BuiltinSystemOperators           = 31,
    /// S-1-5-32-550
    BuiltinPrintOperators            = 32,
    /// S-1-5-32-551
    BuiltinBackupOperators           = 33,
    /// S-1-5-32-552
    BuiltinReplicator                = 34,
    /// S-1-5-32-554
    BuiltinPreWindows2000CompatibleAccess = 35,
    /// S-1-5-32-555
    BuiltinRemoteDesktopUsers        = 36,
    /// S-1-5-32-556
    BuiltinNetworkConfigOperators    = 37,
    /// S-1-5-X-500
    DomainAdministrator              = 38,
    /// S-1-5-X-501
    DomainGuest                      = 39,
    /// S-1-5-X-502
    DoaminKrbtgt                     = 40,
    /// S-1-5-X-512
    DomainAdmins                     = 41,
    /// S-1-5-X-513
    DomainUsers                      = 42,
    /// S-1-5-X-514
    DomainGuests                     = 43,
    /// S-1-5-X-515
    DomainComputers                  = 44,
    /// S-1-5-X-516
    DomainControllers                = 45,
    /// S-1-5-X-517
    ///
    /// Also known as "Cert Publishers"
    DomainCertAdmins                 = 46,
    /// S-1-5-X-518
    DomainSchemaAdmins               = 47,
    /// S-1-5-X-519
    DomainEnterpriseAdmins           = 48,
    /// S-1-5-X-520
    DomainGroupPolicyAdmins          = 49,
    /// S-1-5-X-553
    DomainRASandIASServers           = 50,
    /// S-1-5-64-10
    NtlmAuthentication               = 51,
    /// S-1-5-64-21
    DigestAuthentication             = 52,
    /// S-1-5-64-14
    SChannelAuthentication           = 53,
    /// S-1-5-15
    ThisOrganization                 = 54,
    /// S-1-5-1000
    OtherOrganization                = 55,
    /// S-1-5-32-557
    BuiltinIncomingForestTrustBuilders = 56,
    /// S-1-5-32-558
    BuiltinPerfMonitoringUsers       = 57,
    /// S-1-5-32-559
    BuiltinPerfLoggingUsers          = 58,
    /// S-1-5-32-560
    BuiltinAuthorizationAccess       = 59,
    /// S-1-5-32-561
    BuiltinTerminalServerLicenseServers = 60,
    /// S-1-5-32-562
    BuiltinDCOMUsers                 = 61,
    /// S-1-5-32-568
    BuiltinIISUsers                  = 62,
    /// S-1-5-17
    BuiltinIUser                     = 63,
    /// S-1-5-32-569
    BuiltinCryptoOperators           = 64,
    /// S-1-16-0
    UntrustedLabel                   = 65,
    /// S-1-16-4096
    LowLabel                         = 66,
    /// S-1-16-8192
    MediumLabel                      = 67,
    /// S-1-16-12288
    HighLabel                        = 68,
    /// S-1-16-16384
    SystemLabel                      = 69,
    /// S-1-5-33
    WriteRestricted                  = 70,
    /// S-1-3-4
    CreatorOwnerRights               = 71,
    /// S-1-5-X-571
    DomainCacheablePrincipalsGroup   = 72,
    /// S-1-5-X-572
    DomainNonCacheablePrincipalsGroup = 73,
    /// S-1-5-22
    BuiltinEnterpriseReadonlyControllers = 74,
    /// S-1-5-X-521
    DomainReadonlyControllers        = 75,
    /// S-1-5-32-573
    BuiltinEventLogReadersGroup      = 76,
    /// S-1-5-X-498
    DomainEnterpriseReadonlyControllers = 77,
    /// S-1-5-32-574
    BuiltinCertSvcDComAccessGroup    = 78,
    /// S-1-16-8448
    MediumPlusLabel                  = 79,
    /// S-1-2-0
    LocalLogon                       = 80,
    /// S-1-2-1
    ConsoleLogon                     = 81,
    /// S-1-5-65-1
    ThisOrganizationCertificate      = 82,
    /// S-1-15-2-0
    ApplicationPackageAuthority      = 83,
    /// S-1-15-2-1
    BuiltinAnyPackage                = 84,
    /// S-1-5-32-575
    BuiltinRDSRemoteAccessServers    = 95,
    /// S-1-5-32-576
    BuiltinRDSEndpointServers        = 96,
    /// S-1-5-32-577
    BuiltinRDSManagementServers      = 97,
    /// S-1-5-84-0-0-0-0-0
    UserModeDrivers                  = 98,
    /// S-1-5-32-578
    BuiltinHyperVAdmins              = 99,
    /// S-1-5-X-522
    DomainCloneableControllers       = 100,
    /// S-1-5-32-579
    BuiltinAccessControlAssistanceOperators = 101,
    /// S-1-5-32-580
    BuiltinRemoteManagementUsers     = 102,
    /// S-1-18-1
    AuthenticationAuthorityAsserted  = 103,
    /// S-1-18-2
    AuthenticationServiceAsserted    = 104,
    /// S-1-5-113
    LocalAccount                     = 105,
    /// S-1-5-114
    LocalAccountAndAdministrator     = 106,
    /// S-1-5-X-525
    DomainProtectedUsers             = 107,
    /// S-1-5-X-503
    DomainDefaultSystemManaged       = 110,
    /// S-1-5-32-581
    BuiltinDefaultSystemManagedGroup = 111,
    /// S-1-5-32-582
    BuiltinStorageReplicaAdmins      = 112,
    /// S-1-5-X-526
    DomainKeyAdmins                  = 113,
    /// S-1-5-X-527
    DomainEnterpriseKeyAdmins        = 114,
    /// S-1-18-4
    AuthenticationKeyTrust           = 115,
    /// S-1-18-5
    AuthenticationKeyPropertyMFA     = 116,
    /// S-1-18-6
    AuthenticationKeyPropertyAttestation = 117,
    /// S-1-18-3
    AuthenticationFreshKeyAuth       = 118,
    /// S-1-5-32-583
    BuiltinDeviceOwners              = 119,
    /// S-1-5-32-584
    BuiltinUserModeHardwareOperators = 120,
    /// S-1-5-32-585
    BuiltinOpenSSHUsers              = 121,
}

#[repr(C)]
pub struct SID {
    pub revision:        u8,
    pub sub_authorities: u8,
    pub identifiers:     [u8; 6],
    pub authorities:     [u32; 2],
}
#[repr(C)]
pub struct LUID {
    pub low:  u32,
    pub high: i32,
}
pub struct InvalidSID(());
#[repr(C)]
pub struct TokenUser<'a> {
    pub user: SIDAndAttributes<'a>,
}
pub struct HeldPrivilege(u8);
#[repr(C)]
pub struct TokenPrivileges {
    pub count:      u32,
    pub privileges: [MaybeUninit<LUIDAndAttributes>; 10],
}
#[repr(transparent)]
pub struct LsaHandle(Handle);
#[repr(transparent)]
pub struct PSID<'a>(&'a SID);
#[repr(C)]
pub struct LUIDAndAttributes {
    pub luid:       LUID,
    pub attributes: u32,
}
#[repr(C)]
pub struct LsaDomainInfo<'a> {
    pub name:   UnicodeString<'a>,
    pub domain: UnicodeString<'a>,
    pub forest: UnicodeString<'a>,
    pub guid:   GUID,
    pub sid:    PSID<'a>,
}
#[repr(C)]
pub struct SecurityDescriptor {
    pad1: [u8; 2],
    pad2: u16,
    pad3: [usize; 2],
    pad4: [usize; 2],
}
pub struct InvalidPrivilege(());
#[repr(transparent)]
pub struct LsaPointer<T>(*mut T);
#[repr(C)]
pub struct SIDAndAttributes<'a> {
    pub sid:        PSID<'a>,
    pub attributes: u32,
}
#[repr(C)]
pub struct SecurityAttributes<'a> {
    pub length:              u32,
    pub security_descriptor: Option<&'a SecurityDescriptor>,
    pub inherit:             u32,
}
#[repr(C)]
pub struct SecurityQualityOfService {
    pub length:                u32,
    pub impersonation_level:   u32, // is an enum
    pub context_tracking_mode: u8,
    pub effective_only:        u8,
}
#[repr(C)]
pub struct LsaAccountDomainInfo<'a> {
    pub domain: UnicodeString<'a>,
    pub sid:    PSID<'a>,
}

pub type SidSlice = Slice<u8, 190>;
pub type SecQoS<'a> = Option<&'a SecurityQualityOfService>;
pub type SecAttrs<'a> = Option<&'a SecurityAttributes<'a>>;

impl SID {
    #[inline]
    pub const fn empty() -> SID {
        SID {
            revision:        1u8,
            identifiers:     [0u8, 0u8, 0u8, 0u8, 0u8, 0u8],
            authorities:     [0u32, 0u32],
            sub_authorities: 0u8,
        }
    }
    #[inline]
    pub const fn everyone() -> SID {
        SID {
            revision:        1u8,
            identifiers:     [0u8, 0u8, 0u8, 0u8, 0u8, 1],
            authorities:     [0u32, 0u32],
            sub_authorities: 1u8,
        }
    }
    #[inline]
    pub const fn well_known(v: WellKnownSID) -> Option<SID> {
        match v {
            WellKnownSID::Null => Some(SID::known(0, 1, 0, 0)),
            WellKnownSID::Everyone => Some(SID::known(1, 1, 0, 0)),
            //
            WellKnownSID::Local => Some(SID::known(2, 2, 0, 0)),
            WellKnownSID::LocalLogon => Some(SID::known(2, 1, 0, 0)),
            WellKnownSID::ConsoleLogon => Some(SID::known(2, 1, 1, 0)),
            //
            WellKnownSID::CreatorOwner => Some(SID::known(3, 1, 0, 0)),
            WellKnownSID::CreatorGroup => Some(SID::known(3, 1, 1, 0)),
            WellKnownSID::CreatorOwnerServer => Some(SID::known(3, 1, 2, 0)),
            WellKnownSID::CreatorGroupServer => Some(SID::known(3, 1, 3, 0)),
            WellKnownSID::CreatorOwnerRights => Some(SID::known(3, 1, 4, 0)),
            //
            WellKnownSID::NtAuthority => Some(SID::known(5, 0, 0, 0)),
            WellKnownSID::DialUp => Some(SID::known(5, 1, 1, 0)),
            WellKnownSID::Network => Some(SID::known(5, 1, 2, 0)),
            WellKnownSID::Batch => Some(SID::known(5, 1, 3, 0)),
            WellKnownSID::Interactive => Some(SID::known(5, 1, 4, 0)),
            WellKnownSID::Service => Some(SID::known(5, 1, 6, 0)),
            WellKnownSID::Anonymous => Some(SID::known(5, 1, 7, 0)),
            WellKnownSID::Proxy => Some(SID::known(5, 1, 8, 0)),
            WellKnownSID::EnterpriseControllers => Some(SID::known(5, 1, 9, 0)),
            WellKnownSID::PrincipalSelf => Some(SID::known(5, 1, 10, 0)),
            WellKnownSID::AuthenticatedUsers => Some(SID::known(5, 1, 11, 0)),
            WellKnownSID::Restricted => Some(SID::known(5, 1, 12, 0)),
            WellKnownSID::TerminalServer => Some(SID::known(5, 1, 13, 0)),
            WellKnownSID::RemoteLogon => Some(SID::known(5, 1, 14, 0)),
            WellKnownSID::ThisOrganization => Some(SID::known(5, 1, 15, 0)),
            WellKnownSID::BuiltinIUser => Some(SID::known(5, 1, 17, 0)),
            WellKnownSID::LocalSystem => Some(SID::known(5, 1, 18, 0)),
            WellKnownSID::LocalService => Some(SID::known(5, 1, 19, 0)),
            WellKnownSID::NetworkService => Some(SID::known(5, 1, 20, 0)),
            WellKnownSID::WriteRestricted => Some(SID::known(5, 1, 33, 0)),
            WellKnownSID::ThisOrganizationCertificate => Some(SID::known(5, 2, 65, 1)),
            WellKnownSID::LocalAccount => Some(SID::known(5, 1, 113, 0)),
            WellKnownSID::LocalAccountAndAdministrator => Some(SID::known(5, 1, 114, 0)),
            WellKnownSID::OtherOrganization => Some(SID::known(5, 1, 1000, 0)),
            //
            WellKnownSID::BuiltinEnterpriseReadonlyControllers => Some(SID::known(5, 1, 22, 0)),
            WellKnownSID::Builtin => Some(SID::known(5, 1, 32, 0)),
            WellKnownSID::BuiltinAdministrators => Some(SID::known(5, 2, 32, 544)),
            WellKnownSID::BuiltinUsers => Some(SID::known(5, 2, 32, 545)),
            WellKnownSID::BuiltinGuests => Some(SID::known(5, 2, 32, 546)),
            WellKnownSID::BuiltinPowerUsers => Some(SID::known(5, 2, 32, 547)),
            WellKnownSID::BuiltinAccountOperators => Some(SID::known(5, 2, 32, 548)),
            WellKnownSID::BuiltinSystemOperators => Some(SID::known(5, 2, 32, 549)),
            WellKnownSID::BuiltinPrintOperators => Some(SID::known(5, 2, 32, 550)),
            WellKnownSID::BuiltinBackupOperators => Some(SID::known(5, 2, 32, 551)),
            WellKnownSID::BuiltinReplicator => Some(SID::known(5, 2, 32, 552)),
            WellKnownSID::BuiltinPreWindows2000CompatibleAccess => Some(SID::known(5, 2, 32, 554)),
            WellKnownSID::BuiltinRemoteDesktopUsers => Some(SID::known(5, 2, 32, 555)),
            WellKnownSID::BuiltinNetworkConfigOperators => Some(SID::known(5, 2, 32, 556)),
            WellKnownSID::BuiltinIncomingForestTrustBuilders => Some(SID::known(5, 2, 32, 557)),
            WellKnownSID::BuiltinPerfMonitoringUsers => Some(SID::known(5, 2, 32, 558)),
            WellKnownSID::BuiltinPerfLoggingUsers => Some(SID::known(5, 2, 32, 559)),
            WellKnownSID::BuiltinAuthorizationAccess => Some(SID::known(5, 2, 32, 560)),
            WellKnownSID::BuiltinTerminalServerLicenseServers => Some(SID::known(5, 2, 32, 561)),
            WellKnownSID::BuiltinDCOMUsers => Some(SID::known(5, 2, 32, 562)),
            WellKnownSID::BuiltinIISUsers => Some(SID::known(5, 2, 32, 568)),
            WellKnownSID::BuiltinCryptoOperators => Some(SID::known(5, 2, 32, 569)),
            WellKnownSID::BuiltinEventLogReadersGroup => Some(SID::known(5, 2, 32, 573)),
            WellKnownSID::BuiltinCertSvcDComAccessGroup => Some(SID::known(5, 2, 32, 574)),
            WellKnownSID::BuiltinRDSRemoteAccessServers => Some(SID::known(5, 2, 32, 575)),
            WellKnownSID::BuiltinRDSEndpointServers => Some(SID::known(5, 2, 32, 576)),
            WellKnownSID::BuiltinRDSManagementServers => Some(SID::known(5, 2, 32, 577)),
            WellKnownSID::BuiltinHyperVAdmins => Some(SID::known(5, 2, 32, 578)),
            WellKnownSID::BuiltinAccessControlAssistanceOperators => Some(SID::known(5, 2, 32, 579)),
            WellKnownSID::BuiltinRemoteManagementUsers => Some(SID::known(5, 2, 32, 580)),
            WellKnownSID::BuiltinDefaultSystemManagedGroup => Some(SID::known(5, 2, 32, 581)),
            WellKnownSID::BuiltinStorageReplicaAdmins => Some(SID::known(5, 2, 32, 582)),
            WellKnownSID::BuiltinDeviceOwners => Some(SID::known(5, 2, 32, 583)),
            WellKnownSID::BuiltinUserModeHardwareOperators => Some(SID::known(5, 2, 32, 584)),
            WellKnownSID::BuiltinOpenSSHUsers => Some(SID::known(5, 2, 32, 585)),
            //
            WellKnownSID::NtlmAuthentication => Some(SID::known(5, 2, 64, 10)),
            WellKnownSID::DigestAuthentication => Some(SID::known(5, 2, 64, 21)),
            WellKnownSID::SChannelAuthentication => Some(SID::known(5, 2, 64, 14)),
            //
            WellKnownSID::ApplicationPackageAuthority => Some(SID::known(15, 2, 2, 0)),
            WellKnownSID::BuiltinAnyPackage => Some(SID::known(15, 2, 2, 1)),
            //
            WellKnownSID::UntrustedLabel => Some(SID::known(16, 1, 0, 0)),
            WellKnownSID::LowLabel => Some(SID::known(16, 1, 4096, 0)),
            WellKnownSID::MediumLabel => Some(SID::known(16, 1, 8192, 0)),
            WellKnownSID::MediumPlusLabel => Some(SID::known(16, 1, 8448, 0)),
            WellKnownSID::HighLabel => Some(SID::known(16, 1, 12288, 0)),
            WellKnownSID::SystemLabel => Some(SID::known(16, 1, 16384, 0)),
            //
            WellKnownSID::AuthenticationAuthorityAsserted => Some(SID::known(18, 1, 1, 0)),
            WellKnownSID::AuthenticationServiceAsserted => Some(SID::known(18, 1, 2, 0)),
            WellKnownSID::AuthenticationFreshKeyAuth => Some(SID::known(18, 1, 3, 0)),
            WellKnownSID::AuthenticationKeyTrust => Some(SID::known(18, 1, 4, 0)),
            WellKnownSID::AuthenticationKeyPropertyMFA => Some(SID::known(18, 1, 5, 0)),
            WellKnownSID::AuthenticationKeyPropertyAttestation => Some(SID::known(18, 1, 6, 0)),
            //
            _ => None,
        }
    }
    #[inline]
    pub const fn well_known_raw(domain: u8, authority: u32) -> SID {
        SID::known(domain, 1, authority, 0)
    }

    #[inline]
    pub fn len(&self) -> u32 {
        8u32.wrapping_add((self.sub_authorities as u32) * 4)
    }
    #[inline]
    pub fn is_base(&self) -> bool {
        self.sub_authorities == 0
    }
    #[inline]
    pub fn is_login(&self) -> bool {
        self.sub_authorities >= 3 && self.is_nt_authority() && unsafe { *self.authorities.get_unchecked(0) == 0x5 }
    }
    #[inline]
    pub fn is_domain(&self) -> bool {
        self.sub_authorities >= 3 && unsafe { *self.authorities.get_unchecked(0) == 0x15 && *self.authorities.get_unchecked(1) > 0 }
    }
    #[inline]
    pub fn is_creator(&self) -> bool {
        self.is(0x3)
    }
    #[inline]
    pub fn is_service(&self) -> bool {
        self.sub_authorities >= 1 && self.is_nt_authority() && unsafe { *self.authorities.get_unchecked(0) == 0x6 || *self.authorities.get_unchecked(0) == 0x50 }
    }
    #[inline]
    pub fn is_built_in(&self) -> bool {
        self.sub_authorities >= 1 && self.is_nt_authority() && unsafe { *self.authorities.get_unchecked(0) == 0x20 }
    }
    #[inline]
    pub fn to_string(&self) -> String {
        unsafe { String::from_utf8_unchecked(self.to_slice().to_vec()) }
    }
    #[inline]
    pub fn to_slice(&self) -> SidSlice {
        let mut b = Slice::default();
        let n = self.into_u8(self.authorities_slice(), &mut b);
        b.truncate(n);
        b
    }
    #[inline]
    pub fn is_capability(&self) -> bool {
        self.sub_authorities >= 1 && self.is(0xF) && unsafe { *self.authorities.get_unchecked(0) == 0x3 }
    }
    #[inline]
    pub fn last_authority(&self) -> u32 {
        match self.sub_authorities {
            0 => 0,
            1 => unsafe { *self.authorities.get_unchecked(0) },
            2 => unsafe { *self.authorities.get_unchecked(1) },
            _ => self.authorities_slice().last().copied().unwrap_or(0),
        }
    }
    #[inline]
    pub fn is_app_package(&self) -> bool {
        self.sub_authorities >= 1 && self.is(0xF) && unsafe { *self.authorities.get_unchecked(0) == 0x2 }
    }
    #[inline]
    pub fn is_nt_authority(&self) -> bool {
        self.is(0x5)
    }
    #[inline]
    pub fn domain(&self) -> Option<&[u32]> {
        if self.is_domain() {
            // We verified there's more authorities
            Some(unsafe { self.authorities_slice().get_unchecked(1..) })
        } else {
            None
        }
    }
    /// Simple check to see if the SID represents the Builtin Administrator
    /// account or the Administrators group.
    #[inline]
    pub fn is_administrators(&self) -> bool {
        if !self.is_nt_authority() {
            return false;
        }
        // Check for Identifier [0, 0, 0, 0, 5] and
        //           Authority  [32, 544] (Local Administrators Group)
        if self.sub_authorities == 2 && unsafe { *self.authorities.get_unchecked(0) == 0x20 && *self.authorities.get_unchecked(1) == 0x220 } {
            return true;
        }
        // Not a user.
        if self.sub_authorities < 3 || unsafe { *self.authorities.get_unchecked(0) != 0x15 } {
            return false;
        }
        // Check the last entry, Domain Administrators should be 512 (0x200).
        // https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/understand-security-identifiers
        self.last_authority() == 0x200
    }
    #[inline]
    pub fn is_mandatory_label(&self) -> bool {
        self.sub_authorities == 1 && self.is(0x10)
    }
    #[inline]
    pub fn as_psid<'a>(&'a self) -> PSID<'a> {
        PSID(self)
    }
    #[inline]
    pub fn authorities_slice(&self) -> &[u32] {
        // 0xF - SID_MAX_SUB_AUTHORITIES
        unsafe {
            from_raw_parts(
                self.authorities.as_ptr(),
                (self.sub_authorities as usize).min(0xF),
            )
        }
    }
    #[inline]
    pub fn username(&self) -> Win32Result<String> {
        username(self)
    }
    #[inline]
    pub fn is_well_known(&self, v: WellKnownSID) -> bool {
        match (
            v,
            self.sub_authorities,
            self.id(),
            unsafe { *self.authorities.get_unchecked(0) }, // Will always have this
            self.last_authority(),
        ) {
            (WellKnownSID::Null, 1, 0, 0, _) => true,
            (WellKnownSID::Everyone, 1, 1, 0, _) => true,
            //
            (WellKnownSID::Local, 1, 2, 0, _) => true,
            (WellKnownSID::LocalLogon, 2, 2, 0, _) => true,
            (WellKnownSID::ConsoleLogon, 1, 2, 1, _) => true,
            //
            (WellKnownSID::CreatorOwner, 1, 3, 0, _) => true,
            (WellKnownSID::CreatorGroup, 1, 3, 1, _) => true,
            (WellKnownSID::CreatorOwnerServer, 1, 3, 2, _) => true,
            (WellKnownSID::CreatorGroupServer, 1, 3, 3, _) => true,
            (WellKnownSID::CreatorOwnerRights, 1, 3, 4, _) => true,
            //
            (WellKnownSID::NtAuthority, 0, 5, 0, _) => true,
            (WellKnownSID::DialUp, 1, 5, 1, _) => true,
            (WellKnownSID::Network, 1, 5, 2, _) => true,
            (WellKnownSID::Batch, 1, 5, 3, _) => true,
            (WellKnownSID::Interactive, 1, 5, 4, _) => true,
            (WellKnownSID::Service, 1, 5, 6, _) => true,
            (WellKnownSID::Anonymous, 1, 5, 7, _) => true,
            (WellKnownSID::Proxy, 1, 5, 8, _) => true,
            (WellKnownSID::EnterpriseControllers, 1, 5, 9, _) => true,
            (WellKnownSID::PrincipalSelf, 1, 5, 10, _) => true,
            (WellKnownSID::AuthenticatedUsers, 1, 5, 11, _) => true,
            (WellKnownSID::Restricted, 1, 5, 12, _) => true,
            (WellKnownSID::TerminalServer, 1, 5, 13, _) => true,
            (WellKnownSID::RemoteLogon, 1, 5, 14, _) => true,
            (WellKnownSID::ThisOrganization, 1, 5, 15, _) => true,
            (WellKnownSID::BuiltinIUser, 1, 5, 17, _) => true,
            (WellKnownSID::LocalSystem, 1, 5, 18, _) => true,
            (WellKnownSID::LocalService, 1, 5, 19, _) => true,
            (WellKnownSID::NetworkService, 1, 5, 20, _) => true,
            (WellKnownSID::WriteRestricted, 1, 5, 33, _) => true,
            (WellKnownSID::ThisOrganizationCertificate, 2, 5, 65, 1) => true,
            (WellKnownSID::UserModeDrivers, 6, 5, 84, 0) => true,
            (WellKnownSID::LocalAccount, 1, 5, 113, _) => true,
            (WellKnownSID::LocalAccountAndAdministrator, 1, 5, 114, _) => true,
            (WellKnownSID::OtherOrganization, 1, 5, 1000, _) => true,
            //
            (WellKnownSID::BuiltinEnterpriseReadonlyControllers, 1, 5, 22, _) => true,
            (WellKnownSID::Builtin, 1, 5, 32, _) => true,
            (WellKnownSID::BuiltinAdministrators, 2, 5, 32, 544) => true,
            (WellKnownSID::BuiltinUsers, 2, 5, 32, 545) => true,
            (WellKnownSID::BuiltinGuests, 2, 5, 32, 546) => true,
            (WellKnownSID::BuiltinPowerUsers, 2, 5, 32, 547) => true,
            (WellKnownSID::BuiltinAccountOperators, 2, 5, 32, 548) => true,
            (WellKnownSID::BuiltinSystemOperators, 2, 5, 32, 549) => true,
            (WellKnownSID::BuiltinPrintOperators, 2, 5, 32, 550) => true,
            (WellKnownSID::BuiltinBackupOperators, 2, 5, 32, 551) => true,
            (WellKnownSID::BuiltinReplicator, 2, 5, 32, 552) => true,
            (WellKnownSID::BuiltinPreWindows2000CompatibleAccess, 2, 5, 32, 554) => true,
            (WellKnownSID::BuiltinRemoteDesktopUsers, 2, 5, 32, 555) => true,
            (WellKnownSID::BuiltinNetworkConfigOperators, 2, 5, 32, 556) => true,
            (WellKnownSID::BuiltinIncomingForestTrustBuilders, 2, 5, 32, 557) => true,
            (WellKnownSID::BuiltinPerfMonitoringUsers, 2, 5, 32, 558) => true,
            (WellKnownSID::BuiltinPerfLoggingUsers, 2, 5, 32, 559) => true,
            (WellKnownSID::BuiltinAuthorizationAccess, 2, 5, 32, 560) => true,
            (WellKnownSID::BuiltinTerminalServerLicenseServers, 2, 5, 32, 561) => true,
            (WellKnownSID::BuiltinDCOMUsers, 2, 5, 32, 562) => true,
            (WellKnownSID::BuiltinIISUsers, 2, 5, 32, 568) => true,
            (WellKnownSID::BuiltinCryptoOperators, 2, 5, 32, 569) => true,
            (WellKnownSID::BuiltinEventLogReadersGroup, 2, 5, 32, 573) => true,
            (WellKnownSID::BuiltinCertSvcDComAccessGroup, 2, 5, 32, 574) => true,
            (WellKnownSID::BuiltinRDSRemoteAccessServers, 2, 5, 32, 575) => true,
            (WellKnownSID::BuiltinRDSEndpointServers, 2, 5, 32, 576) => true,
            (WellKnownSID::BuiltinRDSManagementServers, 2, 5, 32, 577) => true,
            (WellKnownSID::BuiltinHyperVAdmins, 2, 5, 32, 578) => true,
            (WellKnownSID::BuiltinAccessControlAssistanceOperators, 2, 5, 32, 579) => true,
            (WellKnownSID::BuiltinRemoteManagementUsers, 2, 5, 32, 580) => true,
            (WellKnownSID::BuiltinDefaultSystemManagedGroup, 2, 5, 32, 581) => true,
            (WellKnownSID::BuiltinStorageReplicaAdmins, 2, 5, 32, 582) => true,
            (WellKnownSID::BuiltinDeviceOwners, 2, 5, 32, 583) => true,
            (WellKnownSID::BuiltinUserModeHardwareOperators, 2, 5, 32, 584) => true,
            (WellKnownSID::BuiltinOpenSSHUsers, 2, 5, 32, 585) => true,
            //
            (WellKnownSID::DomainEnterpriseReadonlyControllers, 3.., 5, 1.., 498) => true,
            (WellKnownSID::DomainAdministrator, 3.., 5, 1.., 500) => true,
            (WellKnownSID::DomainGuest, 3.., 5, 1.., 501) => true,
            (WellKnownSID::DoaminKrbtgt, 3.., 5, 1.., 502) => true,
            (WellKnownSID::DomainDefaultSystemManaged, 3.., 5, 1.., 503) => true,
            (WellKnownSID::DomainAdmins, 3.., 5, 1.., 512) => true,
            (WellKnownSID::DomainUsers, 3.., 5, 1.., 513) => true,
            (WellKnownSID::DomainGuests, 3.., 5, 1.., 514) => true,
            (WellKnownSID::DomainComputers, 3.., 5, 1.., 515) => true,
            (WellKnownSID::DomainControllers, 3.., 5, 1.., 516) => true,
            (WellKnownSID::DomainCertAdmins, 3.., 5, 1.., 517) => true,
            (WellKnownSID::DomainSchemaAdmins, 3.., 5, 1.., 518) => true,
            (WellKnownSID::DomainEnterpriseAdmins, 3.., 5, 1.., 519) => true,
            (WellKnownSID::DomainGroupPolicyAdmins, 3.., 5, 1.., 520) => true,
            (WellKnownSID::DomainReadonlyControllers, 3.., 5, 1.., 521) => true,
            (WellKnownSID::DomainCloneableControllers, 3.., 5, 1.., 522) => true,
            (WellKnownSID::DomainProtectedUsers, 3.., 5, 1.., 525) => true,
            (WellKnownSID::DomainKeyAdmins, 3.., 5, 1.., 526) => true,
            (WellKnownSID::DomainEnterpriseKeyAdmins, 3.., 5, 1.., 527) => true,
            (WellKnownSID::DomainRASandIASServers, 3.., 5, 1.., 553) => true,
            (WellKnownSID::DomainCacheablePrincipalsGroup, 3.., 5, 1.., 571) => true,
            (WellKnownSID::DomainNonCacheablePrincipalsGroup, 3.., 5, 1.., 572) => true,
            //
            (WellKnownSID::NtlmAuthentication, 2, 5, 64, 10) => true,
            (WellKnownSID::DigestAuthentication, 2, 5, 64, 21) => true,
            (WellKnownSID::SChannelAuthentication, 2, 5, 64, 14) => true,
            //
            (WellKnownSID::ApplicationPackageAuthority, 2, 15, 2, 0) => true,
            (WellKnownSID::BuiltinAnyPackage, 2, 15, 2, 1) => true,
            //
            (WellKnownSID::UntrustedLabel, 1, 16, 0, _) => true,
            (WellKnownSID::LowLabel, 1, 16, 4096, _) => true,
            (WellKnownSID::MediumLabel, 1, 16, 8192, _) => true,
            (WellKnownSID::MediumPlusLabel, 1, 16, 8448, _) => true,
            (WellKnownSID::HighLabel, 1, 16, 12288, _) => true,
            (WellKnownSID::SystemLabel, 1, 16, 16384, _) => true,
            //
            (WellKnownSID::AuthenticationAuthorityAsserted, 1, 18, 1, _) => true,
            (WellKnownSID::AuthenticationServiceAsserted, 1, 18, 2, _) => true,
            (WellKnownSID::AuthenticationFreshKeyAuth, 1, 18, 3, _) => true,
            (WellKnownSID::AuthenticationKeyTrust, 1, 18, 4, _) => true,
            (WellKnownSID::AuthenticationKeyPropertyMFA, 1, 18, 5, _) => true,
            (WellKnownSID::AuthenticationKeyPropertyAttestation, 1, 18, 6, _) => true,
            //
            _ => false,
        }
    }

    #[inline]
    const fn known(domain: u8, v: u8, a1: u32, a2: u32) -> SID {
        SID {
            revision:        1u8,
            identifiers:     [0u8, 0u8, 0u8, 0u8, 0u8, domain],
            authorities:     [a1, a2],
            sub_authorities: v,
        }
    }

    #[inline]
    fn id(&self) -> u32 {
        if self.is_single() {
            unsafe { *self.identifiers.get_unchecked(5) as u32 }
        } else {
            read_u32(unsafe { self.identifiers.get_unchecked(2..6) })
        }
    }
    #[inline]
    fn is(&self, v: u8) -> bool {
        self.is_single() && unsafe { *self.identifiers.get_unchecked(5) == v }
    }
    #[inline]
    fn is_single(&self) -> bool {
        unsafe { *self.identifiers.get_unchecked(0) == 0 && *self.identifiers.get_unchecked(1) == 0 && *self.identifiers.get_unchecked(2) == 0 && *self.identifiers.get_unchecked(3) == 0 && *self.identifiers.get_unchecked(4) == 0 }
    }
    fn into_u8(&self, aut: &[u32], buf: &mut [u8]) -> usize {
        // We bounds check the slice before we call this, so this is all in bounds.
        // We use unchecked as we can't rely on the compiler to determine this
        // on it's own.
        unsafe {
            *buf.get_unchecked_mut(0) = 0x53;
            *buf.get_unchecked_mut(1) = 0x2D;
            *buf.get_unchecked_mut(2) = 0x31; // Revision, always '1'
            *buf.get_unchecked_mut(3) = 0x2D;
        }
        let mut r = 4 + unsafe {
            if *self.identifiers.get_unchecked(0) == 0 && *self.identifiers.get_unchecked(1) == 0 {
                read_u32(self.identifiers.get_unchecked(2..6)).into_buf(buf.get_unchecked_mut(4..))
            } else {
                write_hex(
                    buf.get_unchecked_mut(4..),
                    *self.identifiers.get_unchecked(0),
                ); //    2
                write_hex(
                    buf.get_unchecked_mut(6..),
                    *self.identifiers.get_unchecked(1),
                ); //    4
                write_hex(
                    buf.get_unchecked_mut(8..),
                    *self.identifiers.get_unchecked(2),
                ); //    6
                write_hex(
                    buf.get_unchecked_mut(10..),
                    *self.identifiers.get_unchecked(3),
                ); //    8
                write_hex(
                    buf.get_unchecked_mut(12..),
                    *self.identifiers.get_unchecked(4),
                ); //    10
                write_hex(
                    buf.get_unchecked_mut(14..),
                    *self.identifiers.get_unchecked(5),
                ); //    12
                12
            }
        }; // 'r' is now the current cursor position so we write from there.
        for i in aut {
            unsafe {
                *buf.get_unchecked_mut(r) = b'-';
                r += i.into_buf(buf.get_unchecked_mut(r + 1..)) + 1;
            }
        }
        r
    }
    fn into_u16(&self, aut: &[u32], buf: &mut [u16]) -> usize {
        // We bounds check the slice before we call this, so this is all in bounds.
        // We use unchecked as we can't rely on the compiler to determine this
        // on it's own.
        unsafe {
            *buf.get_unchecked_mut(0) = 0x53;
            *buf.get_unchecked_mut(1) = 0x2D;
            *buf.get_unchecked_mut(2) = 0x31; // Revision, always '1'
            *buf.get_unchecked_mut(3) = 0x2D;
        }
        let mut r = 4 + unsafe {
            if *self.identifiers.get_unchecked(0) == 0 && *self.identifiers.get_unchecked(1) == 0 {
                write_u32_u16(
                    buf.get_unchecked_mut(4..),
                    read_u32(self.identifiers.get_unchecked(2..6)),
                )
            } else {
                write_hex_u16(
                    buf.get_unchecked_mut(4..),
                    *self.identifiers.get_unchecked(0),
                ); //    2
                write_hex_u16(
                    buf.get_unchecked_mut(6..),
                    *self.identifiers.get_unchecked(1),
                ); //    4
                write_hex_u16(
                    buf.get_unchecked_mut(8..),
                    *self.identifiers.get_unchecked(2),
                ); //    6
                write_hex_u16(
                    buf.get_unchecked_mut(10..),
                    *self.identifiers.get_unchecked(3),
                ); //    8
                write_hex_u16(
                    buf.get_unchecked_mut(12..),
                    *self.identifiers.get_unchecked(4),
                ); //    10
                write_hex_u16(
                    buf.get_unchecked_mut(14..),
                    *self.identifiers.get_unchecked(5),
                ); //    12
                12
            }
        };
        // 'r' is now the current cursor position so we write from there.
        for i in aut {
            unsafe {
                *buf.get_unchecked_mut(r) = 0x2D;
                r += write_u32_u16(buf.get_unchecked_mut(r + 1..), *i) + 1;
            }
        }
        r
    }
}
impl LUID {
    #[inline]
    pub const fn empty() -> LUID {
        LUID { low: 0u32, high: 0i32 }
    }
    #[inline]
    pub const fn new_u32(v: u32) -> LUID {
        LUID { low: v, high: 0i32 }
    }
    #[inline]
    pub const fn new(v: Privilege) -> LUID {
        LUID::new_u32(v as u32)
    }
}
impl Privilege {
    #[inline]
    pub const fn new(v: u32) -> Option<Privilege> {
        Privilege::new_u8(v as u8)
    }
    #[inline]
    pub const fn new_u8(v: u8) -> Option<Privilege> {
        match v {
            2 => Some(Privilege::SeCreateToken),
            3 => Some(Privilege::SeAssignPrimaryToken),
            4 => Some(Privilege::SeLockMemory),
            5 => Some(Privilege::SeIncreaseQuota),
            6 => Some(Privilege::SeMachineAccount),
            7 => Some(Privilege::SeTcb),
            8 => Some(Privilege::SeSecurity),
            9 => Some(Privilege::SeTakeOwnership),
            10 => Some(Privilege::SeLoadDriver),
            11 => Some(Privilege::SeSystemProfile),
            12 => Some(Privilege::SeSystemTime),
            13 => Some(Privilege::SeProfileSingleProcess),
            14 => Some(Privilege::SeIncreaseBasePriority),
            15 => Some(Privilege::SeCreatePagefile),
            16 => Some(Privilege::SeCreatePermanent),
            17 => Some(Privilege::SeBackup),
            18 => Some(Privilege::SeRestore),
            19 => Some(Privilege::SeShutdown),
            20 => Some(Privilege::SeDebug),
            21 => Some(Privilege::SeAudit),
            22 => Some(Privilege::SeSystemEnvironment),
            23 => Some(Privilege::SeChangeNotify),
            24 => Some(Privilege::SeRemoteShutdown),
            25 => Some(Privilege::SeUndock),
            26 => Some(Privilege::SeSyncAgent),
            27 => Some(Privilege::SeEnableDelegation),
            28 => Some(Privilege::SeManageVolume),
            29 => Some(Privilege::SeImpersonate),
            30 => Some(Privilege::SeCreateGlobal),
            31 => Some(Privilege::SeTrustedCredManAccess),
            32 => Some(Privilege::SeRelabel),
            33 => Some(Privilege::SeIncreaseWorkingSet),
            34 => Some(Privilege::SeTimeZone),
            35 => Some(Privilege::SeCreateSymbolicLink),
            36 => Some(Privilege::SeDelegateSessionUserImpersonate),
            _ => None,
        }
    }
}
impl HeldPrivilege {
    #[inline]
    pub fn new(v: Privilege) -> HeldPrivilege {
        match HeldPrivilege::new_checked(v) {
            Ok(x) => x,
            Err(_) => HeldPrivilege(0),
        }
    }
    #[inline]
    pub fn new_checked(v: Privilege) -> Win32Result<HeldPrivilege> {
        privilege_accquire(v).map(|_| HeldPrivilege(v as u8))
    }

    #[inline]
    pub fn success(&self) -> bool {
        self.0 != 0
    }
}
impl TokenPrivileges {
    #[inline]
    pub const fn empty() -> TokenPrivileges {
        TokenPrivileges {
            count:      0u32,
            privileges: [
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }
    #[inline]
    pub const fn simple(v: LUIDAndAttributes) -> TokenPrivileges {
        TokenPrivileges {
            count:      1u32,
            privileges: [
                MaybeUninit::new(v),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
                MaybeUninit::uninit(),
            ],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.count as usize) * 0xC
    }
    #[inline]
    pub fn as_slice(&self) -> &[LUIDAndAttributes] {
        unsafe {
            from_raw_parts(
                self.privileges.as_ptr() as *const LUIDAndAttributes,
                self.count as usize,
            )
        }
    }
    #[inline]
    pub fn set(&mut self, pos: usize, v: LUIDAndAttributes) {
        if pos > 10 || self.count > 10 {
            return;
        }
        // Bounds check above
        unsafe { self.privileges.get_unchecked_mut(pos).write(v) };
        self.count += 1;
    }
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [LUIDAndAttributes] {
        unsafe {
            from_raw_parts_mut(
                self.privileges.as_mut_ptr() as *mut LUIDAndAttributes,
                self.count as usize,
            )
        }
    }
}
impl<T> LsaPointer<T> {
    #[inline]
    pub const fn null() -> LsaPointer<T> {
        LsaPointer(null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
    #[inline]
    pub fn as_ref(&self) -> Option<&T> {
        unsafe { self.0.as_ref() }
    }
    #[inline]
    pub fn as_ref_mut(&self) -> Option<&mut T> {
        unsafe { self.0.as_mut() }
    }
}
impl LUIDAndAttributes {
    #[inline]
    pub const fn empty() -> LUIDAndAttributes {
        LUIDAndAttributes {
            luid:       LUID::empty(),
            attributes: 0u32,
        }
    }
    #[inline]
    pub const fn new_u32(v: u32, enabled: bool) -> LUIDAndAttributes {
        LUIDAndAttributes {
            luid:       LUID::new_u32(v),
            attributes: if enabled { 2u32 } else { 0u32 },
            // 0x2 - SE_PRIVILEGE_ENABLED
            // 0x4 - SE_PRIVILEGE_REMOVED
            // 0x0 - SE_PRIVILEGE_DISABLED
            //
            // From M$, 'SE_PRIVILEGE_REMOVED' REMOVES the privilege from the
            // Token and CANNOT be re-enabled. Use 0 instead to disable.
        }
    }
    #[inline]
    pub const fn new(v: Privilege, enabled: bool) -> LUIDAndAttributes {
        LUIDAndAttributes::new_u32(v as u32, enabled)
    }
}
impl SIDAndAttributes<'_> {
    #[inline]
    pub fn len(&self) -> u32 {
        self.sid.len() + 4
    }
}
impl SecurityQualityOfService {
    #[inline]
    pub const fn empty() -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0xCu32,
            effective_only:        0u8,
            impersonation_level:   0u32,
            context_tracking_mode: 0u8,
        }
    }
    #[inline]
    pub const fn level(level: u32) -> SecurityQualityOfService {
        SecurityQualityOfService {
            length:                0xCu32,
            effective_only:        0u8,
            impersonation_level:   level,
            context_tracking_mode: 0u8,
        }
    }
}
impl<'a> SecurityAttributes<'a> {
    #[inline]
    pub const fn empty() -> SecurityAttributes<'a> {
        SecurityAttributes {
            length:              size_of::<SecurityAttributes>() as u32,
            inherit:             0u32,
            security_descriptor: None,
        }
    }
    #[inline]
    pub const fn inherit() -> SecurityAttributes<'a> {
        SecurityAttributes {
            length:              size_of::<SecurityAttributes>() as u32,
            inherit:             1u32,
            security_descriptor: None,
        }
    }
}

impl Drop for LsaHandle {
    #[inline]
    fn drop(&mut self) {
        let _ = lsa_close(self.0);
    }
}
impl Deref for LsaHandle {
    type Target = Handle;

    #[inline]
    fn deref(&self) -> &Handle {
        &self.0
    }
}
impl Default for LsaHandle {
    #[inline]
    fn default() -> LsaHandle {
        LsaHandle(Handle::EMPTY)
    }
}
impl From<Handle> for LsaHandle {
    #[inline]
    fn from(v: Handle) -> LsaHandle {
        LsaHandle(v)
    }
}
impl AsRef<Handle> for LsaHandle {
    #[inline]
    fn as_ref(&self) -> &Handle {
        &self.0
    }
}

impl<T> Drop for LsaPointer<T> {
    #[inline]
    fn drop(&mut self) {
        if !self.0.is_null() {
            LocalFree(self.0);
        }
    }
}
impl<T> Deref for LsaPointer<T> {
    type Target = *mut T;

    #[inline]
    fn deref(&self) -> &*mut T {
        &self.0
    }
}
impl<T> DerefMut for LsaPointer<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut *mut T {
        &mut self.0
    }
}

impl Default for SID {
    #[inline]
    fn default() -> SID {
        SID::empty()
    }
}
impl StringWritable<u8> for SID {
    #[inline]
    fn into_buf(&self, buf: &mut [u8]) -> usize {
        let v = self.authorities_slice();
        let n = (v.len() * 10) + 16;
        if buf.len() < n {
            return 0;
        }
        self.into_u8(v, buf)
    }
    fn into_vec(&self, buf: &mut impl VecLike<u8>) -> usize {
        let v = self.authorities_slice();
        let n = (v.len() * 10) + 16;
        let i = buf.len();
        buf.resize(i + n, 0);
        let r = self.into_u8(v, unsafe { buf.get_unchecked_mut(i..) });
        buf.truncate(r + i);
        buf.shrink_to_fit();
        r
    }
}
impl StringWritable<u16> for SID {
    #[inline]
    fn into_buf(&self, buf: &mut [u16]) -> usize {
        let v = self.authorities_slice();
        let n = (v.len() * 10) + 16;
        if buf.len() < n {
            return 0;
        }
        self.into_u16(v, buf)
    }
    fn into_vec(&self, buf: &mut impl VecLike<u16>) -> usize {
        let v = self.authorities_slice();
        let n = (v.len() * 10) + 16;
        // ^ This should cover the size of all the written text in chars. This
        // assumes each authority is the max size and we need all 12 chars for
        // identifiers section. 16 is 4 start chars plus the identifiers section.
        //
        // For VecLike, we can use this to shrink back, as we'll get the actual
        // written count once complete.
        let i = buf.len();
        buf.resize(i + n, 0);
        let r = self.into_u16(v, unsafe { buf.get_unchecked_mut(i..) }); // Amount written.
        buf.truncate(r + i);
        buf.shrink_to_fit();
        r
    }
}

impl Deref for PSID<'_> {
    type Target = SID;

    #[inline]
    fn deref(&self) -> &SID {
        &self.0
    }
}

impl Eq for Privilege {}
impl Copy for Privilege {}
impl Clone for Privilege {
    #[inline]
    fn clone(&self) -> Privilege {
        *self
    }
}
impl PartialEq for Privilege {
    #[inline]
    fn eq(&self, other: &Privilege) -> bool {
        *self as u8 == *other as u8
    }
}
impl TryFrom<u8> for Privilege {
    type Error = InvalidPrivilege;

    #[inline]
    fn try_from(v: u8) -> Result<Privilege, InvalidPrivilege> {
        match v {
            2 => Ok(Privilege::SeCreateToken),
            3 => Ok(Privilege::SeAssignPrimaryToken),
            4 => Ok(Privilege::SeLockMemory),
            5 => Ok(Privilege::SeIncreaseQuota),
            6 => Ok(Privilege::SeMachineAccount),
            7 => Ok(Privilege::SeTcb),
            8 => Ok(Privilege::SeSecurity),
            9 => Ok(Privilege::SeTakeOwnership),
            10 => Ok(Privilege::SeLoadDriver),
            11 => Ok(Privilege::SeSystemProfile),
            12 => Ok(Privilege::SeSystemTime),
            13 => Ok(Privilege::SeProfileSingleProcess),
            14 => Ok(Privilege::SeIncreaseBasePriority),
            15 => Ok(Privilege::SeCreatePagefile),
            16 => Ok(Privilege::SeCreatePermanent),
            17 => Ok(Privilege::SeBackup),
            18 => Ok(Privilege::SeRestore),
            19 => Ok(Privilege::SeShutdown),
            20 => Ok(Privilege::SeDebug),
            21 => Ok(Privilege::SeAudit),
            22 => Ok(Privilege::SeSystemEnvironment),
            23 => Ok(Privilege::SeChangeNotify),
            24 => Ok(Privilege::SeRemoteShutdown),
            25 => Ok(Privilege::SeUndock),
            26 => Ok(Privilege::SeSyncAgent),
            27 => Ok(Privilege::SeEnableDelegation),
            28 => Ok(Privilege::SeManageVolume),
            29 => Ok(Privilege::SeImpersonate),
            30 => Ok(Privilege::SeCreateGlobal),
            31 => Ok(Privilege::SeTrustedCredManAccess),
            32 => Ok(Privilege::SeRelabel),
            33 => Ok(Privilege::SeIncreaseWorkingSet),
            34 => Ok(Privilege::SeTimeZone),
            35 => Ok(Privilege::SeCreateSymbolicLink),
            36 => Ok(Privilege::SeDelegateSessionUserImpersonate),
            _ => Err(InvalidPrivilege(())),
        }
    }
}
impl TryFrom<u32> for Privilege {
    type Error = InvalidPrivilege;

    #[inline]
    fn try_from(v: u32) -> Result<Privilege, InvalidPrivilege> {
        Privilege::try_from(v as u8)
    }
}

impl Drop for HeldPrivilege {
    #[inline]
    fn drop(&mut self) {
        if self.0 != 0 {
            let _ = privilege_raw_release(self.0 as u32);
        }
    }
}

impl Eq for JoinState {}
impl Copy for JoinState {}
impl Clone for JoinState {
    #[inline]
    fn clone(&self) -> JoinState {
        *self
    }
}
impl PartialEq for JoinState {
    #[inline]
    fn eq(&self, other: &JoinState) -> bool {
        match (self, other) {
            (JoinState::Domain, JoinState::Domain) => true,
            (JoinState::Unknown, JoinState::Unknown) => true,
            (JoinState::Unjoined, JoinState::Unjoined) => true,
            (JoinState::Workgroup, JoinState::Workgroup) => true,
            _ => false,
        }
    }
}

impl Eq for WellKnownSID {}
impl Copy for WellKnownSID {}
impl Clone for WellKnownSID {
    #[inline]
    fn clone(&self) -> WellKnownSID {
        *self
    }
}
impl PartialEq for WellKnownSID {
    #[inline]
    fn eq(&self, other: &WellKnownSID) -> bool {
        *self as u8 == *other as u8
    }
}
impl TryFrom<u8> for WellKnownSID {
    type Error = InvalidSID;

    #[inline]
    fn try_from(v: u8) -> Result<WellKnownSID, InvalidSID> {
        match v {
            0 => Ok(WellKnownSID::Null),
            1 => Ok(WellKnownSID::Everyone),
            2 => Ok(WellKnownSID::Local),
            3 => Ok(WellKnownSID::CreatorOwner),
            4 => Ok(WellKnownSID::CreatorGroup),
            5 => Ok(WellKnownSID::CreatorOwnerServer),
            6 => Ok(WellKnownSID::CreatorGroupServer),
            7 => Ok(WellKnownSID::NtAuthority),
            8 => Ok(WellKnownSID::DialUp),
            9 => Ok(WellKnownSID::Network),
            10 => Ok(WellKnownSID::Batch),
            11 => Ok(WellKnownSID::Interactive),
            12 => Ok(WellKnownSID::Service),
            13 => Ok(WellKnownSID::Anonymous),
            14 => Ok(WellKnownSID::Proxy),
            15 => Ok(WellKnownSID::EnterpriseControllers),
            16 => Ok(WellKnownSID::PrincipalSelf),
            17 => Ok(WellKnownSID::AuthenticatedUsers),
            18 => Ok(WellKnownSID::Restricted),
            19 => Ok(WellKnownSID::TerminalServer),
            20 => Ok(WellKnownSID::RemoteLogon),
            22 => Ok(WellKnownSID::LocalSystem),
            23 => Ok(WellKnownSID::LocalService),
            24 => Ok(WellKnownSID::NetworkService),
            25 => Ok(WellKnownSID::Builtin),
            26 => Ok(WellKnownSID::BuiltinAdministrators),
            27 => Ok(WellKnownSID::BuiltinUsers),
            28 => Ok(WellKnownSID::BuiltinGuests),
            29 => Ok(WellKnownSID::BuiltinPowerUsers),
            30 => Ok(WellKnownSID::BuiltinAccountOperators),
            31 => Ok(WellKnownSID::BuiltinSystemOperators),
            32 => Ok(WellKnownSID::BuiltinPrintOperators),
            33 => Ok(WellKnownSID::BuiltinBackupOperators),
            34 => Ok(WellKnownSID::BuiltinReplicator),
            35 => Ok(WellKnownSID::BuiltinPreWindows2000CompatibleAccess),
            36 => Ok(WellKnownSID::BuiltinRemoteDesktopUsers),
            37 => Ok(WellKnownSID::BuiltinNetworkConfigOperators),
            38 => Ok(WellKnownSID::DomainAdministrator),
            39 => Ok(WellKnownSID::DomainGuest),
            40 => Ok(WellKnownSID::DoaminKrbtgt),
            41 => Ok(WellKnownSID::DomainAdmins),
            42 => Ok(WellKnownSID::DomainUsers),
            43 => Ok(WellKnownSID::DomainGuests),
            44 => Ok(WellKnownSID::DomainComputers),
            45 => Ok(WellKnownSID::DomainControllers),
            46 => Ok(WellKnownSID::DomainCertAdmins),
            47 => Ok(WellKnownSID::DomainSchemaAdmins),
            48 => Ok(WellKnownSID::DomainEnterpriseAdmins),
            49 => Ok(WellKnownSID::DomainGroupPolicyAdmins),
            50 => Ok(WellKnownSID::DomainRASandIASServers),
            51 => Ok(WellKnownSID::NtlmAuthentication),
            52 => Ok(WellKnownSID::DigestAuthentication),
            53 => Ok(WellKnownSID::SChannelAuthentication),
            54 => Ok(WellKnownSID::ThisOrganization),
            55 => Ok(WellKnownSID::OtherOrganization),
            56 => Ok(WellKnownSID::BuiltinIncomingForestTrustBuilders),
            57 => Ok(WellKnownSID::BuiltinPerfMonitoringUsers),
            58 => Ok(WellKnownSID::BuiltinPerfLoggingUsers),
            59 => Ok(WellKnownSID::BuiltinAuthorizationAccess),
            60 => Ok(WellKnownSID::BuiltinTerminalServerLicenseServers),
            61 => Ok(WellKnownSID::BuiltinDCOMUsers),
            62 => Ok(WellKnownSID::BuiltinIISUsers),
            63 => Ok(WellKnownSID::BuiltinIUser),
            64 => Ok(WellKnownSID::BuiltinCryptoOperators),
            65 => Ok(WellKnownSID::UntrustedLabel),
            66 => Ok(WellKnownSID::LowLabel),
            67 => Ok(WellKnownSID::MediumLabel),
            68 => Ok(WellKnownSID::HighLabel),
            69 => Ok(WellKnownSID::SystemLabel),
            70 => Ok(WellKnownSID::WriteRestricted),
            71 => Ok(WellKnownSID::CreatorOwnerRights),
            72 => Ok(WellKnownSID::DomainCacheablePrincipalsGroup),
            73 => Ok(WellKnownSID::DomainNonCacheablePrincipalsGroup),
            74 => Ok(WellKnownSID::BuiltinEnterpriseReadonlyControllers),
            75 => Ok(WellKnownSID::DomainReadonlyControllers),
            76 => Ok(WellKnownSID::BuiltinEventLogReadersGroup),
            77 => Ok(WellKnownSID::DomainEnterpriseReadonlyControllers),
            78 => Ok(WellKnownSID::BuiltinCertSvcDComAccessGroup),
            79 => Ok(WellKnownSID::MediumPlusLabel),
            80 => Ok(WellKnownSID::LocalLogon),
            81 => Ok(WellKnownSID::ConsoleLogon),
            82 => Ok(WellKnownSID::ThisOrganizationCertificate),
            83 => Ok(WellKnownSID::ApplicationPackageAuthority),
            84 => Ok(WellKnownSID::BuiltinAnyPackage),
            95 => Ok(WellKnownSID::BuiltinRDSRemoteAccessServers),
            96 => Ok(WellKnownSID::BuiltinRDSEndpointServers),
            97 => Ok(WellKnownSID::BuiltinRDSManagementServers),
            98 => Ok(WellKnownSID::UserModeDrivers),
            99 => Ok(WellKnownSID::BuiltinHyperVAdmins),
            100 => Ok(WellKnownSID::DomainCloneableControllers),
            101 => Ok(WellKnownSID::BuiltinAccessControlAssistanceOperators),
            102 => Ok(WellKnownSID::BuiltinRemoteManagementUsers),
            103 => Ok(WellKnownSID::AuthenticationAuthorityAsserted),
            104 => Ok(WellKnownSID::AuthenticationServiceAsserted),
            105 => Ok(WellKnownSID::LocalAccount),
            106 => Ok(WellKnownSID::LocalAccountAndAdministrator),
            107 => Ok(WellKnownSID::DomainProtectedUsers),
            110 => Ok(WellKnownSID::DomainDefaultSystemManaged),
            111 => Ok(WellKnownSID::BuiltinDefaultSystemManagedGroup),
            112 => Ok(WellKnownSID::BuiltinStorageReplicaAdmins),
            113 => Ok(WellKnownSID::DomainKeyAdmins),
            114 => Ok(WellKnownSID::DomainEnterpriseKeyAdmins),
            115 => Ok(WellKnownSID::AuthenticationKeyTrust),
            116 => Ok(WellKnownSID::AuthenticationKeyPropertyMFA),
            117 => Ok(WellKnownSID::AuthenticationKeyPropertyAttestation),
            118 => Ok(WellKnownSID::AuthenticationFreshKeyAuth),
            119 => Ok(WellKnownSID::BuiltinDeviceOwners),
            120 => Ok(WellKnownSID::BuiltinUserModeHardwareOperators),
            121 => Ok(WellKnownSID::BuiltinOpenSSHUsers),
            _ => Err(InvalidSID(())),
        }
    }
}
impl TryFrom<u32> for WellKnownSID {
    type Error = InvalidSID;

    #[inline]
    fn try_from(v: u32) -> Result<WellKnownSID, InvalidSID> {
        WellKnownSID::try_from(v as u8)
    }
}

impl Drop for TokenPrivileges {
    #[inline]
    fn drop(&mut self) {
        for i in 0..self.count as usize {
            unsafe { self.privileges.get_unchecked_mut(i).assume_init_drop() }
        }
    }
}
impl Default for TokenPrivileges {
    #[inline]
    fn default() -> TokenPrivileges {
        TokenPrivileges::empty()
    }
}

impl Default for LUIDAndAttributes {
    #[inline]
    fn default() -> LUIDAndAttributes {
        LUIDAndAttributes::empty()
    }
}
impl Default for SecurityQualityOfService {
    #[inline]
    fn default() -> SecurityQualityOfService {
        SecurityQualityOfService::empty()
    }
}
impl<'a> Default for SecurityAttributes<'a> {
    #[inline]
    fn default() -> SecurityAttributes<'a> {
        SecurityAttributes::empty()
    }
}

fn username(sid: &SID) -> Win32Result<String> {
    let (mut c, mut x, mut t) = (64u32, 64u32, 0u32);
    let mut n: WChars = Blob::with_capacity(c as usize);
    let mut d: WChars = Blob::with_capacity(x as usize);
    let f = syscall!(
        advapi32().LookupAccountSidW,
        fn(*const u16, *const SID, *mut u16, *mut u32, *mut u16, *mut u32, *mut u32) -> u32
    );
    loop {
        n.resize((c as usize) * 2);
        d.resize((x as usize) * 2);
        let r = unsafe {
            f(
                null(),
                sid,
                n.as_mut_ptr(),
                &mut c,
                d.as_mut_ptr(),
                &mut x,
                &mut t,
            )
        };
        match r {
            0x7A => (), // 0x7A - ERROR_INSUFFICIENT_BUFFER
            _ if x > 0 || c > 0 => break,
            _ => return Err(Win32Error::last_error()),
        }
    }
    let r = if x > 0 {
        d.truncate(x as usize);
        d.push(0x5C);
        // We can rely on the 'resize' calls above for this to be initialized correctly,
        unsafe { d.extend_from_slice(n.get_unchecked(0..c as usize)) }
        c + x + 1
    } else {
        c
    };
    Ok(utf16_to_string(unsafe { d.get_unchecked(0..r as usize) }))
}

#[cfg(not(feature = "strip"))]
mod display {
    extern crate core;

    use core::fmt::{Debug, Display, Formatter, Result};

    use crate::structs::{JoinState, Privilege, WellKnownSID, PSID, SID};

    impl Debug for SID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("SID")
                .field("revision", &self.revision)
                .field("sub_authorities", &self.sub_authorities)
                .field("identifiers", &self.identifiers)
                .field("authorities", &self.authorities_slice())
                .finish()
        }
    }
    impl Display for SID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            let v = self.to_slice();
            f.write_str(unsafe { v.as_str_unchecked() })
        }
    }

    impl Debug for PSID<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Debug::fmt(self.0, f)
        }
    }
    impl Display for PSID<'_> {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            Display::fmt(self.0, f)
        }
    }

    impl Debug for JoinState {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                JoinState::Unknown => f.write_str("Unknown"),
                JoinState::Unjoined => f.write_str("Unjoined"),
                JoinState::Workgroup => f.write_str("Workgroup"),
                JoinState::Domain => f.write_str("Domain"),
            }
        }
    }
    impl Debug for Privilege {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                Privilege::SeCreateToken => f.write_str("SeCreateToken"),
                Privilege::SeAssignPrimaryToken => f.write_str("SeAssignPrimaryToken"),
                Privilege::SeLockMemory => f.write_str("SeLockMemory"),
                Privilege::SeIncreaseQuota => f.write_str("SeIncreaseQuota"),
                Privilege::SeMachineAccount => f.write_str("SeMachineAccount"),
                Privilege::SeTcb => f.write_str("SeTcb"),
                Privilege::SeSecurity => f.write_str("SeSecurity"),
                Privilege::SeTakeOwnership => f.write_str("SeTakeOwnership"),
                Privilege::SeLoadDriver => f.write_str("SeLoadDriver"),
                Privilege::SeSystemProfile => f.write_str("SeSystemProfile"),
                Privilege::SeSystemTime => f.write_str("SeSystemTime"),
                Privilege::SeProfileSingleProcess => f.write_str("SeProfileSingleProcess"),
                Privilege::SeIncreaseBasePriority => f.write_str("SeIncreaseBasePriority"),
                Privilege::SeCreatePagefile => f.write_str("SeCreatePagefile"),
                Privilege::SeCreatePermanent => f.write_str("SeCreatePermanent"),
                Privilege::SeBackup => f.write_str("SeBackup"),
                Privilege::SeRestore => f.write_str("SeRestore"),
                Privilege::SeShutdown => f.write_str("SeShutdown"),
                Privilege::SeDebug => f.write_str("SeDebug"),
                Privilege::SeAudit => f.write_str("SeAudit"),
                Privilege::SeSystemEnvironment => f.write_str("SeSystemEnvironment"),
                Privilege::SeChangeNotify => f.write_str("SeChangeNotify"),
                Privilege::SeRemoteShutdown => f.write_str("SeRemoteShutdown"),
                Privilege::SeUndock => f.write_str("SeUndock"),
                Privilege::SeSyncAgent => f.write_str("SeSyncAgent"),
                Privilege::SeEnableDelegation => f.write_str("SeEnableDelegation"),
                Privilege::SeManageVolume => f.write_str("SeManageVolume"),
                Privilege::SeImpersonate => f.write_str("SeImpersonate"),
                Privilege::SeCreateGlobal => f.write_str("SeCreateGlobal"),
                Privilege::SeTrustedCredManAccess => f.write_str("SeTrustedCredManAccess"),
                Privilege::SeRelabel => f.write_str("SeRelabel"),
                Privilege::SeIncreaseWorkingSet => f.write_str("SeIncreaseWorkingSet"),
                Privilege::SeTimeZone => f.write_str("SeTimeZone"),
                Privilege::SeCreateSymbolicLink => f.write_str("SeCreateSymbolicLink"),
                Privilege::SeDelegateSessionUserImpersonate => f.write_str("SeDelegateSessionUserImpersonate"),
            }
        }
    }
    impl Debug for WellKnownSID {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            match self {
                WellKnownSID::Null => f.write_str("Null"),
                WellKnownSID::Everyone => f.write_str("Everyone"),
                WellKnownSID::Local => f.write_str("Local"),
                WellKnownSID::CreatorOwner => f.write_str("CreatorOwner"),
                WellKnownSID::CreatorGroup => f.write_str("CreatorGroup"),
                WellKnownSID::CreatorOwnerServer => f.write_str("CreatorOwnerServer"),
                WellKnownSID::CreatorGroupServer => f.write_str("CreatorGroupServer"),
                WellKnownSID::NtAuthority => f.write_str("NtAuthority"),
                WellKnownSID::DialUp => f.write_str("DialUp"),
                WellKnownSID::Network => f.write_str("Network"),
                WellKnownSID::Batch => f.write_str("Batch"),
                WellKnownSID::Interactive => f.write_str("Interactive"),
                WellKnownSID::Service => f.write_str("Service"),
                WellKnownSID::Anonymous => f.write_str("Anonymous"),
                WellKnownSID::Proxy => f.write_str("Proxy"),
                WellKnownSID::EnterpriseControllers => f.write_str("EnterpriseControllers"),
                WellKnownSID::PrincipalSelf => f.write_str("PrincipalSelf"),
                WellKnownSID::AuthenticatedUsers => f.write_str("AuthenticatedUsers"),
                WellKnownSID::Restricted => f.write_str("Restricted"),
                WellKnownSID::TerminalServer => f.write_str("TerminalServer"),
                WellKnownSID::RemoteLogon => f.write_str("RemoteLogon"),
                WellKnownSID::LocalSystem => f.write_str("LocalSystem"),
                WellKnownSID::LocalService => f.write_str("LocalService"),
                WellKnownSID::NetworkService => f.write_str("NetworkService"),
                WellKnownSID::Builtin => f.write_str("Builtin"),
                WellKnownSID::BuiltinAdministrators => f.write_str("BuiltinAdministrators"),
                WellKnownSID::BuiltinUsers => f.write_str("BuiltinUsers"),
                WellKnownSID::BuiltinGuests => f.write_str("BuiltinGuests"),
                WellKnownSID::BuiltinPowerUsers => f.write_str("BuiltinPowerUsers"),
                WellKnownSID::BuiltinAccountOperators => f.write_str("BuiltinAccountOperators"),
                WellKnownSID::BuiltinSystemOperators => f.write_str("BuiltinSystemOperators"),
                WellKnownSID::BuiltinPrintOperators => f.write_str("BuiltinPrintOperators"),
                WellKnownSID::BuiltinBackupOperators => f.write_str("BuiltinBackupOperators"),
                WellKnownSID::BuiltinReplicator => f.write_str("BuiltinReplicator"),
                WellKnownSID::BuiltinPreWindows2000CompatibleAccess => f.write_str("BuiltinPreWindows2000CompatibleAccess"),
                WellKnownSID::BuiltinRemoteDesktopUsers => f.write_str("BuiltinRemoteDesktopUsers"),
                WellKnownSID::BuiltinNetworkConfigOperators => f.write_str("BuiltinNetworkConfigOperators"),
                WellKnownSID::DomainAdministrator => f.write_str("DomainAdministrator"),
                WellKnownSID::DomainGuest => f.write_str("DomainGuest"),
                WellKnownSID::DoaminKrbtgt => f.write_str("DoaminKrbtgt"),
                WellKnownSID::DomainAdmins => f.write_str("DomainAdmins"),
                WellKnownSID::DomainUsers => f.write_str("DomainUsers"),
                WellKnownSID::DomainGuests => f.write_str("DomainGuests"),
                WellKnownSID::DomainComputers => f.write_str("DomainComputers"),
                WellKnownSID::DomainControllers => f.write_str("DomainControllers"),
                WellKnownSID::DomainCertAdmins => f.write_str("DomainCertAdmins"),
                WellKnownSID::DomainSchemaAdmins => f.write_str("DomainSchemaAdmins"),
                WellKnownSID::DomainEnterpriseAdmins => f.write_str("DomainEnterpriseAdmins"),
                WellKnownSID::DomainGroupPolicyAdmins => f.write_str("DomainGroupPolicyAdmins"),
                WellKnownSID::DomainRASandIASServers => f.write_str("DomainRASandIASServers"),
                WellKnownSID::NtlmAuthentication => f.write_str("NtlmAuthentication"),
                WellKnownSID::DigestAuthentication => f.write_str("DigestAuthentication"),
                WellKnownSID::SChannelAuthentication => f.write_str("SChannelAuthentication"),
                WellKnownSID::ThisOrganization => f.write_str("ThisOrganization"),
                WellKnownSID::OtherOrganization => f.write_str("OtherOrganization"),
                WellKnownSID::BuiltinIncomingForestTrustBuilders => f.write_str("BuiltinIncomingForestTrustBuilders"),
                WellKnownSID::BuiltinPerfMonitoringUsers => f.write_str("BuiltinPerfMonitoringUsers"),
                WellKnownSID::BuiltinPerfLoggingUsers => f.write_str("BuiltinPerfLoggingUsers"),
                WellKnownSID::BuiltinAuthorizationAccess => f.write_str("BuiltinAuthorizationAccess"),
                WellKnownSID::BuiltinTerminalServerLicenseServers => f.write_str("BuiltinTerminalServerLicenseServers"),
                WellKnownSID::BuiltinDCOMUsers => f.write_str("BuiltinDCOMUsers"),
                WellKnownSID::BuiltinIISUsers => f.write_str("BuiltinIISUsers"),
                WellKnownSID::BuiltinIUser => f.write_str("BuiltinIUser"),
                WellKnownSID::BuiltinCryptoOperators => f.write_str("BuiltinCryptoOperators"),
                WellKnownSID::UntrustedLabel => f.write_str("UntrustedLabel"),
                WellKnownSID::LowLabel => f.write_str("LowLabel"),
                WellKnownSID::MediumLabel => f.write_str("MediumLabel"),
                WellKnownSID::HighLabel => f.write_str("HighLabel"),
                WellKnownSID::SystemLabel => f.write_str("SystemLabel"),
                WellKnownSID::WriteRestricted => f.write_str("WriteRestricted"),
                WellKnownSID::CreatorOwnerRights => f.write_str("CreatorOwnerRights"),
                WellKnownSID::DomainCacheablePrincipalsGroup => f.write_str("DomainCacheablePrincipalsGroup"),
                WellKnownSID::DomainNonCacheablePrincipalsGroup => f.write_str("DomainNonCacheablePrincipalsGroup"),
                WellKnownSID::BuiltinEnterpriseReadonlyControllers => f.write_str("BuiltinEnterpriseReadonlyControllers"),
                WellKnownSID::DomainReadonlyControllers => f.write_str("DomainReadonlyControllers"),
                WellKnownSID::BuiltinEventLogReadersGroup => f.write_str("BuiltinEventLogReadersGroup"),
                WellKnownSID::DomainEnterpriseReadonlyControllers => f.write_str("DomainEnterpriseReadonlyControllers"),
                WellKnownSID::BuiltinCertSvcDComAccessGroup => f.write_str("BuiltinCertSvcDComAccessGroup"),
                WellKnownSID::MediumPlusLabel => f.write_str("MediumPlusLabel"),
                WellKnownSID::LocalLogon => f.write_str("LocalLogon"),
                WellKnownSID::ConsoleLogon => f.write_str("ConsoleLogon"),
                WellKnownSID::ThisOrganizationCertificate => f.write_str("ThisOrganizationCertificate"),
                WellKnownSID::ApplicationPackageAuthority => f.write_str("ApplicationPackageAuthority"),
                WellKnownSID::BuiltinAnyPackage => f.write_str("BuiltinAnyPackage"),
                WellKnownSID::BuiltinRDSRemoteAccessServers => f.write_str("BuiltinRDSRemoteAccessServers"),
                WellKnownSID::BuiltinRDSEndpointServers => f.write_str("BuiltinRDSEndpointServers"),
                WellKnownSID::BuiltinRDSManagementServers => f.write_str("BuiltinRDSManagementServers"),
                WellKnownSID::UserModeDrivers => f.write_str("UserModeDrivers"),
                WellKnownSID::BuiltinHyperVAdmins => f.write_str("BuiltinHyperVAdmins"),
                WellKnownSID::DomainCloneableControllers => f.write_str("DomainCloneableControllers"),
                WellKnownSID::BuiltinAccessControlAssistanceOperators => f.write_str("BuiltinAccessControlAssistanceOperators"),
                WellKnownSID::BuiltinRemoteManagementUsers => f.write_str("BuiltinRemoteManagementUsers"),
                WellKnownSID::AuthenticationAuthorityAsserted => f.write_str("AuthenticationAuthorityAsserted"),
                WellKnownSID::AuthenticationServiceAsserted => f.write_str("AuthenticationServiceAsserted"),
                WellKnownSID::LocalAccount => f.write_str("LocalAccount"),
                WellKnownSID::LocalAccountAndAdministrator => f.write_str("LocalAccountAndAdministrator"),
                WellKnownSID::DomainProtectedUsers => f.write_str("DomainProtectedUsers"),
                WellKnownSID::DomainDefaultSystemManaged => f.write_str("DomainDefaultSystemManaged"),
                WellKnownSID::BuiltinDefaultSystemManagedGroup => f.write_str("BuiltinDefaultSystemManagedGroup"),
                WellKnownSID::BuiltinStorageReplicaAdmins => f.write_str("BuiltinStorageReplicaAdmins"),
                WellKnownSID::DomainKeyAdmins => f.write_str("DomainKeyAdmins"),
                WellKnownSID::DomainEnterpriseKeyAdmins => f.write_str("DomainEnterpriseKeyAdmins"),
                WellKnownSID::AuthenticationKeyTrust => f.write_str("AuthenticationKeyTrust"),
                WellKnownSID::AuthenticationKeyPropertyMFA => f.write_str("AuthenticationKeyPropertyMFA"),
                WellKnownSID::AuthenticationKeyPropertyAttestation => f.write_str("AuthenticationKeyPropertyAttestation"),
                WellKnownSID::AuthenticationFreshKeyAuth => f.write_str("AuthenticationFreshKeyAuth"),
                WellKnownSID::BuiltinDeviceOwners => f.write_str("BuiltinDeviceOwners"),
                WellKnownSID::BuiltinUserModeHardwareOperators => f.write_str("BuiltinUserModeHardwareOperators"),
                WellKnownSID::BuiltinOpenSSHUsers => f.write_str("BuiltinOpenSSHUsers"),
            }
        }
    }
}
