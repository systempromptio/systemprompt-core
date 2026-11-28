//! Windows: a named pipe whose name is derived from the data root.
//!
//! The primary claims the name by creating its first instance, and every
//! instance refuses remote clients and carries a protected DACL that grants
//! access to its owner alone. Each launch is served on an instance of its own:
//! once a launch connects, a fresh instance takes over the listening, so no
//! instance outlives its launch and nothing one launch left on a pipe reaches
//! the next. A launch connects at identification level, so a process that
//! took the name first learns who called and can do nothing as them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "the pipe's security descriptor is raw Win32 FFI"
)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::{io, mem};

use sha2::{Digest as _, Sha256};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use tokio::time::{Instant, sleep};
use windows_sys::Win32::Foundation::{ERROR_PIPE_BUSY, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION;

use super::error::InstanceError;
use super::frame::{self, CONNECT_STEP, CONNECT_WAIT};
use super::{HandOff, Transport};

const PIPE_PREFIX: &str = r"\\.\pipe\systemprompt-instance-";

const NAME_DIGEST_BYTES: usize = 16;

// Why: SDDL for a protected DACL with one entry, all access for the owner
// rights identity, which stands for whoever owns the object.
const OWNER_ONLY: &str = "D:P(A;;GA;;;OW)";

const SDDL_REVISION: u32 = 1;

// Why: one instance serves a launch while the next listens, and an instance
// whose launch stalled closes only once its cancelled read has completed.
const INSTANCES: usize = 3;

struct Descriptor(*mut c_void);

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: the pointer is a descriptor Windows allocated for the caller
        // to release with LocalFree; it is released exactly once, here.
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn owner_only() -> io::Result<Descriptor> {
    let sddl: Vec<u16> = OWNER_ONLY.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = null_mut();
    // SAFETY: the SDDL is NUL terminated and the output slot is a live local;
    // the descriptor Windows allocates is owned by the returned Descriptor.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION,
            &raw mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Descriptor(descriptor))
}

#[derive(Debug)]
pub(super) struct Endpoint {
    server: NamedPipeServer,
    name: PathBuf,
}

fn pipe_name(data_root: &Path) -> PathBuf {
    let digest = Sha256::digest(data_root.as_os_str().as_encoded_bytes());
    let mut name = String::from(PIPE_PREFIX);
    for byte in digest.iter().take(NAME_DIGEST_BYTES) {
        name.push_str(&format!("{byte:02x}"));
    }
    PathBuf::from(name)
}

// Why: a launch that lost the lock can arrive before the primary has created
// its pipe, or while every instance of the pipe is taken by another launch.
async fn connect(name: &Path) -> Result<NamedPipeClient, InstanceError> {
    let deadline = Instant::now() + CONNECT_WAIT;
    loop {
        match ClientOptions::new()
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(name)
        {
            Ok(client) => return Ok(client),
            Err(error)
                if (error.kind() == io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(ERROR_PIPE_BUSY.cast_signed()))
                    && Instant::now() < deadline =>
            {
                sleep(CONNECT_STEP).await;
            },
            Err(source) => {
                return Err(InstanceError::Unreachable {
                    endpoint: name.to_path_buf(),
                    source,
                });
            },
        }
    }
}

// Why: the first instance must find the name free, so a process that took it
// first is an error and not a pipe shared with a stranger; later instances
// join the name the primary already owns.
fn create(name: &Path, first: bool) -> Result<NamedPipeServer, InstanceError> {
    let descriptor =
        owner_only().map_err(|source| InstanceError::io("describe the access of", name, source))?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    // SAFETY: the attributes are a live SECURITY_ATTRIBUTES whose descriptor
    // stays allocated until `descriptor` drops after the call, and Windows
    // copies the descriptor into the pipe it creates.
    let server = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(INSTANCES)
            .create_with_security_attributes_raw(name, (&raw mut attributes).cast())
    }
    .map_err(|source| InstanceError::io("create", name, source))?;
    drop(descriptor);
    Ok(server)
}

impl Transport for Endpoint {
    fn bind(data_root: &Path) -> Result<Self, InstanceError> {
        let name = pipe_name(data_root);
        let server = create(&name, true)?;
        Ok(Self { server, name })
    }

    async fn receive(&mut self) -> Result<HandOff, InstanceError> {
        self.server
            .connect()
            .await
            .map_err(|source| InstanceError::io("await a launch on", &self.name, source))?;
        let listening = create(&self.name, false)?;
        let mut launch = mem::replace(&mut self.server, listening);
        frame::serve(&mut launch)
            .await
            .map_err(|source| InstanceError::Rejected { source })
    }

    async fn send(data_root: &Path, hand_off: &HandOff) -> Result<(), InstanceError> {
        let name = pipe_name(data_root);
        let mut client = connect(&name).await?;
        frame::deliver(&mut client, hand_off)
            .await
            .map_err(|source| InstanceError::Rejected { source })
    }
}
