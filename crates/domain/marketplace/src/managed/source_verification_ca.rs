//! Explicit private-source CA trust is copied from an atomically opened regular
//! file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{GitTreeRead, GitTreeReader, NativeGitTreeReader, import_source};
use crate::managed::{ManagedError, Result, RevisionFiles};

struct CertificateGitTreeReader {
    certificate: Vec<u8>,
}

impl NativeGitTreeReader {
    pub fn with_certificate_authority(
        path: &std::path::Path,
    ) -> Result<impl GitTreeReader + use<>> {
        Ok(CertificateGitTreeReader {
            certificate: read_certificate(path)?,
        })
    }
}

#[cfg(unix)]
fn read_certificate(path: &std::path::Path) -> Result<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    // Why: opening a FIFO can block before metadata validation; O_NONBLOCK permits
    // checking the opened file.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 1024 * 1024 {
        return Err(ManagedError::Integrity);
    }
    let mut certificate = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut certificate)?;
    if certificate.len() as u64 != metadata.len() {
        return Err(ManagedError::Integrity);
    }
    Ok(certificate)
}

#[cfg(not(unix))]
fn read_certificate(_path: &std::path::Path) -> Result<Vec<u8>> {
    Err(ManagedError::Unavailable)
}

impl GitTreeReader for CertificateGitTreeReader {
    fn read(&self, request: &GitTreeRead<'_>) -> Result<RevisionFiles> {
        import_source(request, Some(&self.certificate))
    }
}
