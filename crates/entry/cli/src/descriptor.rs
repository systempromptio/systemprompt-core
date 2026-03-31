#[derive(Debug, Clone, Copy, Default)]
pub struct CommandDescriptor {
    flags: u8,
}

impl CommandDescriptor {
    const FLAG_PROFILE: u8 = 0b0000_0001;
    const FLAG_SECRETS: u8 = 0b0000_0010;
    const FLAG_PATHS: u8 = 0b0000_0100;
    const FLAG_DATABASE: u8 = 0b0000_1000;
    const FLAG_REMOTE_ELIGIBLE: u8 = 0b0001_0000;
    const FLAG_SKIP_VALIDATION: u8 = 0b0010_0000;

    pub const NONE: Self = Self { flags: 0 };

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

    pub const fn remote_eligible(&self) -> bool {
        self.flags & Self::FLAG_REMOTE_ELIGIBLE != 0
    }

    pub const fn skip_validation(&self) -> bool {
        self.flags & Self::FLAG_SKIP_VALIDATION != 0
    }

    pub const fn with_remote_eligible(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_REMOTE_ELIGIBLE,
        }
    }

    pub const fn with_skip_validation(self) -> Self {
        Self {
            flags: self.flags | Self::FLAG_SKIP_VALIDATION,
        }
    }
}

pub trait DescribeCommand {
    fn descriptor(&self) -> CommandDescriptor;
}
