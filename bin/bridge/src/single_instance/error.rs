//! Why acquiring the single-instance guard failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub(crate) enum SingletonError {
    #[error("{stage} {path}: {source}")]
    Lock {
        stage: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[cfg(windows)]
    #[error("CreateMutexW failed with status {status}")]
    Mutex { status: u32 },
}

#[cfg(unix)]
pub(super) fn lock_error(
    stage: &'static str,
    path: &std::path::Path,
    source: std::io::Error,
) -> super::SingletonResult {
    super::SingletonResult::Error(SingletonError::Lock {
        stage,
        path: path.display().to_string(),
        source,
    })
}
