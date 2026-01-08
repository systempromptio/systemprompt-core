use anyhow::{bail, Result};
use std::path::Path;

use systemprompt_loader::ExtensionLoader;

const DOCKERFILE_HEADER: &str = r"# SystemPrompt Cloud Image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libpq5 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 app
WORKDIR /app

RUN mkdir -p /app/bin /app/data /app/logs /app/storage/images/blog /app/storage/images/social /app/storage/images/logos /app/storage/images/generated /app/storage/files/audio /app/storage/files/video /app/storage/files/documents /app/storage/files/uploads

COPY target/release/systemprompt /app/bin/
";

#[allow(clippy::needless_raw_string_hashes)]
const DOCKERFILE_FOOTER: &str = r#"
COPY services /app/services
COPY .systemprompt/profiles /app/services/profiles
COPY .systemprompt/entrypoint.sh /app/entrypoint.sh
COPY core/web/dist /app/web/dist
COPY core/web/src/assets/images /app/storage/images

RUN chmod +x /app/bin/* /app/entrypoint.sh && chown -R app:app /app

USER app
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="/app/bin:$PATH" \
    SYSTEMPROMPT_SERVICES_PATH=/app/services \
    WEB_DIR=/app/web

CMD ["/app/bin/systemprompt", "services", "serve", "--foreground"]
"#;

pub fn generate_dockerfile_content(project_root: &Path) -> String {
    let mcp_binaries = ExtensionLoader::get_mcp_binary_names(project_root);

    let mcp_section = if mcp_binaries.is_empty() {
        String::new()
    } else {
        mcp_binaries
            .iter()
            .map(|bin| format!("COPY target/release/{} /app/bin/", bin))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!("{}{}{}", DOCKERFILE_HEADER, mcp_section, DOCKERFILE_FOOTER)
}

pub fn get_required_mcp_copy_lines(project_root: &Path) -> Vec<String> {
    ExtensionLoader::get_mcp_binary_names(project_root)
        .iter()
        .map(|bin| format!("COPY target/release/{} /app/bin/", bin))
        .collect()
}

pub fn validate_dockerfile_has_mcp_binaries(
    dockerfile_content: &str,
    project_root: &Path,
) -> Vec<String> {
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

pub fn check_dockerfile_completeness(project_root: &Path) -> Result<()> {
    let ctx = systemprompt_cloud::ProjectContext::new(project_root.to_path_buf());
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(());
    }

    let profile_dockerfiles: Vec<_> = std::fs::read_dir(&profiles_dir)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let dockerfile = entry.path().join("docker").join("Dockerfile");
            dockerfile.exists().then_some(dockerfile)
        })
        .collect();

    if profile_dockerfiles.is_empty() {
        return Ok(());
    }

    for dockerfile_path in profile_dockerfiles {
        let content = std::fs::read_to_string(&dockerfile_path)?;
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
    }

    Ok(())
}
