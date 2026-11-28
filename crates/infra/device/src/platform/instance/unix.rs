//! The endpoint macOS and Linux share: a Unix socket in the data root, mode
//! 0600. The two systems differ in how long a socket address may be, which is
//! the `LIMIT` each names.
//!
//! The primary holds the lock when it binds, so a socket file already there
//! belongs to an instance that is gone and is replaced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};
use tokio::time::{Instant, sleep};

use super::error::InstanceError;
use super::frame::{self, CONNECT_STEP, CONNECT_WAIT};
use super::{HandOff, Transport};

const SOCKET_FILE: &str = "instance.sock";

const OWNER_ONLY: u32 = 0o600;

#[derive(Debug)]
pub(super) struct SocketEndpoint<const LIMIT: usize> {
    listener: UnixListener,
    path: PathBuf,
}

// Why: a socket address is a fixed array that also holds the terminating NUL.
fn socket_path<const LIMIT: usize>(data_root: &Path) -> Result<PathBuf, InstanceError> {
    let path = data_root.join(SOCKET_FILE);
    if path.as_os_str().len() >= LIMIT {
        return Err(InstanceError::EndpointTooLong { path, limit: LIMIT });
    }
    Ok(path)
}

// Why: a launch that lost the lock can arrive before the primary has bound
// its socket, so an absent or refusing socket is retried for a moment.
async fn connect(path: &Path) -> Result<UnixStream, InstanceError> {
    let deadline = Instant::now() + CONNECT_WAIT;
    loop {
        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) && Instant::now() < deadline =>
            {
                sleep(CONNECT_STEP).await;
            },
            Err(source) => {
                return Err(InstanceError::Unreachable {
                    endpoint: path.to_path_buf(),
                    source,
                });
            },
        }
    }
}

impl<const LIMIT: usize> Transport for SocketEndpoint<LIMIT> {
    fn bind(data_root: &Path) -> Result<Self, InstanceError> {
        let path = socket_path::<LIMIT>(data_root)?;
        match fs::remove_file(&path) {
            Err(source) if source.kind() != io::ErrorKind::NotFound => {
                return Err(InstanceError::io("clear the stale socket", &path, source));
            },
            Ok(()) | Err(_) => {},
        }
        let listener =
            UnixListener::bind(&path).map_err(|source| InstanceError::io("bind", &path, source))?;
        fs::set_permissions(&path, Permissions::from_mode(OWNER_ONLY))
            .map_err(|source| InstanceError::io("restrict", &path, source))?;
        Ok(Self { listener, path })
    }

    async fn receive(&mut self) -> Result<HandOff, InstanceError> {
        let (mut stream, _address) = self
            .listener
            .accept()
            .await
            .map_err(|source| InstanceError::io("accept on", &self.path, source))?;
        frame::serve(&mut stream)
            .await
            .map_err(|source| InstanceError::Rejected { source })
    }

    async fn send(data_root: &Path, hand_off: &HandOff) -> Result<(), InstanceError> {
        let path = socket_path::<LIMIT>(data_root)?;
        let mut stream = connect(&path).await?;
        frame::deliver(&mut stream, hand_off)
            .await
            .map_err(|source| InstanceError::Rejected { source })
    }
}

impl<const LIMIT: usize> Drop for SocketEndpoint<LIMIT> {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::debug!(
                path = %self.path.display(),
                error = %error,
                "instance socket left behind"
            );
        }
    }
}
