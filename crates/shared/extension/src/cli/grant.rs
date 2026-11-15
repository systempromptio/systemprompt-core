//! The kernel grants a command may request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A descriptor capability that relaxes a requirement the CLI runner imposes.
///
/// `ChildHost` skips the profile, session and cloud credentials; `ServesApi`
/// hands interrupt handling to the server; `CloudOperator` loads the operator
/// cloud credentials and runs against a cloud profile with no remote route;
/// `LocalExport` keeps an `--export` run on this machine; `SkipValidation`
/// skips the services-config validation. A member's command requests the
/// grants it needs; the composition and the CLI registry refuse a request
/// the reserved CLI paths do not list for that path and owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
