//! Command requirements specification.
//!
//! This module provides a trait-based system for commands to declare their
//! initialization requirements (profile, secrets, paths), replacing scattered
//! `requires_*` methods with a unified interface.

/// Specifies what initialization a command requires.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandRequirements {
    /// Whether the command needs a profile to be loaded.
    pub profile: bool,
    /// Whether the command needs secrets to be initialized.
    pub secrets: bool,
    /// Whether the command needs application paths to be initialized.
    pub paths: bool,
}

impl CommandRequirements {
    /// Command requires nothing - standalone operation.
    pub const NONE: Self = Self {
        profile: false,
        secrets: false,
        paths: false,
    };

    /// Command requires only a profile to be loaded.
    pub const PROFILE_ONLY: Self = Self {
        profile: true,
        secrets: false,
        paths: false,
    };

    /// Command requires profile and secrets but not paths.
    pub const PROFILE_AND_SECRETS: Self = Self {
        profile: true,
        secrets: true,
        paths: false,
    };

    /// Command requires full initialization (profile, secrets, and paths).
    pub const FULL: Self = Self {
        profile: true,
        secrets: true,
        paths: true,
    };
}

/// Trait for commands to declare their initialization requirements.
pub trait HasRequirements {
    /// Returns the requirements for this command.
    fn requirements(&self) -> CommandRequirements;
}
