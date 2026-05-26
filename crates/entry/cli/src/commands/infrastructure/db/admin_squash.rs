use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{Database, MigrationService, SquashPlan};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::Config;
use systemprompt_runtime::DatabaseContext;

use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, render_result};

use super::types::DbSquashOutput;

pub(super) struct SquashArgs<'a> {
    pub extension: &'a str,
    pub through: u32,
    pub apply: bool,
}

pub(super) async fn execute_squash(config: &CliConfig, args: SquashArgs<'_>) -> Result<()> {
    let sys_config = Config::get()?;

    let database = Arc::new(
        Database::from_config_with_write(
            &sys_config.database_type,
            &sys_config.database_url,
            sys_config.database_write_url.as_deref(),
        )
        .await
        .context("Failed to connect to database")?,
    );

    let registry = ExtensionRegistry::discover()?;
    run_squash(&registry, database.write(), config, &args).await
}

pub(super) async fn execute_squash_standalone(
    db_ctx: &DatabaseContext,
    config: &CliConfig,
    args: SquashArgs<'_>,
) -> Result<()> {
    let registry = ExtensionRegistry::discover()?;
    run_squash(&registry, db_ctx.db_pool().write(), config, &args).await
}

async fn run_squash(
    registry: &ExtensionRegistry,
    write_provider: &dyn DatabaseProvider,
    config: &CliConfig,
    args: &SquashArgs<'_>,
) -> Result<()> {
    let extension_id = args.extension;
    let through = args.through;
    let apply = args.apply;
    let ext = registry
        .get(extension_id)
        .ok_or_else(|| anyhow!("Extension '{}' not found in registry", extension_id))?;

    let migration_service = MigrationService::new(write_provider);
    let plan: SquashPlan = migration_service
        .squash_through(ext.as_ref(), through, apply)
        .await
        .map_err(|e| anyhow!("Squash failed: {}", e))?;

    let baseline_path = baseline_target_path(extension_id, through)?;
    let baseline_path_written = if apply {
        write_baseline_file(&baseline_path, &plan.baseline_sql)?;
        true
    } else {
        false
    };

    let follow_up = build_follow_up(&plan, &baseline_path, apply);
    let message = if apply {
        format!(
            "Squash applied: extension '{ext_id}' migrations 1..={through} retired; baseline \
             written to {path}",
            ext_id = plan.extension_id,
            through = plan.through,
            path = baseline_path.display(),
        )
    } else {
        format!(
            "Dry-run: would squash extension '{ext_id}' migrations 1..={through} (re-run with \
             --apply to commit; baseline target: {path})",
            ext_id = plan.extension_id,
            through = plan.through,
            path = baseline_path.display(),
        )
    };

    let output = DbSquashOutput {
        extension_id: plan.extension_id.clone(),
        through: plan.through,
        baseline_name: plan.baseline_name.clone(),
        baseline_checksum: plan.baseline_checksum.clone(),
        source_versions: plan.source_versions.clone(),
        already_applied_versions: plan.already_applied_versions.clone(),
        baseline_path: baseline_path.display().to_string(),
        baseline_path_written,
        applied: plan.applied,
        message: message.clone(),
        follow_up: follow_up.clone(),
    };

    if config.is_json_output() {
        let result = CommandResult::text(output).with_title("Database Migration Squash");
        render_result(&result);
    } else {
        render_squash_text(&plan, &baseline_path, &follow_up, &message, apply);
    }

    Ok(())
}

fn render_squash_text(
    plan: &SquashPlan,
    baseline_path: &Path,
    follow_up: &[String],
    message: &str,
    apply: bool,
) {
    if apply {
        CliService::success(message);
    } else {
        CliService::warning(message);
    }
    CliService::info(&format!(
        "  Source versions     : {:?}",
        plan.source_versions
    ));
    CliService::info(&format!(
        "  Already applied     : {:?}",
        plan.already_applied_versions
    ));
    CliService::info(&format!("  Baseline name       : {}", plan.baseline_name));
    CliService::info(&format!(
        "  Baseline checksum   : {}",
        plan.baseline_checksum
    ));
    CliService::info(&format!(
        "  Baseline file       : {}",
        baseline_path.display()
    ));
    CliService::info("");
    CliService::info("Follow-up steps:");
    for step in follow_up {
        CliService::info(&format!("  - {step}"));
    }
    if !apply {
        CliService::info("");
        CliService::info(
            "Dry-run only — no rows changed and no file written. Re-run with --apply.",
        );
    }
}

fn build_follow_up(plan: &SquashPlan, baseline_path: &Path, apply: bool) -> Vec<String> {
    let mut steps = Vec::new();
    if !apply {
        steps.push(format!(
            "Re-run with --apply to write {path} and rewrite extension_migrations rows.",
            path = baseline_path.display()
        ));
    }
    steps.push(format!(
        "Delete the squashed source files for migrations {versions:?} from the extension crate.",
        versions = plan.source_versions
    ));
    steps.push(format!(
        "In the extension's `extension.rs`, replace the squashed `Migration::new(...)` entries \
         with: Migration::new(0, \"{name}\", BASELINE_SQL) using `include_str!` of the new \
         baseline file.",
        name = plan.baseline_name
    ));
    steps.push(format!(
        "Bump any newly-added migrations so their version is > {through}.",
        through = plan.through
    ));
    steps
}

fn write_baseline_file(path: &Path, sql: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    std::fs::write(path, sql)
        .with_context(|| format!("Failed to write baseline SQL to {}", path.display()))?;
    Ok(())
}

fn baseline_target_path(extension_id: &str, through: u32) -> Result<PathBuf> {
    let crate_dir = locate_extension_crate(extension_id)?;
    Ok(crate_dir
        .join("schema")
        .join("migrations")
        .join(format!("000_baseline_v{through}.sql")))
}

fn locate_extension_crate(extension_id: &str) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to read current working directory")?;
    let repo_root = find_repo_root(&cwd).unwrap_or(cwd);

    let layers = ["domain", "infra", "app", "shared", "entry"];
    let mut tried = Vec::new();
    for layer in layers {
        let candidate = repo_root.join("crates").join(layer).join(extension_id);
        if candidate.join("Cargo.toml").is_file() {
            return Ok(candidate);
        }
        tried.push(candidate.display().to_string());
    }

    bail!(
        "Could not locate source crate for extension '{extension_id}'. Tried: {tried:?}. The \
         squash tool maps extension id → crate dir as crates/{{layer}}/{{id}}; if your extension \
         lives elsewhere, write the baseline file by hand."
    );
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join("Cargo.toml").is_file() && cur.join("crates").is_dir() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}
