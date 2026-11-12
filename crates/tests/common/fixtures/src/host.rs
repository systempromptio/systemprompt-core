//! The environment a spawned MCP server boots a `HostContext` from, built for
//! a fixture bootstrap: the profile path, the database URL and the fixture
//! deployment's public verification keys, and nothing else.

use std::collections::BTreeMap;

use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;

use crate::bootstrap::TestBootstrap;

pub fn host_child_env(bootstrap: &TestBootstrap) -> BTreeMap<String, String> {
    let keys = crate::jwt::test_signing_keys();
    BTreeMap::from([
        (
            "SYSTEMPROMPT_PROFILE".to_owned(),
            bootstrap.profile_path.display().to_string(),
        ),
        ("DATABASE_URL".to_owned(), bootstrap.database_url.clone()),
        (
            VERIFICATION_KEYS_ENV.to_owned(),
            serde_json::to_string(keys.verification_keys()).expect("verification keys serialise"),
        ),
    ])
}
