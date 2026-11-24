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

use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde::{Deserialize, Serialize};
use systemprompt_extension::cli::{
    CommandHost, CommandOutput, ConfirmArgs, TokenKeyPromotion, TokenKeySet,
};
use systemprompt_logging::CliService;

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

pub(super) fn execute_stage(host: &dyn CommandHost) -> Result<CommandOutput> {
    let next = host.signing_keys()?.stage_token_key()?;
    CliService::success(&format!(
        "Signing key {} staged. Restart every replica, then run `admin keys promote`.",
        next.staged_kid.as_deref().unwrap_or_default()
    ));
    Ok(output("Signing Key Staged", &next, false))
}

pub(super) fn execute_promote(args: PromoteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let next = host.signing_keys()?.promote_token_key(promotion(args))?;
    Ok(promoted(args, &next))
}

pub(super) fn execute_rotate(args: PromoteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let next = host.signing_keys()?.rotate_token_key(promotion(args))?;
    Ok(promoted(args, &next))
}

const fn promotion(args: PromoteArgs) -> TokenKeyPromotion {
    if args.revoke {
        TokenKeyPromotion::Revoke
    } else {
        TokenKeyPromotion::Overlap {
            secs: args.overlap_secs,
            force: args.force,
        }
    }
}

fn promoted(args: PromoteArgs, next: &TokenKeySet) -> CommandOutput {
    for kind in &next.cut_off {
        CliService::warning(&format!(
            "--force: {kind} signed with the outgoing key stop verifying at the overlap end"
        ));
    }
    if args.revoke {
        CliService::warning(
            "Promoted with no overlap: every token signed by an earlier key, and every previous \
             key, stops verifying once replicas restart.",
        );
    }
    CliService::success(&format!(
        "Signing key {} promoted; {} retired key(s) still verifying. Restart every replica.",
        next.active_kid,
        next.retired.len()
    ));
    output("Signing Key Promoted", next, args.revoke)
}

fn output(title: &str, set: &TokenKeySet, revoked: bool) -> CommandOutput {
    let output = KeySetOutput {
        active_kid: set.active_kid.clone(),
        staged_kid: set.staged_kid.clone(),
        revoked,
        retired: set
            .retired
            .iter()
            .map(|entry| RetiredKeyOutput {
                kid: entry.kid.clone(),
                expires_at: entry.expires_at,
            })
            .collect(),
    };
    CommandOutput::card_value(title, &output)
}
