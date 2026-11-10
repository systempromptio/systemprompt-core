//! Bounded, handle-validated policy-writer request input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, Read};
use std::path::Path;

use super::{MAX_REQUEST_BYTES, PolicyWriteRequest, PolicyWriterError, REQUEST_VERSION};

pub fn read_request(path: &Path) -> Result<PolicyWriteRequest, PolicyWriterError> {
    let file = open_request(path).map_err(read_error(path))?;
    let bytes = read_capped(&file).map_err(read_error(path))?;
    decode_request(&bytes, path)
}

fn read_error(path: &Path) -> impl FnOnce(io::Error) -> PolicyWriterError + '_ {
    move |source| PolicyWriterError::Io {
        context: format!("read request {}", path.display()),
        source,
    }
}

pub fn decode_request(bytes: &[u8], path: &Path) -> Result<PolicyWriteRequest, PolicyWriterError> {
    let request: PolicyWriteRequest =
        serde_json::from_slice(bytes).map_err(|e| PolicyWriterError::Io {
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

// Why: the SYSTEM side reads the owner from the same handle it reads the
// bytes from, so the file it judged is the file it decodes.
#[cfg(target_os = "windows")]
pub(super) fn read_owned_request(
    path: &Path,
) -> Result<(PolicyWriteRequest, super::AccountSid), PolicyWriterError> {
    let file = open_request(path).map_err(read_error(path))?;
    let owner = super::owner::owner_of(&file).map_err(|source| PolicyWriterError::Io {
        context: format!("read the owner of {}", path.display()),
        source,
    })?;
    let owner = super::AccountSid::try_new(owner)?;
    let bytes = read_capped(&file).map_err(read_error(path))?;
    Ok((decode_request(&bytes, path)?, owner))
}

fn open_request(path: &Path) -> io::Result<std::fs::File> {
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
    Ok(file)
}

fn read_capped(file: &std::fs::File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err(io::Error::other("request exceeds the size limit"));
    }
    Ok(bytes)
}
