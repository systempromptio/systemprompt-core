//! `services keygen` — mint an ed25519 bundle signing key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use super::signing::BundleSigningKey;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct KeygenArgs {
    #[arg(
        long,
        help = "Write the base64 seed to this file instead of printing it"
    )]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct KeygenOutcome {
    pub public_key: String,
    pub key_id: String,
    pub seed: Option<String>,
    pub seed_file: Option<String>,
}

pub fn execute(args: &KeygenArgs) -> Result<CommandOutput> {
    let key = BundleSigningKey::generate();
    let outcome = write_key(&key, args.out.as_deref())?;
    Ok(CommandOutput::card_value("Bundle Signing Key", &outcome))
}

fn write_key(key: &BundleSigningKey, out: Option<&std::path::Path>) -> Result<KeygenOutcome> {
    let mut outcome = KeygenOutcome {
        public_key: key.public_key.clone(),
        key_id: key.key_id.clone(),
        seed: None,
        seed_file: None,
    };

    match out {
        Some(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }
            std::fs::write(path, key.seed_b64())
                .with_context(|| format!("Failed to write {}", path.display()))?;
            outcome.seed_file = Some(path.display().to_string());
        },
        None => outcome.seed = Some(key.seed_b64()),
    }
    Ok(outcome)
}
