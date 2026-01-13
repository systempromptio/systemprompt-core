use anyhow::{bail, Result};
use std::path::Path;

use systemprompt_cloud::constants::{container, storage};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::{ConfigLoader, ExtensionLoader};
use systemprompt_models::ServicesConfig;

use super::tenant::find_services_config;

#[derive(Debug)]
pub struct DockerfileBuilder<'a> {
    project_root: &'a Path,
    profile_name: Option<&'a str>,
    services_config: Option<ServicesConfig>,
}

impl<'a> DockerfileBuilder<'a> {
    pub fn new(project_root: &'a Path) -> Self {
        let services_config = find_services_config(project_root)
            .ok()
            .and_then(|path| ConfigLoader::load_from_path(&path).ok());
        Self {
            project_root,
            profile_name: None,
            services_config,
        }
    }

    pub const fn with_profile(mut self, name: &'a str) -> Self {
        self.profile_name = Some(name);
        self
    }

    pub fn build(&self) -> String {
        let mcp_section = self.mcp_copy_section();
        let env_section = self.env_section();
        let extension_dirs = Self::extension_storage_dirs();

        format!(
            r#"# SystemPrompt Application Dockerfile
# Built by: systemprompt cloud profile create
# Used by: systemprompt cloud deploy

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    libpq5 \
    lsof \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 app
WORKDIR {app}

RUN mkdir -p {bin} {logs} {storage}/{images} {storage}/{generated} {storage}/{logos} {storage}/{audio} {storage}/{video} {storage}/{documents} {storage}/{uploads}{extension_dirs}

# Copy pre-built binaries
COPY target/release/systemprompt {bin}/
{mcp_section}
# Copy web assets
COPY core/web/dist {web}/dist

# Copy storage assets (images, etc.)
COPY storage {storage}

# Copy services configuration
COPY services {services}

# Copy profiles
COPY .systemprompt/profiles {profiles}

RUN chmod +x {bin}/* && chown -R app:app {app}

USER app
EXPOSE 8080

# Environment configuration
{env_section}

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["{bin}/systemprompt", "services", "serve", "--foreground"]
"#,
            app = container::APP,
            bin = container::BIN,
            logs = container::LOGS,
            storage = container::STORAGE,
            services = container::SERVICES,
            web = container::WEB,
            profiles = container::PROFILES,
            images = storage::IMAGES,
            generated = storage::GENERATED,
            logos = storage::LOGOS,
            audio = storage::AUDIO,
            video = storage::VIDEO,
            documents = storage::DOCUMENTS,
            uploads = storage::UPLOADS,
            extension_dirs = extension_dirs,
            mcp_section = mcp_section,
            env_section = env_section,
        )
    }

    fn extension_storage_dirs() -> String {
        let registry = ExtensionRegistry::discover();
        let paths = registry.all_required_storage_paths();
        if paths.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        for path in paths {
            result.push(' ');
            result.push_str(container::STORAGE);
            result.push('/');
            result.push_str(path);
        }
        result
    }

    fn mcp_copy_section(&self) -> String {
        let binaries = self.services_config.as_ref().map_or_else(
            || ExtensionLoader::get_mcp_binary_names(self.project_root),
            |config| ExtensionLoader::get_production_mcp_binary_names(self.project_root, config),
        );

        if binaries.is_empty() {
            return String::new();
        }

        let lines: Vec<String> = binaries
            .iter()
            .map(|bin| format!("COPY target/release/{} {}/", bin, container::BIN))
            .collect();

        format!("\n# Copy MCP server binaries\n{}\n", lines.join("\n"))
    }

    fn env_section(&self) -> String {
        let profile_env = self
            .profile_name
            .map(|name| {
                format!(
                    "    SYSTEMPROMPT_PROFILE={}/{}/profile.yaml \\",
                    container::PROFILES,
                    name
                )
            })
            .unwrap_or_default();

        if profile_env.is_empty() {
            format!(
                r#"ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="{}:$PATH" \
    SYSTEMPROMPT_SERVICES_PATH={} \
    WEB_DIR={}"#,
                container::BIN,
                container::SERVICES,
                container::WEB
            )
        } else {
            format!(
                r#"ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="{}:$PATH" \
{}
    SYSTEMPROMPT_SERVICES_PATH={} \
    WEB_DIR={}"#,
                container::BIN,
                profile_env,
                container::SERVICES,
                container::WEB
            )
        }
    }
}

pub fn generate_dockerfile_content(project_root: &Path) -> String {
    DockerfileBuilder::new(project_root).build()
}

pub fn get_required_mcp_copy_lines(
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
        .iter()
        .map(|bin| format!("COPY target/release/{} {}/", bin, container::BIN))
        .collect()
}

fn extract_mcp_binary_names_from_dockerfile(dockerfile_content: &str) -> Vec<String> {
    dockerfile_content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with("COPY target/release/systemprompt-") {
                return None;
            }
            let after_copy = trimmed.strip_prefix("COPY target/release/")?;
            let binary_name = after_copy.split_whitespace().next()?;
            if binary_name.starts_with("systemprompt-") && binary_name != "systemprompt-*" {
                Some(binary_name.to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn validate_dockerfile_has_mcp_binaries(
    dockerfile_content: &str,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    let has_wildcard = dockerfile_content.contains("target/release/systemprompt-*");
    if has_wildcard {
        return Vec::new();
    }

    ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
        .into_iter()
        .filter(|binary| {
            let expected_pattern = format!("target/release/{}", binary);
            !dockerfile_content.contains(&expected_pattern)
        })
        .collect()
}

pub fn validate_dockerfile_has_no_stale_binaries(
    dockerfile_content: &str,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Vec<String> {
    let has_wildcard = dockerfile_content.contains("target/release/systemprompt-*");
    if has_wildcard {
        return Vec::new();
    }

    let dockerfile_binaries = extract_mcp_binary_names_from_dockerfile(dockerfile_content);
    let current_binaries: std::collections::HashSet<String> =
        ExtensionLoader::get_production_mcp_binary_names(project_root, services_config)
            .into_iter()
            .collect();

    dockerfile_binaries
        .into_iter()
        .filter(|binary| !current_binaries.contains(binary))
        .collect()
}

pub fn print_dockerfile_suggestion(project_root: &Path) {
    systemprompt_core_logging::CliService::info(&generate_dockerfile_content(project_root));
}

pub fn validate_profile_dockerfile(
    dockerfile_path: &Path,
    project_root: &Path,
    services_config: &ServicesConfig,
) -> Result<()> {
    if !dockerfile_path.exists() {
        bail!(
            "Dockerfile not found at {}\n\nCreate a profile first with: systemprompt cloud \
             profile create",
            dockerfile_path.display()
        );
    }

    let content = std::fs::read_to_string(dockerfile_path)?;
    let missing = validate_dockerfile_has_mcp_binaries(&content, project_root, services_config);
    let stale = validate_dockerfile_has_no_stale_binaries(&content, project_root, services_config);

    match (missing.is_empty(), stale.is_empty()) {
        (true, true) => Ok(()),
        (false, true) => {
            bail!(
                "Dockerfile at {} is missing COPY commands for MCP binaries:\n\n{}\n\nAdd these \
                 lines:\n\n{}",
                dockerfile_path.display(),
                missing.join(", "),
                get_required_mcp_copy_lines(project_root, services_config).join("\n")
            );
        },
        (true, false) => {
            bail!(
                "Dockerfile at {} has COPY commands for dev-only or removed \
                 binaries:\n\n{}\n\nRemove these lines or regenerate with: systemprompt cloud \
                 profile create",
                dockerfile_path.display(),
                stale.join(", ")
            );
        },
        (false, false) => {
            bail!(
                "Dockerfile at {} has issues:\n\nMissing binaries: {}\nDev-only/stale binaries: \
                 {}\n\nRegenerate with: systemprompt cloud profile create",
                dockerfile_path.display(),
                missing.join(", "),
                stale.join(", ")
            );
        },
    }
}
