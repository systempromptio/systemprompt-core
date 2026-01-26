use anyhow::Result;
use systemprompt_cloud::{CliSession, SessionKey, SessionStore};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use crate::paths::ResolvedPaths;

pub fn clear_session() -> Result<()> {
    let profile = ProfileBootstrap::get().ok();
    let tenant_id = profile
        .as_ref()
        .and_then(|p| p.cloud.as_ref())
        .and_then(|c| c.tenant_id.as_deref());
    let session_key = SessionKey::from_tenant_id(tenant_id);

    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;

    let mut store = SessionStore::load_or_create(&sessions_dir)?;
    store.remove_session(&session_key);
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn clear_all_sessions() -> Result<()> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;

    let store = SessionStore::new();
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn get_session_for_key(session_key: &SessionKey) -> Result<Option<CliSession>> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;

    let store = SessionStore::load_or_create(&sessions_dir)?;
    Ok(store.get_valid_session(session_key).cloned())
}

pub fn load_session_store() -> Result<SessionStore> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;
    SessionStore::load_or_create(&sessions_dir)
}
