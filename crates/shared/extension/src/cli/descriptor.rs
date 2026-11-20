//! Per-command bootstrap descriptor: which of profile/secrets/paths a command
//! needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::KernelGrant;

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
/// `local_export` marks a command whose `--export` flag writes a local file:
/// when the flag is present the runner keeps it on this machine instead of
/// forwarding it to a tenant.
///
/// `child_host`, `serves_api`, `cloud_operator`, `local_export`,
/// `skip_validation` and `boot_profile` relax a requirement the runner would
/// otherwise impose, so each is a [`KernelGrant`]: a registered command holds
/// one only when it declares it and the kernel lists its path for it.
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

/// How a command treats the data behind the profile it resolves.
///
/// `Destructive` commands delete rows, rewrite schema, run jobs or change
/// privilege. They refuse a cloud profile that arrived implicitly (stored
/// session or directory discovery), and a failed remote route never falls
/// back to direct database access for them. Every command enum classifies
/// its variants in an exhaustive `match` with no wildcard arm.
///
/// `Irreversible` is `Destructive` plus an operator confirmation: the runner
/// prompts before the command runs, and with no terminal the command refuses
/// unless `--yes` was passed. Such a command flattens
/// the CLI's `ConfirmArgs` so that `--yes` parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataImpact {
    Preserving,
    Destructive,
    Irreversible,
}

impl DataImpact {
    pub const fn destroys_data(self) -> bool {
        match self {
            Self::Preserving => false,
            Self::Destructive | Self::Irreversible => true,
        }
    }
}

/// What a command is allowed to do when the active profile is a cloud profile.
///
/// `LocalOnly` is never routed remotely and runs against whatever the profile
/// resolves; `ReadOnly` prefers remote when a session is available and falls
/// back to local with a warning; `Mutating` must route remotely or fail loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingClass {
    LocalOnly,
    ReadOnly,
    Mutating,
}

/// How a command relates to a directly supplied `--database-url`.
///
/// The flag bypasses profile resolution, but only a subset of commands can run
/// against a bare connection: `Direct` commands do, `ProfileDriven` commands
/// keep their profile-establishing path and ignore it, and `Unsupported`
/// commands fail before any connection is dialed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbUrlRouting {
    Direct,
    ProfileDriven,
    Unsupported,
}

/// The bootstrap requirements a parsed command declares before it runs.
pub trait DescribeCommand {
    fn descriptor(&self) -> CommandDescriptor;
}
