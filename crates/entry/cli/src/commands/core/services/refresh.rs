//! `services refresh` — re-resolve the profile's bundle sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_loader::ServicesSourceBootstrap;
use systemprompt_loader::bundle::source::{AnyFetcher, BundleFetcher};
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_models::Profile;
use systemprompt_models::services::bundle::ServicesBundleState;

use super::reconcile::{ReconcileRow, reconcile_after_swap};
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

pub const EXIT_CHANGED: i32 = 3;

#[derive(Debug, Clone, Copy, Args)]
pub struct RefreshArgs {
    #[arg(
        long,
        help = "Only compare remote digests; fetch nothing and swap nothing"
    )]
    pub check: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    Unchanged,
    Changed,
}

#[must_use]
pub const fn exit_code_for(outcome: RefreshOutcome) -> i32 {
    match outcome {
        RefreshOutcome::Unchanged => 0,
        RefreshOutcome::Changed => EXIT_CHANGED,
    }
}

#[derive(Debug, Serialize)]
pub struct SourceRow {
    pub name: String,
    pub previous_digest: String,
    pub new_digest: String,
    pub version: String,
    pub changed: bool,
}

pub async fn execute(args: &RefreshArgs, ctx: &CommandContext) -> Result<()> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let cache = BundleCache::new(cache_root(profile));
    let before = cache.read_state();

    let (rows, outcome, reconciled) = if args.check {
        let (rows, outcome) = check_sources(profile, &before).await?;
        (rows, outcome, Vec::new())
    } else {
        swap_sources(profile, &cache, &before, ctx).await?
    };

    let title = if args.check {
        "Services Sources (check)"
    } else {
        "Services Sources"
    };
    render_result(
        &CommandOutput::table_of(
            vec![
                "name",
                "previous_digest",
                "new_digest",
                "version",
                "changed",
            ],
            &rows,
        )
        .with_title(title),
        &ctx.cli,
    );

    if !reconciled.is_empty() {
        render_result(
            &CommandOutput::table_of(
                vec![
                    "bundle",
                    "inserted",
                    "updated",
                    "deleted",
                    "protected",
                    "inert_role_rules",
                ],
                &reconciled,
            )
            .with_title("Access Rules Reconciled"),
            &ctx.cli,
        );
    }

    // Why: a supervisor script distinguishes "nothing to do" from "restart me"
    // by exit status, and every error path the binary has collapses to 1.
    let code = exit_code_for(outcome);
    if code != 0 {
        #[expect(
            clippy::exit,
            reason = "the documented exit code is this command's contract with a supervisor \
                      script; anyhow can only produce 1"
        )]
        std::process::exit(code);
    }
    Ok(())
}

async fn check_sources(
    profile: &Profile,
    before: &ServicesBundleState,
) -> Result<(Vec<SourceRow>, RefreshOutcome)> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("Failed to build an HTTP client")?;

    let mut rows = Vec::new();
    for source in &profile.services.sources {
        let auth = source.auth_secret().and_then(lookup_secret);
        let fetcher = AnyFetcher::from_source(source, auth, &client)
            .with_context(|| format!("Source {} is not usable", source.name))?;
        let remote = fetcher
            .head()
            .await
            .with_context(|| format!("Source {} could not be reached", source.name))?;
        let known = before.sources.get(&source.name);
        let previous = known.map_or_else(String::new, |s| s.digest.clone());
        rows.push(SourceRow {
            name: source.name.clone(),
            changed: remote.is_unknown() || previous != remote.digest,
            new_digest: remote.digest,
            previous_digest: previous,
            version: known.map_or_else(String::new, |s| s.version.clone()),
        });
    }
    let outcome = outcome_for(&rows);
    Ok((rows, outcome))
}

#[must_use]
pub fn outcome_for(rows: &[SourceRow]) -> RefreshOutcome {
    if rows.iter().any(|row| row.changed) {
        RefreshOutcome::Changed
    } else {
        RefreshOutcome::Unchanged
    }
}

async fn swap_sources(
    profile: &Profile,
    cache: &BundleCache,
    before: &ServicesBundleState,
    ctx: &CommandContext,
) -> Result<(Vec<SourceRow>, RefreshOutcome, Vec<ReconcileRow>)> {
    let root = ServicesSourceBootstrap::resolve(profile, lookup_secret, env!("CARGO_PKG_VERSION"))
        .await
        .context("Failed to resolve the services bundle sources")?;

    let after = cache.read_state();
    let rows = diff_states(before, &after);
    let outcome = outcome_for(&rows);

    let reconciled = if outcome == RefreshOutcome::Changed {
        reconcile_after_swap(profile, &root, cache, ctx).await?
    } else {
        Vec::new()
    };
    Ok((rows, outcome, reconciled))
}

#[must_use]
pub fn diff_states(before: &ServicesBundleState, after: &ServicesBundleState) -> Vec<SourceRow> {
    after
        .sources
        .iter()
        .map(|(name, state)| {
            let previous = before.sources.get(name);
            SourceRow {
                name: name.clone(),
                previous_digest: previous.map_or_else(String::new, |s| s.digest.clone()),
                changed: previous.is_none_or(|s| s.digest != state.digest),
                new_digest: state.digest.clone(),
                version: state.version.clone(),
            }
        })
        .collect()
}

fn lookup_secret(name: &str) -> Option<String> {
    SecretsBootstrap::get()
        .ok()
        .and_then(|secrets| secrets.get(name).cloned())
}
