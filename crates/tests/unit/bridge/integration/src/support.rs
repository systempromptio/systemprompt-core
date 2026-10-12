use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_bridge::ids::LoopbackSecret;
use systemprompt_bridge::integration::host_app::{
    HostApp, ProbeEnv, ProfileFacts, ProfileFactsError, ProfileGenInputs,
};

pub(crate) const SECRET: &str = "loopback-secret-value";

pub(crate) fn facts() -> ProfileFacts {
    ProfileFacts {
        gateway_base_url: format!(
            "http://127.0.0.1:{}",
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT
        ),
        secret: LoopbackSecret::new(SECRET),
        profile: None,
        mcp_servers: None,
        overrides: BTreeMap::new(),
        operator_org_uuid: None,
    }
}

pub(crate) fn env_with(facts: ProfileFacts) -> ProbeEnv {
    ProbeEnv {
        proxy_port: systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        start_menu: Arc::default(),
        facts: Ok(facts),
        policy_writer_ready: false,
    }
}

pub(crate) fn probe_env() -> ProbeEnv {
    env_with(facts())
}

pub(crate) fn unreadable_env(error: ProfileFactsError) -> ProbeEnv {
    ProbeEnv {
        proxy_port: systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
        start_menu: Arc::default(),
        facts: Err(Arc::new(error)),
        policy_writer_ready: false,
    }
}

pub(crate) fn inputs_for(host: &dyn HostApp) -> ProfileGenInputs {
    facts().inputs_for(host.id(), host.accepted_surfaces())
}

pub(crate) fn host_token(host: systemprompt_models::bridge::host::HostKind) -> String {
    systemprompt_bridge::proxy::scoped_token::host_token(&LoopbackSecret::new(SECRET), host)
        .as_str()
        .to_owned()
}
