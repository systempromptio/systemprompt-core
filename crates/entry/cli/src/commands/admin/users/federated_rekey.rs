//! `admin users federated-rekey`: re-home federated links recorded under the
//! deployment's own issuer.
//!
//! A self-issued ID-JAG now resolves under the upstream issuer that vouched
//! for it, so links recorded under this deployment's `security.issuer` are
//! never matched again. With exactly one trusted issuer allowed to issue
//! ID-JAGs, those links move to it and keep their user; with none or several,
//! the upstream cannot be attributed and the links are removed, so the next
//! sign-in resolves through the issuer's email rule. Without `--apply` the
//! command only reports what it would do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_manifest::profile::TrustedIssuer;
use systemprompt_users::{IssuerRekeyOutcome, UserRepository};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, ConfirmArgs};

#[derive(Debug, Clone, Copy, Args)]
pub struct FederatedRekeyArgs {
    #[arg(long, help = "Apply the change (default: report only)")]
    pub apply: bool,

    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RekeyTarget {
    MoveTo(String),
    Remove { id_jag_issuers: usize },
}

#[must_use]
pub fn rekey_target(own_issuer: &str, trusted: &[TrustedIssuer]) -> RekeyTarget {
    let own = own_issuer.trim_end_matches('/');
    let candidates: Vec<&TrustedIssuer> = trusted
        .iter()
        .filter(|t| t.can_issue_id_jag && t.issuer.trim_end_matches('/') != own)
        .collect();
    match candidates.as_slice() {
        [only] => RekeyTarget::MoveTo(only.issuer.clone()),
        other => RekeyTarget::Remove {
            id_jag_issuers: other.len(),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FederatedRekeyOutput {
    pub own_issuer: String,
    pub action: String,
    pub target_issuer: Option<String>,
    pub rows: i64,
    pub already_linked_at_target: i64,
    pub applied: bool,
    pub moved: u64,
    pub removed: u64,
    pub message: String,
}

pub(super) async fn execute(
    args: FederatedRekeyArgs,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let profile = ctx.profile()?;
    let own_issuer = profile.security.issuer.clone();
    let target = rekey_target(&own_issuer, &profile.security.trusted_issuers);
    let pool = ctx.db_pool().await?;
    let repo = UserRepository::new(&pool);
    let output = run(&repo, &own_issuer, &target, args.apply).await?;
    Ok(CommandOutput::card_value("Federated Link Re-key", &output))
}

pub async fn run(
    repo: &UserRepository,
    own_issuer: &str,
    target: &RekeyTarget,
    apply: bool,
) -> Result<FederatedRekeyOutput> {
    let target_issuer = match target {
        RekeyTarget::MoveTo(issuer) => Some(issuer.clone()),
        RekeyTarget::Remove { .. } => None,
    };
    let plan = repo
        .plan_federated_issuer_rekey(own_issuer, target_issuer.as_deref())
        .await?;
    let outcome = if apply && plan.rows > 0 {
        match target {
            RekeyTarget::MoveTo(issuer) => repo.move_federated_issuer(own_issuer, issuer).await?,
            RekeyTarget::Remove { .. } => repo.remove_federated_issuer(own_issuer).await?,
        }
    } else {
        IssuerRekeyOutcome::default()
    };
    let (action, message) = describe(target, plan.rows, apply, outcome);
    Ok(FederatedRekeyOutput {
        own_issuer: own_issuer.to_owned(),
        action: action.to_owned(),
        target_issuer,
        rows: plan.rows,
        already_linked_at_target: plan.already_linked_at_target,
        applied: apply,
        moved: outcome.moved,
        removed: outcome.removed,
        message,
    })
}

fn describe(
    target: &RekeyTarget,
    rows: i64,
    apply: bool,
    outcome: IssuerRekeyOutcome,
) -> (&'static str, String) {
    match target {
        RekeyTarget::MoveTo(issuer) => (
            "move",
            if apply {
                format!(
                    "Moved {} link(s) to '{issuer}', removed {} already linked there",
                    outcome.moved, outcome.removed
                )
            } else {
                format!("{rows} link(s) would move to '{issuer}'. Re-run with --apply.")
            },
        ),
        RekeyTarget::Remove { id_jag_issuers } => (
            "remove",
            if apply {
                format!("Removed {} link(s)", outcome.removed)
            } else {
                format!(
                    "{rows} link(s) would be removed: {id_jag_issuers} trusted issuers may issue \
                     ID-JAGs, so the upstream cannot be attributed. Re-run with --apply."
                )
            },
        ),
    }
}
