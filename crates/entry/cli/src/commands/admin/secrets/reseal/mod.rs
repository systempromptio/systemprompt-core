//! `admin secrets reseal [--member <id>]`: re-seal a member's pre-0.88
//! ciphertext under that member's at-rest cipher, once, by core.
//!
//! The columns come from the members' admitted `ResealColumn` declarations
//! (`systemprompt_extension::AdmittedReseals`), never from the command line,
//! and each table must be recorded to the member in `schema_table_owners`.
//! Without `--member` the command reports every declaring member, core
//! members included. Core holds the master key: a value carrying the
//! `spmc1.` envelope that opens under the member's cipher is skipped; any
//! other value is opened under the pre-0.88 scheme (`at_rest::pre_088`, read
//! nowhere else) and sealed under the member's cipher. A value that opens
//! under neither is reported, left as it is, and makes the command fail.
//!
//! Without `--apply` nothing is written. With it, the command first plans
//! every column and writes one `governance_decisions` row for the run, with
//! the policy, member, per-column counts and operator, stopping if that write
//! fails; it then rewrites the planned rows in batches of
//! [`BATCH_ROWS`], one transaction per batch, each row only if it still holds
//! the ciphertext that was read. Output and logs carry counts and identifiers,
//! never a plaintext or key material.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_database::DbPool;
use systemprompt_database::admin::{Replacement, ResealStore, ResealTable};
use systemprompt_extension::cli::CommandOutput;
use systemprompt_extension::{AdmittedReseals, ResealColumn};
use systemprompt_governance::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_identifiers::{Actor, ContextId, ExtensionId};
use systemprompt_manifest::services::SystemAdmin;

use crate::kernel::KernelHost;

mod ciphers;

pub use ciphers::{ResealCiphers, RowState};

pub const BATCH_ROWS: i64 = 500;
const RESEAL_POLICY: &str = "member_reseal";

#[derive(Debug, Clone, Args)]
pub struct ResealArgs {
    #[arg(
        long,
        help = "The member whose declared columns are re-sealed (omit to report every declaring \
                member)"
    )]
    pub member: Option<String>,

    #[arg(long, help = "Write the re-sealed values (default: report only)")]
    pub apply: bool,
}

/// The counts of one declared column.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ColumnReport {
    pub table: String,
    pub column: String,
    pub total: u64,
    pub member_sealed: u64,
    pub resealed: u64,
    pub undecodable: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResealOutput {
    pub member: String,
    pub applied: bool,
    pub columns: Vec<ColumnReport>,
    pub message: String,
}

fn target_of(column: &ResealColumn) -> Result<ResealTable> {
    ResealTable::new(column.table, column.keys, column.ciphertext, column.nonce()).with_context(
        || {
            format!(
                "declared column {}.{} is not a valid identifier",
                column.table, column.ciphertext
            )
        },
    )
}

pub async fn check_recorded_owner(
    store: &ResealStore,
    column: &ResealColumn,
    member: &ExtensionId,
) -> Result<()> {
    let owner = store
        .recorded_owner(column.table)
        .await
        .with_context(|| format!("reading the recorded owner of {}", column.table))?;
    if owner.as_deref() != Some(member.as_str()) {
        bail!(
            "table '{}' is recorded to {} in schema_table_owners, not to member '{member}'; \
             nothing was read",
            column.table,
            owner.map_or_else(|| "no member".to_owned(), |owner| format!("'{owner}'"))
        );
    }
    Ok(())
}

pub async fn reseal_column(
    store: &ResealStore,
    column: &ResealColumn,
    ciphers: &ResealCiphers,
    apply: bool,
) -> Result<ColumnReport> {
    let target = target_of(column)?;
    let mut report = ColumnReport {
        table: column.table.to_owned(),
        column: column.ciphertext.to_owned(),
        ..ColumnReport::default()
    };
    let mut after: Option<Vec<String>> = None;
    loop {
        let rows = store
            .page(&target, after.as_deref(), BATCH_ROWS)
            .await
            .with_context(|| format!("reading {}.{}", column.table, column.ciphertext))?;
        let Some(last) = rows.last() else {
            break;
        };
        after = Some(last.keys.clone());
        let mut replacements = Vec::new();
        for row in &rows {
            report.total += 1;
            match ciphers.judge(column.layout, row) {
                RowState::MemberSealed => report.member_sealed += 1,
                RowState::Undecodable => report.undecodable += 1,
                RowState::Reseal(ciphertext) => {
                    if !apply {
                        report.resealed += 1;
                    }
                    replacements.push(Replacement { row, ciphertext });
                },
            }
        }
        if apply && !replacements.is_empty() {
            report.resealed += store
                .replace(&target, &replacements)
                .await
                .with_context(|| format!("writing {}.{}", column.table, column.ciphertext))?;
        }
    }
    Ok(report)
}

async fn audit(
    repository: &GovernanceDecisionRepository,
    operator: &SystemAdmin,
    member: &ExtensionId,
    planned: &[ColumnReport],
) -> Result<()> {
    let actor = Actor::job(operator.id().clone(), "cli:admin secrets reseal");
    let context = ContextId::generate();
    let id = format!("member-reseal-{}", context.as_str());
    let reason = format!("core re-sealed the declared ciphertext of {member}");
    let rules = serde_json::json!({
        "member": member.as_str(),
        "operator": operator.id().as_str(),
        "columns": planned,
    });
    repository
        .insert(&GovernanceDecisionRecord {
            id: &id,
            actor: &actor,
            session_id: None,
            tool_name: RESEAL_POLICY,
            agent_id: None,
            agent_scope: None,
            decision: DecisionTag::Allow,
            policy: RESEAL_POLICY,
            reason: &reason,
            evaluated_rules: &rules,
            plugin_id: None,
            act_chain: &[],
            context_id: &context,
            task_id: None,
            trace_id: None,
            client_id: None,
            tool_use_id: None,
        })
        .await
        .context("the re-seal could not be audited, so nothing was written")
}

async fn reseal_member(
    host: &dyn KernelHost,
    store: &ResealStore,
    member: &ExtensionId,
    columns: &[ResealColumn],
    apply: bool,
) -> Result<Vec<ColumnReport>> {
    for column in columns {
        check_recorded_owner(store, column, member).await?;
    }
    let secrets = host.secrets()?;
    let master = secrets
        .get("encryption_master_key")
        .ok_or_else(|| anyhow!("the profile's secrets hold no encryption_master_key"))?;
    let master = systemprompt_config::decode_master_key(master)
        .context("encryption_master_key is not a 32-byte hex key")?;
    let ciphers = ResealCiphers::new(master, member)?;
    let mut planned = Vec::with_capacity(columns.len());
    for column in columns {
        planned.push(reseal_column(store, column, &ciphers, false).await?);
    }
    if !apply || planned.iter().all(|report| report.resealed == 0) {
        return Ok(planned);
    }
    let db = super::super::state::get::<DbPool>(host).await?;
    let operator = super::super::state::get::<SystemAdmin>(host).await?;
    audit(
        &GovernanceDecisionRepository::from_pool(db.write_pool()),
        &operator,
        member,
        &planned,
    )
    .await?;
    let mut applied = Vec::with_capacity(columns.len());
    for column in columns {
        applied.push(reseal_column(store, column, &ciphers, true).await?);
    }
    Ok(applied)
}

pub(super) async fn execute(args: ResealArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let admitted = host.admitted()?;
    let reseals = AdmittedReseals::of(admitted.built())?;
    let members: Vec<ExtensionId> = match &args.member {
        Some(member) => {
            let member = ExtensionId::try_new(member.as_str())
                .map_err(|error| anyhow!("--member is not an extension id: {error}"))?;
            if !admitted.built().members().any(|m| m.owner == member) {
                bail!("member '{member}' is not in the admitted composition");
            }
            vec![member]
        },
        None if args.apply => bail!("--apply re-seals one member: pass --member <id>"),
        None => reseals.members().into_iter().cloned().collect(),
    };
    let db = super::super::state::get::<DbPool>(host).await?;
    let store = ResealStore::new(db.write_pool());
    let mut outputs = Vec::with_capacity(members.len());
    let mut undecodable: u64 = 0;
    for member in &members {
        let columns: Vec<ResealColumn> = reseals.of_member(member).into_iter().copied().collect();
        let reports = reseal_member(host, &store, member, &columns, args.apply).await?;
        for report in &reports {
            tracing::info!(
                member = %member,
                table = %report.table,
                column = %report.column,
                total = report.total,
                member_sealed = report.member_sealed,
                resealed = report.resealed,
                undecodable = report.undecodable,
                applied = args.apply,
                "member re-seal"
            );
        }
        undecodable += reports.iter().map(|report| report.undecodable).sum::<u64>();
        let resealed: u64 = reports.iter().map(|report| report.resealed).sum();
        let message = match (args.apply, columns.is_empty()) {
            (_, true) => format!("member '{member}' declares no re-seal columns"),
            (true, false) => format!("{resealed} value(s) re-sealed"),
            (false, false) => {
                format!("{resealed} value(s) would be re-sealed. Re-run with --apply.")
            },
        };
        outputs.push(ResealOutput {
            member: member.to_string(),
            applied: args.apply,
            columns: reports,
            message,
        });
    }
    if undecodable > 0 {
        bail!(
            "{undecodable} value(s) opened under neither the raw scheme nor the member cipher and \
             were left as they are: {}",
            serde_json::to_string(&outputs).unwrap_or_default()
        );
    }
    Ok(CommandOutput::card_value("Member Re-seal", &outputs))
}
