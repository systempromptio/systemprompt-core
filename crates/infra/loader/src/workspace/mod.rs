//! The on-disk workspace a local CLI reads before anything boots.
//!
//! - [`paths`] — project discovery and the typed project, profile and
//!   `.systemprompt` file paths, including where the credentials, tenants and
//!   session files live.
//! - [`cli_session`] — the persisted CLI sessions, keyed per local or per
//!   tenant, and the [`SessionStore`] index file.
//! - [`profile`] — profile resolution from a CLI override, an environment
//!   variable, the stored session or directory discovery, with the
//!   [`ProfileSource`](profile::ProfileSource) it came from.
//! - [`provider_catalog`] — the gateway catalog a new install is seeded with.
//! - [`identity`] — the replica identity secrets a new profile is given.
//! - [`docker_compose`] — the local PostgreSQL compose file and container.
//! - [`private_dir`] — owner-only files for documents that hold tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cli_session;
pub mod constants;
pub mod docker_compose;
pub mod error;
pub mod identity;
pub mod paths;
pub mod private_dir;
pub mod profile;
pub mod provider_catalog;

pub use cli_session::{
    CliSession, CliSessionBuilder, LOCAL_SESSION_KEY, SessionBinding, SessionIdentity, SessionKey,
    SessionStore,
};
pub use error::{WorkspaceError, WorkspaceResult};
pub use paths::{
    CloudPath, CloudPaths, DiscoveredProject, ProfilePath, ProjectContext, ProjectPath,
    ResolvedPaths, UnifiedContext, expand_home, get_cloud_paths, resolve_path,
};
