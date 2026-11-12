//! `admin users federated-rekey`: re-home federated links recorded under the
//! deployment's own issuer.
//!
//! A self-issued ID-JAG now resolves under the upstream issuer that vouched
//! for it, so links recorded under this deployment's `security.issuer` are
//! never matched again. Which upstream wrote them is not recorded, so the
//! command never infers it: `--apply` needs either `--to-issuer`, naming a
//! currently trusted ID-JAG issuer the links move to (keeping their user), or
//! `--delete`, so the next sign-in resolves through the issuer's email rule.
//! Moving is only safe when the named issuer is the one that issued those
//! subjects. A link the target already holds for a different user is a
//! conflict and is left in place. Without `--apply` the command reports what
//! each choice would do.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_manifest::profile::TrustedIssuer;
use systemprompt_users::{IssuerRekeyOutcome, UserRepository};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, ConfirmArgs};

#[derive(Debug, Clone, Args)]
pub struct FederatedRekeyArgs {
    #[arg(long, help = "Apply the chosen action (default: report every choice)")]
    pub apply: bool,

    #[arg(
        long,
        value_name = "ISSUER",
        conflicts_with = "delete",
        help = "Move the links to this trusted ID-JAG issuer"
    )]
    pub to_issuer: Option<String>,

    #[arg(long, help = "Delete the links instead of moving them")]
    pub delete: bool,

    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RekeyChoice {
    MoveTo(String),
    Delete,
}

fn same_issuer(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

#[must_use]
pub fn id_jag_candidates(own_issuer: &str, trusted: &[TrustedIssuer]) -> Vec<String> {
    trusted
        .iter()
        .filter(|t| t.can_issue_id_jag && !same_issuer(&t.issuer, own_issuer))
        .map(|t| t.issuer.clone())
        .collect()
}

pub fn resolve_choice(
    candidates: &[String],
    to_issuer: Option<&str>,
    delete: bool,
    apply: bool,
) -> Result<Option<RekeyChoice>> {
    let choice = match (to_issuer, delete) {
        (Some(_), true) => bail!("--to-issuer and --delete are mutually exclusive"),
        (Some(issuer), false) => {
            let Some(known) = candidates.iter().find(|c| same_issuer(c, issuer)) else {
                bail!(
                    "'{issuer}' is not a trusted issuer that may issue ID-JAGs; trusted ID-JAG \
                     issuers: [{}]",
                    candidates.join(", ")
                );
            };
            Some(RekeyChoice::MoveTo(known.clone()))
        },
        (None, true) => Some(RekeyChoice::Delete),
        (None, false) => None,
    };
    if apply && choice.is_none() {
        bail!("--apply needs an explicit choice: --to-issuer <ISSUER> or --delete");
    }
    Ok(choice)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RekeyOption {
    pub action: String,
    pub target_issuer: Option<String>,
    pub rows: i64,
    pub already_linked_same_user: i64,
    pub conflicting_users: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FederatedRekeyOutput {
    pub own_issuer: String,
    pub options: Vec<RekeyOption>,
    pub applied: Option<String>,
    pub moved: u64,
    pub removed: u64,
    pub conflicts_kept: u64,
    pub message: String,
}

pub(super) async fn execute(
    args: FederatedRekeyArgs,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let profile = ctx.profile()?;
    let own_issuer = profile.security.issuer.clone();
    let candidates = id_jag_candidates(&own_issuer, &profile.security.trusted_issuers);
    let choice = resolve_choice(
        &candidates,
        args.to_issuer.as_deref(),
        args.delete,
        args.apply,
    )?;
    let pool = ctx.db_pool().await?;
    let repo = UserRepository::new(&pool);
    let apply = if args.apply { choice.as_ref() } else { None };
    let output = run(&repo, &own_issuer, &candidates, choice.as_ref(), apply).await?;
    Ok(CommandOutput::card_value("Federated Link Re-key", &output))
}

pub async fn run(
    repo: &UserRepository,
    own_issuer: &str,
    candidates: &[String],
    choice: Option<&RekeyChoice>,
    apply: Option<&RekeyChoice>,
) -> Result<FederatedRekeyOutput> {
    let targets: Vec<Option<&str>> = match choice {
        Some(RekeyChoice::MoveTo(issuer)) => vec![Some(issuer.as_str())],
        Some(RekeyChoice::Delete) => vec![None],
        None => candidates
            .iter()
            .map(|c| Some(c.as_str()))
            .chain(std::iter::once(None))
            .collect(),
    };
    let mut options = Vec::with_capacity(targets.len());
    for target in targets {
        let plan = repo.plan_federated_issuer_rekey(own_issuer, target).await?;
        options.push(RekeyOption {
            action: if target.is_some() { "move" } else { "delete" }.to_owned(),
            target_issuer: target.map(str::to_owned),
            rows: plan.rows,
            already_linked_same_user: plan.already_linked_same_user,
            conflicting_users: plan.conflicting_users,
        });
    }
    let outcome = match apply {
        Some(RekeyChoice::MoveTo(issuer)) => repo.move_federated_issuer(own_issuer, issuer).await?,
        Some(RekeyChoice::Delete) => repo.remove_federated_issuer(own_issuer).await?,
        None => IssuerRekeyOutcome::default(),
    };
    let message = match apply {
        Some(RekeyChoice::MoveTo(issuer)) => format!(
            "Moved {} link(s) to '{issuer}', removed {} the target already held for the same \
             user, kept {} that conflict with a different user",
            outcome.moved, outcome.removed, outcome.conflicts_kept
        ),
        Some(RekeyChoice::Delete) => format!("Deleted {} link(s)", outcome.removed),
        None => "Report only. Re-run with --apply and --to-issuer <ISSUER> or --delete; move only \
                 to the issuer that issued these subjects."
            .to_owned(),
    };
    Ok(FederatedRekeyOutput {
        own_issuer: own_issuer.to_owned(),
        options,
        applied: apply.map(|c| match c {
            RekeyChoice::MoveTo(_) => "move".to_owned(),
            RekeyChoice::Delete => "delete".to_owned(),
        }),
        moved: outcome.moved,
        removed: outcome.removed,
        conflicts_kept: outcome.conflicts_kept,
        message,
    })
}
