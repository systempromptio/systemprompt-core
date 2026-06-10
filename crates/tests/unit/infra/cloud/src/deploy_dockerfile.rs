//! Unit tests for deploy Dockerfile generation against an expected fixture

use systemprompt_cloud::deploy::{DockerfileBuilder, generate_dockerfile_content};
use tempfile::TempDir;

const BASE_ENV_SECTION: &str = r#"ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="/app/bin:$PATH" \
    SYSTEMPROMPT_SERVICES_PATH=/app/services \
    SYSTEMPROMPT_TEMPLATES_PATH=/app/services/web/templates \
    SYSTEMPROMPT_ASSETS_PATH=/app/services/web/assets"#;

const PROFILE_ENV_SECTION: &str = r#"ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="/app/bin:$PATH" \
    SYSTEMPROMPT_PROFILE=/app/services/profiles/prod/profile.yaml \
    SYSTEMPROMPT_SERVICES_PATH=/app/services \
    SYSTEMPROMPT_TEMPLATES_PATH=/app/services/web/templates \
    SYSTEMPROMPT_ASSETS_PATH=/app/services/web/assets"#;

const MKDIR_PREFIX: &str = "RUN mkdir -p /app/bin /app/logs /app/storage/files/images \
                            /app/storage/files/images/generated /app/storage/files/images/logos \
                            /app/storage/files/audio /app/storage/files/video \
                            /app/storage/files/documents /app/storage/files/uploads /app/web";

fn expected_fixture(env_section: &str) -> String {
    format!(
        r#"# systemprompt.io Application Dockerfile
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
WORKDIR /app

<MKDIR>

# Copy pre-built binaries
COPY target/release/systemprompt /app/bin/

# Copy storage assets (images, etc.)
COPY storage /app/storage

# Copy web dist (generated HTML, CSS, JS)
COPY web/dist /app/web/dist

# Copy services configuration
COPY services /app/services

# Copy profiles
COPY .systemprompt/profiles /app/services/profiles
RUN chmod +x /app/bin/* && chown -R app:app /app

USER app
EXPOSE 8080

# Environment configuration
{env_section}

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["/app/bin/systemprompt", "infra", "services", "serve", "--foreground"]
"#
    )
}

// The `RUN mkdir -p` line carries extension storage directories discovered via
// the inventory registry, which depends on which extension crates the test
// binary happens to link. Compare it by prefix and everything else exactly.
fn assert_matches_fixture(content: &str, fixture_template: &str) {
    let mut content_lines = content.lines();
    for expected in fixture_template.lines() {
        let actual = content_lines.next().unwrap_or_else(|| {
            panic!("generated Dockerfile ended early; expected line: {expected}")
        });
        if expected == "<MKDIR>" {
            assert!(
                actual.starts_with(MKDIR_PREFIX),
                "mkdir line mismatch:\n  actual: {actual}\n  expected prefix: {MKDIR_PREFIX}"
            );
        } else {
            assert_eq!(actual, expected);
        }
    }
    assert_eq!(content_lines.next(), None);
}

#[test]
fn test_generated_dockerfile_matches_fixture() {
    let temp = TempDir::new().unwrap();
    let content = generate_dockerfile_content(temp.path());

    assert_matches_fixture(&content, &expected_fixture(BASE_ENV_SECTION));
}

#[test]
fn test_generated_dockerfile_with_profile_matches_fixture() {
    let temp = TempDir::new().unwrap();
    let content = DockerfileBuilder::new(temp.path())
        .with_profile("prod")
        .build();

    assert_matches_fixture(&content, &expected_fixture(PROFILE_ENV_SECTION));
}

#[test]
fn test_builder_and_helper_agree() {
    let temp = TempDir::new().unwrap();
    assert_eq!(
        generate_dockerfile_content(temp.path()),
        DockerfileBuilder::new(temp.path()).build()
    );
}
