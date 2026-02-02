use anyhow::Result;
use std::path::PathBuf;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

use super::types::{ProfileDiscoveryResult, ProfileEntryResult, ProfileInfo, ProfileSkipReason};

fn process_profile_entry(ctx: &ProjectContext, path: PathBuf) -> ProfileEntryResult {
    if !path.is_dir() {
        return ProfileEntryResult::NotDirectory;
    }

    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_string(),
        None => return ProfileEntryResult::Skip(ProfileSkipReason::InvalidDirectoryName { path }),
    };

    let profile_yaml = ctx.profile_path(&name, ProfilePath::Config);
    let secrets_json = ctx.profile_path(&name, ProfilePath::Secrets);

    if !profile_yaml.exists() {
        return ProfileEntryResult::Skip(ProfileSkipReason::MissingConfig { path: profile_yaml });
    }
    if !secrets_json.exists() {
        return ProfileEntryResult::Skip(ProfileSkipReason::MissingSecrets { path: secrets_json });
    }

    match load_database_url_from_secrets(&secrets_json, &name) {
        Ok(db_url) => ProfileEntryResult::Valid(ProfileInfo {
            name,
            database_url: db_url,
            path,
        }),
        Err(reason) => ProfileEntryResult::Skip(reason),
    }
}

fn load_database_url_from_secrets(
    secrets_json: &PathBuf,
    profile_name: &str,
) -> Result<String, ProfileSkipReason> {
    let content =
        std::fs::read_to_string(secrets_json).map_err(|e| ProfileSkipReason::SecretsReadError {
            path: secrets_json.clone(),
            error: e.to_string(),
        })?;

    let secrets: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| ProfileSkipReason::SecretsParseError {
            path: secrets_json.clone(),
            error: e.to_string(),
        })?;

    secrets
        .get("database_url")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ProfileSkipReason::MissingDatabaseUrl {
            profile: profile_name.to_string(),
        })
}

pub fn discover_profiles() -> Result<ProfileDiscoveryResult> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(ProfileDiscoveryResult {
            profiles: Vec::new(),
            skipped: Vec::new(),
        });
    }

    let mut profiles = Vec::new();
    let mut skipped = Vec::new();

    for entry in std::fs::read_dir(&profiles_dir)? {
        match process_profile_entry(&ctx, entry?.path()) {
            ProfileEntryResult::Valid(info) => profiles.push(info),
            ProfileEntryResult::Skip(reason) => skipped.push(reason),
            ProfileEntryResult::NotDirectory => {},
        }
    }

    Ok(ProfileDiscoveryResult { profiles, skipped })
}

pub fn print_discovery_summary(result: &ProfileDiscoveryResult, verbose: bool) {
    let found_count = result.profiles.len();
    let skipped_count = result.skipped.len();

    if found_count > 0 {
        CliService::info(&format!(
            "Found {} profile(s) with database configuration",
            found_count
        ));
    }

    if skipped_count > 0 {
        print_skipped_profiles(&result.skipped, verbose, skipped_count);
    }
}

fn print_skipped_profiles(skipped: &[ProfileSkipReason], verbose: bool, count: usize) {
    if verbose {
        CliService::warning(&format!("Skipped {} profile(s):", count));
        for reason in skipped {
            print_skip_reason(reason);
        }
    } else {
        CliService::info(&format!(
            "Skipped {} profile(s) (use -v for details)",
            count
        ));
    }
}

fn print_skip_reason(reason: &ProfileSkipReason) {
    match reason {
        ProfileSkipReason::MissingConfig { path } => {
            CliService::warning(&format!("  - Missing config: {}", path.display()));
        },
        ProfileSkipReason::MissingSecrets { path } => {
            CliService::warning(&format!("  - Missing secrets: {}", path.display()));
        },
        ProfileSkipReason::SecretsReadError { path, error } => {
            CliService::warning(&format!("  - Cannot read {}: {}", path.display(), error));
        },
        ProfileSkipReason::SecretsParseError { path, error } => {
            CliService::warning(&format!(
                "  - Invalid JSON in {}: {}",
                path.display(),
                error
            ));
        },
        ProfileSkipReason::MissingDatabaseUrl { profile } => {
            CliService::warning(&format!("  - No database_url in profile '{}'", profile));
        },
        ProfileSkipReason::InvalidDirectoryName { path } => {
            CliService::warning(&format!("  - Invalid directory name: {}", path.display()));
        },
    }
}
