//! Diagnostic-bundle assembly: logs, state files and feedback outboxes zipped
//! for support.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub(super) fn build_bundle(ctx: &crate::context::BridgeContext) -> io::Result<PathBuf> {
    let log_dir = crate::obs::log_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "log dir unavailable"))?;
    let dest_dir = crate::basedirs::desktop_dir()
        .or_else(crate::basedirs::home_dir)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home dir"))?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let zip_path = dest_dir.join(format!(
        "{}-diagnostics-{ts}.zip",
        crate::brand::brand().binary_name
    ));

    let file = fs::File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let include = name.starts_with("bridge.")
                || name.starts_with("bridge-crash-")
                || name == "activity.jsonl"
                || name == "activity.jsonl.1";
            if !include {
                continue;
            }
            if path.is_file() {
                add_file(&mut zip, &path, name, opts)?;
            }
        }
    }

    zip.start_file("diagnostics.txt", opts)?;
    zip.write_all(crate::buildinfo::render().as_bytes())?;

    zip.start_file("state.txt", opts)?;
    zip.write_all(crate::diagnostics_state::render(ctx).as_bytes())?;

    zip.start_file("registry.txt", opts)?;
    zip.write_all(crate::diagnostics_state::registry::render().as_bytes())?;

    for (name, path) in state_files() {
        if let Ok(bytes) = fs::read(&path) {
            zip.start_file(name, opts)?;
            zip.write_all(&bytes)?;
        }
    }

    for (index, path) in feedback_outboxes().iter().enumerate() {
        add_file(
            &mut zip,
            path,
            &format!("feedback-outbox-{index}.json"),
            opts,
        )?;
    }

    if let Some(yaml) = crate::config::redaction::redacted_config() {
        zip.start_file("config.redacted.toml", opts)?;
        zip.write_all(yaml.as_bytes())?;
    }

    zip.finish()?;
    Ok(zip_path)
}

// Why: these hold no secret, and a bundle that carries them verbatim lets the
// port record, install identity and sync checkpoint be compared across
// bundles without asking the user for another export.
fn state_files() -> Vec<(&'static str, PathBuf)> {
    let mut files = Vec::new();
    if let Some(path) = crate::proxy::portfile::portfile_path() {
        files.push(("bridge-proxy.json", path));
    }
    if let Some(path) = crate::proxy::identity::install_id_path() {
        files.push(("bridge-install.id", path));
    }
    if let Some(meta) = crate::config::paths::bridge_metadata_dir() {
        files.push((
            "last-sync.json",
            meta.join(crate::config::paths::LAST_SYNC_SENTINEL),
        ));
    }
    files
}

// Why: an evidence failure is decided by what the outbox already holds, so a
// bundle without it cannot explain one. `device.json` is the credential and
// stays out; outboxes carry digests and paths, never file bytes or tokens.
fn feedback_outboxes() -> Vec<PathBuf> {
    let Ok(root) = crate::feedback::metadata_root() else {
        return Vec::new();
    };
    let Ok(dir) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = dir
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().is_some_and(|name| name != "device.json")
        })
        .collect();
    paths.sort();
    paths
}

fn add_file(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    name: &str,
    opts: SimpleFileOptions,
) -> io::Result<()> {
    zip.start_file(name, opts)?;
    let buf = fs::read(path)?;
    zip.write_all(&buf)?;
    Ok(())
}
