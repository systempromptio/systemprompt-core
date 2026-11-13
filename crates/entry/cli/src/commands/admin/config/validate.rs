//! `admin config validate` command: validate config files against their
//! schemas.
//!
//! Resolves a target to a single file, a [`ConfigSection`], or the full set of sections,
//! parses each as YAML, and validates a full profile document against the
//! [`systemprompt_manifest::profile::Profile`] schema; prints its JSON schema on demand.
//!
//! A full validation (no target) also parses, offline, the declaration of
//! every config plane an enabled extension registers ([`super::plane_validate`]).
//!
//! A full or section validation also loads the merged services tree and runs
//! its cross-checks ([`super::validate_services`]). The command's bootstrap
//! stops short of loading that tree, so a services failure is reported as a
//! `services` entry instead of aborting before the command runs.
//!
//! Whenever the scheduler section is in scope, the merged services config is
//! additionally cross-checked against the registered job catalog, so a job name
//! no `submit_job!` registers is caught here instead of aborting scheduler
//! startup on a deploy. That catalog is the set of jobs linked into the *CLI*
//! binary; a downstream that links job-registering crates only into its API
//! binary would see false positives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use systemprompt_logging::CliService;

use anyhow::{Result, anyhow};
use clap::Args;

use super::types::{ConfigFileInfo, ConfigSection, ConfigValidateOutput};
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use crate::shared::yaml_document::read_yaml_file;
use systemprompt_config::LoadedProfile;
use systemprompt_loader::BootContext;
use systemprompt_manifest::profile::Profile;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_scheduler::SchedulerConfig;

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(value_name = "PATH_OR_SECTION")]
    pub target: Option<String>,

    #[arg(long)]
    pub strict: bool,

    #[arg(
        long,
        help = "Print the generated JSON schema for the Profile config type instead of validating \
                any file"
    )]
    pub schema: bool,
}

pub fn execute(args: &ValidateArgs, ctx: &CommandContext) -> Result<(CommandOutput, bool)> {
    if args.schema {
        return print_profile_schema();
    }

    if let Some(path) = profile_document_target(args) {
        return validate_profile_file(&path);
    }

    let (mut results, mut all_valid) = validate_files(files_for_target(
        ctx.loaded_profile()?,
        args.target.as_deref(),
    )?);

    all_valid &= super::plane_validate::append_full_validation(
        ctx.config()?,
        &ctx.boot()?.profile().extensions.disabled,
        args.target.as_ref(),
        &mut results,
    );

    let services = if covers_services(args) {
        let (entry, services) = super::validate_services::load(ctx.boot()?);
        all_valid &= entry.valid;
        results.push(entry);
        services
    } else {
        None
    };

    if let Some(error) =
        scheduler_catalog_error(ctx.boot()?, services.as_ref(), &results, args.strict)
    {
        all_valid = false;
        for file in &mut results {
            if file.section == "scheduler" && file.error.is_none() {
                file.valid = false;
                file.error = Some(error.clone());
            }
        }
    }

    Ok(render(&ConfigValidateOutput {
        files: results,
        all_valid,
    }))
}

pub(super) fn profile_document_target(args: &ValidateArgs) -> Option<std::path::PathBuf> {
    let target = args.target.as_ref()?;
    let path = std::path::PathBuf::from(target);
    (path.exists() && is_yaml_file(&path) && target.parse::<ConfigSection>().is_err())
        .then_some(path)
}

fn covers_services(args: &ValidateArgs) -> bool {
    args.target
        .as_deref()
        .is_none_or(|target| target.parse::<ConfigSection>().is_ok())
}

fn files_for_target(
    loaded: &LoadedProfile,
    target: Option<&str>,
) -> Result<Vec<std::path::PathBuf>> {
    let Some(target) = target else {
        let mut all_files = Vec::new();
        for section in ConfigSection::all() {
            all_files.extend(section.all_files(loaded)?);
        }
        return Ok(all_files);
    };
    match target.parse::<ConfigSection>() {
        Ok(section) => Ok(section.all_files(loaded)?),
        Err(_) => Ok(vec![std::path::PathBuf::from(target)]),
    }
}

fn validate_files(files: Vec<std::path::PathBuf>) -> (Vec<ConfigFileInfo>, bool) {
    let mut results = Vec::new();
    let mut all_valid = true;
    for file_path in files {
        let section = detect_section(&file_path);
        let exists = file_path.exists();
        let error = if exists {
            read_yaml_file(&file_path).err().map(|e| format!("{e:#}"))
        } else {
            Some("File not found".to_owned())
        };
        let valid = error.is_none();
        all_valid &= valid;
        results.push(ConfigFileInfo {
            path: file_path.display().to_string(),
            section,
            exists,
            valid,
            error,
        });
    }
    (results, all_valid)
}

fn render(output: &ConfigValidateOutput) -> (CommandOutput, bool) {
    let all_valid = output.all_valid;
    let title = if all_valid {
        "Validation Passed"
    } else {
        "Validation Failed"
    };
    (
        CommandOutput::table_of(
            vec!["path", "section", "exists", "valid", "error"],
            &output.files,
        )
        .with_title(title),
        all_valid,
    )
}

fn print_profile_schema() -> Result<(CommandOutput, bool)> {
    let schema = schemars::schema_for!(Profile);
    let json = CliService::json_string(&schema)
        .map_err(|e| anyhow!("failed to serialize Profile JSON schema: {e}"))?;
    Ok((CommandOutput::text(json), true))
}

fn validate_profile_file(path: &std::path::Path) -> Result<(CommandOutput, bool)> {
    let loaded = Profile::load_from_path(path).and_then(|profile| {
        profile.validate_committed(path)?;
        Ok(profile)
    });

    match loaded {
        Ok(profile) => {
            let output = ConfigValidateOutput {
                files: vec![ConfigFileInfo {
                    path: path.display().to_string(),
                    section: "profile".to_owned(),
                    exists: true,
                    valid: true,
                    error: None,
                }],
                all_valid: true,
            };
            let title = format!("Profile '{}' is valid", profile.name);
            Ok((
                CommandOutput::table_of(
                    vec!["path", "section", "exists", "valid", "error"],
                    &output.files,
                )
                .with_title(title),
                true,
            ))
        },
        Err(e) => Err(anyhow!(
            "invalid profile {}: {e}\nThe error above names the offending field or value — fix it \
             and re-run.",
            path.display()
        )),
    }
}

fn scheduler_catalog_error(
    boot: &BootContext,
    services: Option<&ServicesConfig>,
    files: &[ConfigFileInfo],
    strict: bool,
) -> Option<String> {
    if !files
        .iter()
        .any(|file| file.section == "scheduler" && file.error.is_none())
    {
        return None;
    }
    let scheduler = services?
        .scheduler
        .clone()
        .unwrap_or_else(SchedulerConfig::with_system_admin);
    unknown_jobs_message(&scheduler).or_else(|| {
        strict
            .then(|| unlisted_jobs_message(&scheduler, boot))
            .flatten()
    })
}

pub fn unlisted_jobs_message(config: &SchedulerConfig, boot: &BootContext) -> Option<String> {
    let unlisted = systemprompt_scheduler::unlisted_job_names(config, boot);
    if unlisted.is_empty() {
        return None;
    }
    Some(format!(
        "registered job(s) with no scheduler.jobs entry: {}. Add an entry for each (enabled: \
         false keeps one off the schedule).",
        unlisted
            .iter()
            .map(systemprompt_identifiers::JobName::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub fn unknown_jobs_message(config: &SchedulerConfig) -> Option<String> {
    let unknown = systemprompt_scheduler::unknown_job_names(config);
    if unknown.is_empty() {
        return None;
    }
    Some(format!(
        "job(s) not registered via submit_job!: {}. Run `systemprompt infra jobs list` to see \
         every registered job.",
        unknown.join(", ")
    ))
}

fn is_yaml_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml" | "yml")
    )
}

fn detect_section(path: &std::path::Path) -> String {
    let path_str = path.display().to_string();

    if path_str.contains("/ai/") {
        "ai".to_owned()
    } else if path_str.contains("/content/") {
        "content".to_owned()
    } else if path_str.contains("/web/") {
        "web".to_owned()
    } else if path_str.contains("/scheduler/") {
        "scheduler".to_owned()
    } else if path_str.contains("/agents/") {
        "agents".to_owned()
    } else if path_str.contains("/mcp/") {
        "mcp".to_owned()
    } else if path_str.contains("/skills/") {
        "skills".to_owned()
    } else if path_str.contains("profile.yaml") {
        "profile".to_owned()
    } else if path_str.contains("/config/config.yaml") {
        "services".to_owned()
    } else {
        "unknown".to_owned()
    }
}
