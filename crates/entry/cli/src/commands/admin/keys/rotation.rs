//! `admin keys stage | promote | rotate`: two-phase rotation of the RS256
//! token key.
//!
//! `stage` writes a new key pair as the staged key, which every replica
//! restarted afterwards accepts and publishes but does not sign with.
//! `promote` makes the staged key the signer and keeps the outgoing key, public
//! half only, as a retired verification key until `--overlap-secs` from now
//! (default: the longest lifetime of any token it can have signed); unexpired
//! retired keys are kept and expired ones pruned. `rotate` is exactly `stage`
//! then `promote`, for a single-replica deployment. `--revoke` promotes with no
//! overlap and drops every retired key, for a compromised key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Args;
use serde::{Deserialize, Serialize};
use systemprompt_config::{SecretsService, SigningKeyState};
use systemprompt_logging::CliService;
use systemprompt_manifest::profile::SecurityConfig;
use systemprompt_security::jwt::{cut_off_by, rotation_overlap, signed_token_lifetimes};
use systemprompt_security::keys::{RotationMode, promote, stage};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, ConfirmArgs};

pub(super) const PROCEDURE: &str = "\
Multi-replica procedure (no replica ever meets a token signed by a key it does not know):
  1. `admin keys stage` writes a new key pair as the staged key in the profile's secrets
     file. Nothing signs with it yet.
  2. Restart every replica (cloud: `cloud deploy`). Signing keys are read once at boot and
     pinned against live secrets reload, so a replica learns of a key only by restarting.
     After the rollout every replica accepts, and the JWKS publishes, the staged key.
  3. `admin keys promote` makes the staged key the signer and keeps the outgoing key as a
     retired verification key until the overlap ends (default: the longest lifetime of any
     token it can have signed).
  4. Restart every replica again. Replicas not yet restarted keep signing with the outgoing
     key, which every replica still accepts; restarted ones sign with the new key, which
     every replica already accepts.
  Each retired key stops verifying, and leaves the JWKS, at its own expiry; unexpired
  retired keys survive later promotions, expired ones are pruned.
Single replica: `admin keys rotate` is exactly stage followed by promote; restart once.
Compromised key: `--revoke` (on promote or rotate) uses no overlap and removes every
retired key. After the restart every outstanding token signed by an earlier key,
including long-lived plugin tokens and bridge and CLI sessions, is rejected; re-issue
plugin tokens and sign in again.";

#[derive(Debug, Clone, Copy, Args)]
#[command(after_long_help = PROCEDURE)]
pub struct StageArgs {
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Debug, Clone, Copy, Args)]
#[command(after_long_help = PROCEDURE)]
pub struct PromoteArgs {
    #[command(flatten)]
    pub confirm: ConfirmArgs,

    #[arg(
        long,
        help = "Seconds the outgoing key keeps verifying tokens (default: the longest lifetime \
                of any token it can have signed)"
    )]
    pub overlap_secs: Option<i64>,

    #[arg(
        long,
        help = "Accept an overlap shorter than a signed token's lifetime, cutting those tokens off"
    )]
    pub force: bool,

    #[arg(
        long,
        conflicts_with_all = ["overlap_secs", "force"],
        help = "Compromised key: NO overlap, and every previous key is removed. Every \
                outstanding token signed by an earlier key is invalidated, including \
                long-lived plugin tokens, bridge and CLI sessions; re-issue them after the restart"
    )]
    pub revoke: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct KeySetOutput {
    pub active_kid: String,
    pub staged_kid: Option<String>,
    pub revoked: bool,
    pub retired: Vec<RetiredKeyOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RetiredKeyOutput {
    pub kid: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub(super) fn execute_stage(ctx: &CommandContext) -> Result<CommandOutput> {
    let state = current_state(ctx)?;
    let (staged, next) = stage(&state).context("Failed to stage a signing key")?;
    persist(ctx, &next)?;
    CliService::success(&format!(
        "Signing key {} staged. Restart every replica, then run `admin keys promote`.",
        staged.kid()
    ));
    output("Signing Key Staged", &next, false)
}

pub(super) fn execute_promote(args: PromoteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let state = current_state(ctx)?;
    promote_state(args, ctx, &state)
}

pub(super) fn execute_rotate(args: PromoteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let state = current_state(ctx)?;
    let (_, staged) = stage(&state).context("Failed to stage a signing key")?;
    promote_state(args, ctx, &staged)
}

fn promote_state(
    args: PromoteArgs,
    ctx: &CommandContext,
    state: &SigningKeyState,
) -> Result<CommandOutput> {
    let mode = if args.revoke {
        RotationMode::Revoke
    } else {
        RotationMode::Overlap(overlap_for(args, &ctx.profile()?.security)?)
    };
    let (promoted, next) =
        promote(state, mode, Utc::now()).context("Failed to promote the staged signing key")?;
    persist(ctx, &next)?;
    if args.revoke {
        CliService::warning(
            "Promoted with no overlap: every token signed by an earlier key, and every previous \
             key, stops verifying once replicas restart.",
        );
    }
    CliService::success(&format!(
        "Signing key {} promoted; {} retired key(s) still verifying. Restart every replica.",
        promoted.kid(),
        next.previous.len()
    ));
    output("Signing Key Promoted", &next, args.revoke)
}

fn current_state(ctx: &CommandContext) -> Result<SigningKeyState> {
    let boot = ctx.boot()?;
    let keys = crate::session::api::load_signing_keys(boot)?;
    let secrets = boot.secrets();
    Ok(SigningKeyState {
        active_private_pem: keys
            .signing_key()
            .to_pkcs8_pem()
            .context("Failed to encode the active signing key")?,
        staged_private_pem: secrets
            .signing_key_staged_pem()
            .context("Failed to read the staged signing key")?,
        previous: secrets
            .signing_keys_previous()
            .context("Failed to read the previous signing keys")?,
    })
}

fn persist(ctx: &CommandContext, state: &SigningKeyState) -> Result<()> {
    SecretsService::persist_signing_key_state(ctx.loaded_profile()?, state)
        .context("Failed to write the signing keys to the secrets file")
}

fn overlap_for(args: PromoteArgs, security: &SecurityConfig) -> Result<Duration> {
    let requested = args
        .overlap_secs
        .map(|secs| {
            Duration::try_seconds(secs)
                .with_context(|| format!("--overlap-secs {secs} is out of range"))
        })
        .transpose()?;
    let overlap = rotation_overlap(security, requested, args.force)?;
    for cut in cut_off_by(&signed_token_lifetimes(security), overlap) {
        CliService::warning(&format!(
            "--force: {} signed with the outgoing key stop verifying at the overlap end",
            cut.kind.description()
        ));
    }
    Ok(overlap)
}

fn output(title: &str, state: &SigningKeyState, revoked: bool) -> Result<CommandOutput> {
    let kid = |pem: &str| -> Result<String> {
        Ok(
            systemprompt_security::keys::RsaSigningKey::from_pkcs8_pem(pem)?
                .kid()
                .to_owned(),
        )
    };
    let output = KeySetOutput {
        active_kid: kid(&state.active_private_pem)?,
        staged_kid: state.staged_private_pem.as_deref().map(kid).transpose()?,
        revoked,
        retired: state
            .previous
            .iter()
            .map(|entry| RetiredKeyOutput {
                kid: entry.kid.clone(),
                expires_at: entry.expires_at,
            })
            .collect(),
    };
    Ok(CommandOutput::card_value(title, &output))
}
