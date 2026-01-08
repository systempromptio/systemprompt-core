use anyhow::{bail, Result};
use std::path::Path;

use systemprompt_cloud::constants::container;
use systemprompt_loader::ExtensionLoader;

/// Builder for generating Dockerfile content with MCP binary detection.
pub struct DockerfileBuilder<'a> {
    project_root: &'a Path,
    profile_name: Option<&'a str>,
}

impl<'a> DockerfileBuilder<'a> {
    pub const fn new(project_root: &'a Path) -> Self {
        Self {
            project_root,
            profile_name: None,
        }
    }

    pub const fn with_profile(mut self, name: &'a str) -> Self {
        self.profile_name = Some(name);
        self
    }

    pub fn build(&self) -> String {
        let mcp_section = self.mcp_copy_section();
        let env_section = self.env_section();

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
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 app
WORKDIR {app}

RUN mkdir -p {bin} {storage}/images/blog {storage}/images/social {storage}/images/logos {storage}/images/generated {storage}/files/audio {storage}/files/video {storage}/files/documents {storage}/files/uploads

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
            storage = container::STORAGE,
            services = container::SERVICES,
            web = container::WEB,
            profiles = container::PROFILES,
            mcp_section = mcp_section,
            env_section = env_section,
        )
    }

    fn mcp_copy_section(&self) -> String {
        let binaries = ExtensionLoader::get_mcp_binary_names(self.project_root);
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

pub fn get_required_mcp_copy_lines(project_root: &Path) -> Vec<String> {
    ExtensionLoader::get_mcp_binary_names(project_root)
        .iter()
        .map(|bin| format!("COPY target/release/{} {}/", bin, container::BIN))
        .collect()
}

pub fn validate_dockerfile_has_mcp_binaries(
    dockerfile_content: &str,
    project_root: &Path,
) -> Vec<String> {
    let has_wildcard = dockerfile_content.contains("target/release/systemprompt-*");
    if has_wildcard {
        return Vec::new();
    }

    ExtensionLoader::get_mcp_binary_names(project_root)
        .into_iter()
        .filter(|binary| {
            let expected_pattern = format!("target/release/{}", binary);
            !dockerfile_content.contains(&expected_pattern)
        })
        .collect()
}

pub fn print_dockerfile_suggestion(project_root: &Path) {
    systemprompt_core_logging::CliService::info(&generate_dockerfile_content(project_root));
}

pub fn validate_profile_dockerfile(dockerfile_path: &Path, project_root: &Path) -> Result<()> {
    if !dockerfile_path.exists() {
        bail!(
            "Dockerfile not found at {}\n\nCreate a profile first with: systemprompt cloud \
             profile create",
            dockerfile_path.display()
        );
    }

    let content = std::fs::read_to_string(dockerfile_path)?;
    let missing = validate_dockerfile_has_mcp_binaries(&content, project_root);

    if !missing.is_empty() {
        bail!(
            "Dockerfile at {} is missing COPY commands for MCP binaries:\n\n{}\n\nAdd these \
             lines:\n\n{}",
            dockerfile_path.display(),
            missing.join(", "),
            get_required_mcp_copy_lines(project_root).join("\n")
        );
    }

    Ok(())
}
