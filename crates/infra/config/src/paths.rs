use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_models::Config;

const CARGO_TARGET: &str = "target";

#[derive(Debug, Copy, Clone)]
pub struct BinaryPaths;

impl BinaryPaths {
    pub fn resolve_binary(binary_name: &str) -> Result<PathBuf> {
        if let Ok(mcp_path) = std::env::var("SYSTEMPROMPT_MCP_PATH") {
            let binary_path = PathBuf::from(&mcp_path).join(binary_name);
            if binary_path.exists() {
                return Self::ensure_absolute(binary_path);
            }
        }

        let config = Config::get()?;
        let target_dir = PathBuf::from(&config.system_path).join(CARGO_TARGET);

        let debug_path = target_dir.join("debug").join(binary_name);
        let release_path = target_dir.join("release").join(binary_name);

        match (release_path.exists(), debug_path.exists()) {
            (true, _) => Self::ensure_absolute(release_path),
            (_, true) => Self::ensure_absolute(debug_path),
            _ => bail!(
                "Binary '{}' not found:\n  - {}\n  - {}\n\nRun: cargo build --bin {}",
                binary_name,
                release_path.display(),
                debug_path.display(),
                binary_name
            ),
        }
    }

    fn ensure_absolute(path: PathBuf) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path)
        } else {
            std::fs::canonicalize(&path)
                .with_context(|| format!("Failed to canonicalize: {}", path.display()))
        }
    }

    pub fn binary_exists(binary_name: &str) -> bool {
        Self::resolve_binary(binary_name).is_ok()
    }

    pub fn resolve_binary_with_path(
        binary_name: &str,
        crate_path: Option<&Path>,
    ) -> Result<PathBuf> {
        if let Ok(mcp_path) = std::env::var("SYSTEMPROMPT_MCP_PATH") {
            let binary_path = PathBuf::from(&mcp_path).join(binary_name);
            if binary_path.exists() {
                return Self::ensure_absolute(binary_path);
            }
        }

        if let Some(crate_path) = crate_path {
            let release_path = crate_path.join("target/release").join(binary_name);
            let debug_path = crate_path.join("target/debug").join(binary_name);

            if release_path.exists() {
                return Self::ensure_absolute(release_path);
            }
            if debug_path.exists() {
                return Self::ensure_absolute(debug_path);
            }
        }

        let config = Config::get()?;
        let target_dir = PathBuf::from(&config.system_path).join(CARGO_TARGET);

        let debug_path = target_dir.join("debug").join(binary_name);
        let release_path = target_dir.join("release").join(binary_name);

        match (release_path.exists(), debug_path.exists()) {
            (true, _) => Self::ensure_absolute(release_path),
            (_, true) => Self::ensure_absolute(debug_path),
            _ => {
                let mut searched = vec![
                    format!("  - {}", release_path.display()),
                    format!("  - {}", debug_path.display()),
                ];
                if let Some(crate_path) = crate_path {
                    searched.insert(
                        0,
                        format!(
                            "  - {}/target/release/{}",
                            crate_path.display(),
                            binary_name
                        ),
                    );
                    searched.insert(
                        1,
                        format!("  - {}/target/debug/{}", crate_path.display(), binary_name),
                    );
                }
                bail!(
                    "Binary '{}' not found:\n{}\n\nRun: cargo build --bin {}",
                    binary_name,
                    searched.join("\n"),
                    binary_name
                )
            },
        }
    }
}
