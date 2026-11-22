//! Per-command bootstrap descriptor: which of profile/secrets/paths a command
//! needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::impact::{DataImpact, RoutingClass};
use super::{CommandClaim, KernelGrant};

/// Bootstrap flags for one command.
///
/// `serves_api` marks a command that runs the API server in this process: the
/// runner stamps the server identity and lets the server own its own interrupt
/// handling.
///
/// `child_host` marks a process the API spawns (`admin agents run`,
/// `admin mcp serve`): it boots from the environment the API granted it, so
/// the runner resolves no profile, reads no CLI session or cloud credential,
/// discovers no log level from a profile, and installs no Ctrl-C handler; the
/// process ends on the signal its supervisor sends.
///
/// Model discovery (installing the services registry through boot-time
/// discovery) is set only for the server; every other command loads the YAML
/// catalog as authored and must not reach for the network.
///
/// `cloud_operator` marks a command that manages the cloud account itself
/// (`cloud`, `admin session`): the runner loads the cloud credentials for it
/// and lets it run against a cloud profile that has no remote route.
///
/// `LOCAL_DATABASE` is `FULL` without remote eligibility: the command runs
/// on this machine against the profile's own database, whatever the profile.
///
/// `local_export` marks a command whose `--export` flag writes a local file:
/// when the flag is present the runner keeps it on this machine instead of
/// forwarding it to a tenant.
///
/// `child_host`, `serves_api`, `cloud_operator`, `local_export`,
/// `skip_validation` and `boot_profile` relax a requirement the runner would
/// otherwise impose, so each is a [`KernelGrant`]: a registered command holds
/// one only when it declares it and the kernel lists its path for it.
///
/// [`CommandDescriptor::makes`] projects the [`CommandClaim`]s a descriptor
/// makes from its flags: read-only routing, a database command that is not
/// remote eligible, a database command that destroys nothing.
///
/// `is_destructive` is never set by hand: it is projected from the command's
/// exhaustive [`DataImpact`] classification, so a command cannot be added
/// without deciding whether it destroys data.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandDescriptor {
    flags: u32,
}

impl CommandDescriptor {
    const FLAG_PROFILE: u32 = 0b0000_0000_0001;
    const FLAG_SECRETS: u32 = 0b0000_0000_0010;
    const FLAG_PATHS: u32 = 0b0000_0000_0100;
    const FLAG_DATABASE: u32 = 0b0000_0000_1000;
    const FLAG_REMOTE_ELIGIBLE: u32 = 0b0000_0001_0000;
    const FLAG_SKIP_VALIDATION: u32 = 0b0000_0010_0000;
    const FLAG_READ_ONLY: u32 = 0b0000_0100_0000;
    const FLAG_MODEL_DISCOVERY: u32 = 0b0000_1000_0000;
    const FLAG_DESTRUCTIVE: u32 = 0b0001_0000_0000;
    const FLAG_IRREVERSIBLE: u32 = 0b0010_0000_0000;
    const FLAG_SERVES_API: u32 = 0b0100_0000_0000;
    const FLAG_CHILD_HOST: u32 = 0b1000_0000_0000;
    const FLAG_CLOUD_OPERATOR: u32 = 0b0001_0000_0000_0000;
    const FLAG_LOCAL_EXPORT: u32 = 0b0010_0000_0000_0000;
    const FLAG_BOOT_PROFILE: u32 = 0b0100_0000_0000_0000;
    const FLAG_CONFIG_RECONCILE: u32 = 0b1000_0000_0000_0000;
    const FLAG_TARGET_ADMISSION: u32 = 0b0001_0000_0000_0000_0000;
    const FLAG_SERVICES_TREE: u32 = 0b0010_0000_0000_0000_0000;
    const FLAG_CALLER_SESSION: u32 = 0b0100_0000_0000_0000_0000;
    const FLAG_SESSION_CONTEXT: u32 = 0b1000_0000_0000_0000_0000;

    pub const NONE: Self = Self { flags: 0 };

    pub const CHILD_HOST: Self = Self {
        flags: Self::FLAG_CHILD_HOST,
    };

    pub const PROFILE_ONLY: Self = Self {
        flags: Self::FLAG_PROFILE,
    };

    pub const PROFILE_AND_SECRETS: Self = Self {
        flags: Self::FLAG_PROFILE | Self::FLAG_SECRETS,
    };

    pub const PROFILE_SECRETS_AND_PATHS: Self = Self {
        flags: Self::FLAG_PROFILE | Self::FLAG_SECRETS | Self::FLAG_PATHS,
    };

    pub const LOCAL_DATABASE: Self = Self {
        flags: Self::FLAG_PROFILE | Self::FLAG_SECRETS | Self::FLAG_PATHS | Self::FLAG_DATABASE,
    };

    pub const FULL: Self = Self {
        flags: Self::FLAG_PROFILE
            | Self::FLAG_SECRETS
            | Self::FLAG_PATHS
            | Self::FLAG_DATABASE
            | Self::FLAG_REMOTE_ELIGIBLE,
    };

    pub const fn profile(&self) -> bool {
        self.flags & Self::FLAG_PROFILE != 0
    }

    pub const fn secrets(&self) -> bool {
        self.flags & Self::FLAG_SECRETS != 0
    }

    pub const fn paths(&self) -> bool {
        self.flags & Self::FLAG_PATHS != 0
    }

    pub const fn database(&self) -> bool {
        self.flags & Self::FLAG_DATABASE != 0
    }

    pub const fn routing_class(&self) -> RoutingClass {
        if self.flags & Self::FLAG_REMOTE_ELIGIBLE == 0 {
            RoutingClass::LocalOnly
        } else if self.flags & Self::FLAG_READ_ONLY != 0 {
            RoutingClass::ReadOnly
        } else {
            RoutingClass::Mutating
        }
    }

    pub const fn skip_validation(&self) -> bool {
        self.holds(KernelGrant::SkipValidation)
    }

    pub const fn discovers_models(&self) -> bool {
        self.flags & Self::FLAG_MODEL_DISCOVERY != 0
    }

    pub const fn with_remote_eligible(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_REMOTE_ELIGIBLE,
        }
    }

    pub const fn with_read_only(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_READ_ONLY,
        }
    }

    pub const fn with_skip_validation(self) -> Self {
        self.with_grant(KernelGrant::SkipValidation)
    }

    pub const fn with_model_discovery(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_MODEL_DISCOVERY,
        }
    }

    pub const fn serves_api(&self) -> bool {
        self.holds(KernelGrant::ServesApi)
    }

    pub const fn child_host(&self) -> bool {
        self.holds(KernelGrant::ChildHost)
    }

    pub const fn owns_signals(&self) -> bool {
        self.serves_api() || self.child_host()
    }

    pub const fn with_serves_api(self) -> Self {
        self.with_grant(KernelGrant::ServesApi)
    }

    pub const fn cloud_operator(&self) -> bool {
        self.holds(KernelGrant::CloudOperator)
    }

    pub const fn with_cloud_operator(self) -> Self {
        self.with_grant(KernelGrant::CloudOperator)
    }

    pub const fn local_export(&self) -> bool {
        self.holds(KernelGrant::LocalExport)
    }

    pub const fn with_local_export(self) -> Self {
        self.with_grant(KernelGrant::LocalExport)
    }

    const fn grant_flag(grant: KernelGrant) -> u32 {
        match grant {
            KernelGrant::ChildHost => Self::FLAG_CHILD_HOST,
            KernelGrant::ServesApi => Self::FLAG_SERVES_API,
            KernelGrant::CloudOperator => Self::FLAG_CLOUD_OPERATOR,
            KernelGrant::LocalExport => Self::FLAG_LOCAL_EXPORT,
            KernelGrant::SkipValidation => Self::FLAG_SKIP_VALIDATION,
            KernelGrant::BootProfile => Self::FLAG_BOOT_PROFILE,
            KernelGrant::ConfigReconcile => Self::FLAG_CONFIG_RECONCILE,
            KernelGrant::TargetAdmission => Self::FLAG_TARGET_ADMISSION,
            KernelGrant::ServicesTree => Self::FLAG_SERVICES_TREE,
            KernelGrant::CallerSession => Self::FLAG_CALLER_SESSION,
            KernelGrant::SessionContext => Self::FLAG_SESSION_CONTEXT,
        }
    }

    pub const fn holds(&self, grant: KernelGrant) -> bool {
        self.flags & Self::grant_flag(grant) != 0
    }

    pub const fn with_grant(self, grant: KernelGrant) -> Self {
        Self {
            flags: self.flags | Self::grant_flag(grant),
        }
    }

    pub const fn is_destructive(&self) -> bool {
        self.flags & Self::FLAG_DESTRUCTIVE != 0
    }

    pub const fn requires_confirmation(&self) -> bool {
        self.flags & Self::FLAG_IRREVERSIBLE != 0
    }

    pub const fn data_impact(&self) -> DataImpact {
        if self.requires_confirmation() {
            DataImpact::Irreversible
        } else if self.is_destructive() {
            DataImpact::Destructive
        } else {
            DataImpact::Preserving
        }
    }

    pub const fn makes(&self, claim: CommandClaim) -> bool {
        match claim {
            CommandClaim::ReadOnlyRouting => {
                matches!(self.routing_class(), RoutingClass::ReadOnly)
            },
            CommandClaim::LocalDatabase => {
                self.database() && matches!(self.routing_class(), RoutingClass::LocalOnly)
            },
            CommandClaim::PreservesData => {
                self.database() && matches!(self.data_impact(), DataImpact::Preserving)
            },
            CommandClaim::DirectDatabase => false,
        }
    }

    pub const fn with_data_impact(self, impact: DataImpact) -> Self {
        match impact {
            DataImpact::Destructive => Self {
                flags: self.flags | Self::FLAG_DESTRUCTIVE,
            },
            DataImpact::Irreversible => Self {
                flags: self.flags | Self::FLAG_DESTRUCTIVE | Self::FLAG_IRREVERSIBLE,
            },
            DataImpact::Preserving => self,
        }
    }
}

/// The bootstrap requirements a parsed command declares before it runs.
pub trait DescribeCommand {
    fn descriptor(&self) -> CommandDescriptor;
}
