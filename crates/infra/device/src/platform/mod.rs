//! One platform contract with a Windows, a macOS and a Linux implementation.
//!
//! The contract is every operation of this crate that differs between
//! systems: sealing a secret for the current user, the OS credential store,
//! owner-only files, and the base directories under which the single data
//! root lives. `Native` is the contract's implementation for the target being
//! built, so a target without one does not compile, and no caller branches on
//! the system: this module is the only place in the crate that names one.
//!
//! | Concern | Windows | macOS | Linux |
//! |---|---|---|---|
//! | Sealing | DPAPI, current user | AES-256-GCM under a login keychain key | AES-256-GCM under a Secret Service key, else a kernel keyring key, else a key in this process's memory |
//! | Credential store | Credential Manager | login keychain | Secret Service, else the kernel keyring |
//! | Owner-only files | protected DACL | `0600`, `O_NOFOLLOW`, owned by the user | the same |
//! | Per-user base | `%LOCALAPPDATA%` | `~/Library/Application Support` | `$XDG_DATA_HOME`, else `~/.local/share` |
//! | Machine base | `%ProgramData%` | `/Library/Application Support` | `/var/lib` |
//!
//! `unsafe` is confined to the Windows FFI modules; the macOS and Linux
//! modules use safe bindings only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![deny(unsafe_code)]

mod contract;
mod dirs;
#[cfg(any(windows, target_os = "linux"))]
mod keyring;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub(crate) use contract::{CredentialStore, Platform};
pub use dirs::{
    DataRoot, DirsError, Environment, RootName, home_dir, local_data_dir, machine_data_dir,
};
#[cfg(target_os = "linux")]
pub use linux::KeyringSealer;
#[cfg(target_os = "linux")]
pub(crate) use linux::Linux as Native;
#[cfg(target_os = "macos")]
pub use macos::KeychainSealer;
#[cfg(target_os = "macos")]
pub(crate) use macos::MacOs as Native;
#[cfg(windows)]
pub use windows::DpapiSealer;
#[cfg(windows)]
pub(crate) use windows::Windows as Native;
