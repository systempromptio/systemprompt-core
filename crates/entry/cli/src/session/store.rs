//! On-disk store for CLI session records, keyed by profile.
//!
//! Every function takes the [`ResolvedPaths`] it reads and writes under, so
//! the caller decides which project's `.systemprompt` directory is touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_cloud::{CliSession, SessionKey, SessionStore};
use systemprompt_config::ProfileBootstrap;

use crate::paths::ResolvedPaths;

pub fn clear_session(paths: &ResolvedPaths) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref());
    let session_key = SessionKey::from_tenant_id(tenant_id);

    let sessions_dir = paths.sessions_dir();

    let mut store = SessionStore::load_or_create(&sessions_dir)?;
    store.remove_session(&session_key);
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn clear_all_sessions(paths: &ResolvedPaths) -> Result<()> {
    let sessions_dir = paths.sessions_dir();

    let store = SessionStore::new();
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn get_session_for_key(
    paths: &ResolvedPaths,
    session_key: &SessionKey,
    issuer: &str,
) -> Result<Option<CliSession>> {
    let sessions_dir = paths.sessions_dir();

    let store = SessionStore::load_or_create(&sessions_dir)?;
    Ok(store.get_valid_session(session_key, issuer).cloned())
}

pub fn load_session_store(paths: &ResolvedPaths) -> Result<SessionStore> {
    let sessions_dir = paths.sessions_dir();
    SessionStore::load_or_create(&sessions_dir).map_err(Into::into)
}
