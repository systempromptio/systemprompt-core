//! The kernel grants a command may request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A descriptor capability that relaxes a requirement the CLI runner imposes.
///
/// `ChildHost` skips the profile, session and cloud credentials; `ServesApi`
/// hands interrupt handling to the server; `CloudOperator` loads the operator
/// cloud credentials and runs against a cloud profile with no remote route;
/// `LocalExport` keeps an `--export` run on this machine and serves the
/// profile's local exports directory through `CommandHost::export_dir`;
/// `SkipValidation` skips the services-config validation; `BootProfile` lets
/// the command boot a profile at a path through
/// [`CommandHost::boot_config`](crate::cli::CommandHost::boot_config);
/// `ConfigReconcile` serves the command the kernel-built config apply service
/// (database rows of every config plane, read with the loaded secrets) through
/// its resolver; `TargetAdmission` admits the binary's composition against the
/// disabled-member list of a profile at a path and returns its open admitted
/// views through
/// [`CommandHost::admitted_views`](crate::cli::CommandHost::admitted_views);
/// `ServicesTree` serves the active services root and the non-secret
/// profile values read beside it through `CommandHost::services_tree`,
/// without a database boot; `CallerSession` serves the CLI caller's own
/// session token through `CommandHost::caller_session`. A
/// member's command requests the grants it needs; the composition and the CLI
/// registry refuse a request the reserved CLI paths do not list for that path
/// and owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelGrant {
    ChildHost,
    ServesApi,
    CloudOperator,
    LocalExport,
    SkipValidation,
    BootProfile,
    ConfigReconcile,
    TargetAdmission,
    ServicesTree,
    CallerSession,
}

impl KernelGrant {
    pub const ALL: [Self; 10] = [
        Self::ChildHost,
        Self::ServesApi,
        Self::CloudOperator,
        Self::LocalExport,
        Self::SkipValidation,
        Self::BootProfile,
        Self::ConfigReconcile,
        Self::TargetAdmission,
        Self::ServicesTree,
        Self::CallerSession,
    ];
}

impl std::fmt::Display for KernelGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ChildHost => "child-host",
            Self::ServesApi => "serves-api",
            Self::CloudOperator => "cloud-operator",
            Self::LocalExport => "local-export",
            Self::SkipValidation => "skip-validation",
            Self::BootProfile => "boot-profile",
            Self::ConfigReconcile => "config-reconcile",
            Self::TargetAdmission => "target-admission",
            Self::ServicesTree => "services-tree",
            Self::CallerSession => "caller-session",
        })
    }
}
