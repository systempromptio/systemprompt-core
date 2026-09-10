//! `services` command group: authoring, packaging and operating services
//! bundles.
//!
//! The group spans the two sides of a bundle's life. An editing repository
//! runs [`validate`], [`bundle`], [`keygen`] and [`publish`] in CI: they touch
//! only the filesystem and the registry, never a profile or a database.
//! An instance runs [`refresh`] and [`inspect`] against its own profile.
//!
//! [`refresh`] is the one command whose exit code carries meaning, because it
//! is written for a supervisor script rather than a reader:
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0 | every source resolved to the composition already active |
//! | 3 | at least one source changed; the new composition is now current |
//! | 1 | a source could not be resolved |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bundle;
pub mod inspect;
pub mod keygen;
pub mod publish;
pub mod reconcile;
pub mod refresh;
pub mod signing;
pub mod staging;
pub mod validate;
pub mod versions;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ServicesCommands {
    #[command(
        about = "Check a services tree loads, optionally against a base and a previous \
                       bundle"
    )]
    Validate(validate::ValidateArgs),

    #[command(about = "Pack a services tree into a signed bundle archive")]
    Bundle(bundle::BundleArgs),

    #[command(about = "Generate an ed25519 bundle signing key")]
    Keygen(keygen::KeygenArgs),

    #[command(about = "Publish a bundle archive to an OCI registry")]
    Publish(publish::PublishArgs),

    #[command(
        about = "Re-resolve the profile's bundle sources (exit 3 when the composition \
                       changed)"
    )]
    Refresh(refresh::RefreshArgs),

    #[command(about = "Print bundle provenance for an archive or the active composition")]
    Inspect(inspect::InspectArgs),
}

pub async fn execute(command: ServicesCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        ServicesCommands::Validate(args) => {
            let (output, ok) = validate::execute(&args)?;
            render_result(&output, &ctx.cli);
            if !ok {
                anyhow::bail!("Services validation failed");
            }
            Ok(())
        },
        ServicesCommands::Bundle(args) => {
            render_result(&bundle::execute(&args)?, &ctx.cli);
            Ok(())
        },
        ServicesCommands::Keygen(args) => {
            render_result(&keygen::execute(&args)?, &ctx.cli);
            Ok(())
        },
        ServicesCommands::Publish(args) => {
            render_result(&publish::execute(&args).await?, &ctx.cli);
            Ok(())
        },
        ServicesCommands::Refresh(args) => Box::pin(refresh::execute(&args, ctx)).await,
        ServicesCommands::Inspect(args) => {
            render_result(&inspect::execute(&args)?, &ctx.cli);
            Ok(())
        },
    }
}
