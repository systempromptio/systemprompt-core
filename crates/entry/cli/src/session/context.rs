use systemprompt_cloud::CliSession;
use systemprompt_identifiers::{AgentName, ContextId, SessionToken, TraceId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::Profile;

#[derive(Debug)]
pub struct CliSessionContext {
    pub session: CliSession,
    pub profile: Profile,
}

impl CliSessionContext {
    pub const fn session_token(&self) -> &SessionToken {
        &self.session.session_token
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.session.context_id
    }

    pub fn api_url(&self) -> &str {
        &self.profile.server.api_external_url
    }

    pub fn to_request_context(&self, agent_name: &str) -> RequestContext {
        RequestContext::new(
            self.session.session_id.clone(),
            TraceId::generate(),
            self.session.context_id.clone(),
            AgentName::new(agent_name.to_string()),
        )
        .with_user_id(self.session.user_id.clone())
        .with_auth_token(self.session.session_token.as_str())
        .with_user_type(self.session.user_type)
    }
}
