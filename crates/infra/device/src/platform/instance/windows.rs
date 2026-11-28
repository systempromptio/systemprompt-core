//! Windows: a named pipe whose name is derived from the data root.
//!
//! The pipe is created as the first and only instance of its name, refuses
//! remote clients, and carries a protected DACL that grants access to its
//! owner alone. A launch connects at identification level, so a process that
//! took the name first learns who called and can do nothing as them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "the pipe's security descriptor is raw Win32 FFI"
)]

use std::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

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
// its pipe, or while the pipe's one instance is serving another launch.
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

impl Transport for Endpoint {
    fn bind(data_root: &Path) -> Result<Self, InstanceError> {
        let name = pipe_name(data_root);
        let descriptor = owner_only()
            .map_err(|source| InstanceError::io("describe the access of", &name, source))?;
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
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .max_instances(1)
                .create_with_security_attributes_raw(&name, (&raw mut attributes).cast())
        }
        .map_err(|source| InstanceError::io("create", &name, source))?;
        drop(descriptor);
        Ok(Self { server, name })
    }

    async fn receive(&mut self) -> Result<HandOff, InstanceError> {
        self.server
            .connect()
            .await
            .map_err(|source| InstanceError::io("await a launch on", &self.name, source))?;
        let served = frame::serve(&mut self.server).await;
        self.server
            .disconnect()
            .map_err(|source| InstanceError::io("disconnect", &self.name, source))?;
        served.map_err(|source| InstanceError::Rejected { source })
    }

    async fn send(data_root: &Path, hand_off: &HandOff) -> Result<(), InstanceError> {
        let name = pipe_name(data_root);
        let mut client = connect(&name).await?;
        frame::deliver(&mut client, hand_off)
            .await
            .map_err(|source| InstanceError::Rejected { source })
    }
}
