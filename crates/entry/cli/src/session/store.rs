use anyhow::Result;
use systemprompt_cloud::{CliSession, ProjectContext, SessionKey, SessionStore};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::resolution::resolve_session_paths;

pub fn clear_session() -> Result<()> {
    let profile = ProfileBootstrap::get().ok();
    let tenant_id = profile
        .as_ref()
        .and_then(|p| p.cloud.as_ref())
        .and_then(|c| c.tenant_id.as_deref());
    let session_key = SessionKey::from_tenant_id(tenant_id);

    let project_ctx = ProjectContext::discover();
    let sessions_dir = resolve_session_paths(&project_ctx)?;

    let mut store = SessionStore::load_or_create(&sessions_dir)?;
    store.remove_session(&session_key);
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn clear_all_sessions() -> Result<()> {
    let project_ctx = ProjectContext::discover();
    let sessions_dir = resolve_session_paths(&project_ctx)?;

    let store = SessionStore::new();
    store.save(&sessions_dir)?;

    Ok(())
}

pub fn get_session_for_key(session_key: &SessionKey) -> Result<Option<CliSession>> {
    let project_ctx = ProjectContext::discover();
    let sessions_dir = resolve_session_paths(&project_ctx)?;

    let store = SessionStore::load_or_create(&sessions_dir)?;
    Ok(store.get_valid_session(session_key).cloned())
}

pub fn load_session_store() -> Result<SessionStore> {
    let project_ctx = ProjectContext::discover();
    let sessions_dir = resolve_session_paths(&project_ctx)?;
    SessionStore::load_or_create(&sessions_dir)
}
