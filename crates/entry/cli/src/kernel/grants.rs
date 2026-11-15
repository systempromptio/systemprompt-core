//! The descriptor capabilities that relax a runner requirement, and the
//! closed list of registration paths allowed each one.
//!
//! `cloud` → cloud-operator; `analytics` → local-export and skip-validation;
//! `admin mcp` → child-host. No registration path holds serves-api or any
//! other grant. The kernel's own commands set their flags directly:
//! `admin session` (cloud-operator), `admin agents run` (child-host),
//! `infra services serve` (serves-api), and the `infra` and `admin session
//! login` commands that skip validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::descriptor::CommandDescriptor;

/// A descriptor capability that relaxes a requirement the runner imposes.
///
/// `ChildHost` skips the profile, session and cloud credentials; `ServesApi`
/// hands interrupt handling to the server; `CloudOperator` loads the operator
/// cloud credentials and runs against a cloud profile with no remote route;
/// `LocalExport` keeps an `--export` run on this machine; `SkipValidation`
/// skips the services-config validation. A registered command declares the
/// grants it needs, the kernel refuses a declaration its closed table does
/// not list for that path, and a parsed descriptor holding an undeclared
/// grant is refused before bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelGrant {
    ChildHost,
    ServesApi,
    CloudOperator,
    LocalExport,
    SkipValidation,
}

impl KernelGrant {
    pub const ALL: [Self; 5] = [
        Self::ChildHost,
        Self::ServesApi,
        Self::CloudOperator,
        Self::LocalExport,
        Self::SkipValidation,
    ];

    pub(crate) const fn flag(self) -> u16 {
        match self {
            Self::ChildHost => CommandDescriptor::FLAG_CHILD_HOST,
            Self::ServesApi => CommandDescriptor::FLAG_SERVES_API,
            Self::CloudOperator => CommandDescriptor::FLAG_CLOUD_OPERATOR,
            Self::LocalExport => CommandDescriptor::FLAG_LOCAL_EXPORT,
            Self::SkipValidation => CommandDescriptor::FLAG_SKIP_VALIDATION,
        }
    }
}

impl std::fmt::Display for KernelGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ChildHost => "child-host",
            Self::ServesApi => "serves-api",
            Self::CloudOperator => "cloud-operator",
            Self::LocalExport => "local-export",
            Self::SkipValidation => "skip-validation",
        })
    }
}

const KERNEL_GRANTS: &[(&[&str], KernelGrant)] = &[
    (&["admin", "mcp"], KernelGrant::ChildHost),
    (&["analytics"], KernelGrant::LocalExport),
    (&["analytics"], KernelGrant::SkipValidation),
    (&["cloud"], KernelGrant::CloudOperator),
];

pub(super) fn listed_for(parent: &[&str], name: &str, grant: KernelGrant) -> bool {
    KERNEL_GRANTS.iter().any(|(path, listed)| {
        *listed == grant
            && path
                .split_last()
                .is_some_and(|(last, head)| *last == name && head == parent)
    })
}
