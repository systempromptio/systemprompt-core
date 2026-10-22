//! Atomic file publication for generated output.
//!
//! Every file the generator writes into `dist/` (pages, feeds, sitemaps,
//! bundled CSS and JS) is served while it is being replaced, possibly by
//! another replica sharing the directory. Content is written to a sibling
//! temporary file and renamed over the target, so a reader sees either the
//! old file or the new one, never a truncated one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::io::AsyncWriteExt;

fn staging_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let unique = format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    );
    target.with_file_name(unique)
}

pub async fn write_atomic(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let staging = staging_path(target);
    let written = async {
        let mut file = fs::File::create(&staging).await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&staging, target).await
    }
    .await;
    if written.is_err()
        && let Err(cleanup) = fs::remove_file(&staging).await
        && cleanup.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(path = %staging.display(), error = %cleanup, "staging file left behind");
    }
    written
}

pub async fn copy_atomic(source: &Path, target: &Path) -> std::io::Result<u64> {
    let content = fs::read(source).await?;
    write_atomic(target, &content).await?;
    Ok(content.len() as u64)
}
