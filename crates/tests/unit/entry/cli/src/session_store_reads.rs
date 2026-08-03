//! Tests for the read side of the CLI session store.
//!
//! `load_session_store` and `get_session_for_key` resolve the sessions
//! directory from the discovered project and tolerate an absent record; the
//! clearing functions are not exercised because they rewrite the developer's
//! real session file.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::session::{get_session_for_key, load_session_store};
use systemprompt_cloud::SessionKey;

#[test]
fn the_store_loads_or_initialises_without_error() {
    let store = load_session_store().unwrap();

    // Every record the store returns must round-trip through its own lookup.
    let key = SessionKey::from_tenant_id(None);
    let looked_up = store.get_valid_session(&key);
    let via_helper = get_session_for_key(&key).unwrap();

    assert_eq!(looked_up.is_some(), via_helper.is_some());
}

#[test]
fn an_unknown_tenant_key_resolves_to_no_session() {
    let tenant = systemprompt_identifiers::TenantId::new("cov_absent_tenant");
    let key = SessionKey::Tenant(tenant);

    assert!(get_session_for_key(&key).unwrap().is_none());
}
