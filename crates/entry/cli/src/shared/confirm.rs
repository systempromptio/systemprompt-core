//! The `--yes` flag every irreversible command carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;

/// `-y/--yes` for a command classified [`DataImpact::Irreversible`].
///
/// The runner reads it before dispatch; the command body never prompts.
///
/// [`DataImpact::Irreversible`]: crate::descriptor::DataImpact::Irreversible
#[derive(Debug, Clone, Copy, Default, Args)]
pub struct ConfirmArgs {
    #[arg(
        short = 'y',
        long = "yes",
        help = "Run without the confirmation prompt (required when not on a terminal)"
    )]
    pub yes: bool,
}
