#[derive(Debug, Clone, Copy, Default)]
pub struct CommandDescriptor {
    pub profile: bool,
    pub secrets: bool,
    pub paths: bool,
    pub database: bool,
    pub remote_eligible: bool,
    pub skip_validation: bool,
}

impl CommandDescriptor {
    pub const NONE: Self = Self {
        profile: false,
        secrets: false,
        paths: false,
        database: false,
        remote_eligible: false,
        skip_validation: false,
    };

    pub const PROFILE_ONLY: Self = Self {
        profile: true,
        secrets: false,
        paths: false,
        database: false,
        remote_eligible: false,
        skip_validation: false,
    };

    pub const PROFILE_AND_SECRETS: Self = Self {
        profile: true,
        secrets: true,
        paths: false,
        database: false,
        remote_eligible: false,
        skip_validation: false,
    };

    pub const PROFILE_SECRETS_AND_PATHS: Self = Self {
        profile: true,
        secrets: true,
        paths: true,
        database: false,
        remote_eligible: false,
        skip_validation: false,
    };

    pub const FULL: Self = Self {
        profile: true,
        secrets: true,
        paths: true,
        database: true,
        remote_eligible: true,
        skip_validation: false,
    };

    pub const fn with_remote_eligible(self) -> Self {
        Self {
            profile: self.profile,
            secrets: self.secrets,
            paths: self.paths,
            database: self.database,
            remote_eligible: true,
            skip_validation: self.skip_validation,
        }
    }

    pub const fn with_skip_validation(self) -> Self {
        Self {
            profile: self.profile,
            secrets: self.secrets,
            paths: self.paths,
            database: self.database,
            remote_eligible: self.remote_eligible,
            skip_validation: true,
        }
    }
}

pub trait DescribeCommand {
    fn descriptor(&self) -> CommandDescriptor;
}
