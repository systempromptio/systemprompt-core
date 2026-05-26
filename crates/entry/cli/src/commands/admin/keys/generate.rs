use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_security::keys::RsaSigningKey;

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long, default_value = "signing_key.pem")]
    output: PathBuf,

    #[arg(long)]
    force: bool,
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "clap-derived args ergonomics"
)]
pub(super) fn execute(args: GenerateArgs) -> Result<()> {
    if args.output.exists() && !args.force {
        anyhow::bail!(
            "Refusing to overwrite existing key at {} (pass --force to replace)",
            args.output.display()
        );
    }

    let key = RsaSigningKey::generate().context("RSA keypair generation failed")?;
    key.write_pem_file(&args.output)
        .with_context(|| format!("writing PEM to {}", args.output.display()))?;

    CliService::output(&format!(
        "Wrote RSA-2048 PKCS#8 PEM to {}",
        args.output.display()
    ));
    CliService::output(&format!("kid: {}", key.kid()));
    Ok(())
}
