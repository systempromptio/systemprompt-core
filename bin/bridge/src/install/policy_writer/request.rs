//! Bounded, handle-validated policy-writer request input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Read};
use std::path::Path;

use super::{MAX_REQUEST_BYTES, PolicyWriteRequest, PolicyWriterError, REQUEST_VERSION};

pub fn read_request(path: &Path) -> Result<PolicyWriteRequest, PolicyWriterError> {
    let bytes = read_bytes(path).map_err(|source| PolicyWriterError::Io {
        context: format!("read request {}", path.display()),
        source,
    })?;
    let request: PolicyWriteRequest =
        serde_json::from_slice(&bytes).map_err(|e| PolicyWriterError::Io {
            context: format!("decode request {}", path.display()),
            source: io::Error::other(e),
        })?;
    if request.version != REQUEST_VERSION {
        return Err(PolicyWriterError::Version {
            actual: request.version,
        });
    }
    Ok(request)
}

fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::other("request is not a regular file"));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other("request is a reparse point"));
        }
    }
    if metadata.len() > MAX_REQUEST_BYTES {
        return Err(io::Error::other("request exceeds the size limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(io::Error::other("request exceeds the size limit"));
    }
    Ok(bytes)
}
