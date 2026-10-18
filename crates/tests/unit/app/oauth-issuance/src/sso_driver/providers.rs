use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_oauth_issuance::sso::{RoleResolver, SsoHookContext};
use systemprompt_oauth_issuance::{
    register_auth_event_sink, register_role_resolver, register_sso_provider,
};
use systemprompt_traits::sso::{
    AuthEventSink, CookieSameSite, ExternalIdentity, SsoError, SsoIdentityPolicy, SsoLoginRefused,
    SsoLoginSuccess, SsoMetadataDoc, SsoPaths, SsoProtocol, SsoProvider, SsoRedirect, SsoReturn,
    SsoStart, SsoStateContext, SsoStateSeed,
};

pub const OPEN: &str = "test-sso-open";
pub const CLOSED: &str = "test-sso-closed";
pub const BLANK: &str = "test-sso-blank";
pub const OPEN_CALLBACK: &str = "/api/v1/core/sso/test-sso-open/callback";
pub const CLOSED_CALLBACK: &str = "/api/v1/core/sso/test-sso-closed/callback";
pub const DEFAULT_RETURN: &str = "/home";
pub const RESOLVED_ROLE: &str = "sso-resolved";
pub const METADATA_BODY: &str = "<EntityDescriptor/>";

#[derive(Debug)]
struct FakeProvider {
    id: &'static str,
    issuer: &'static str,
    callback: &'static str,
    policy: SsoIdentityPolicy,
    unsolicited: bool,
    blank_state: bool,
}

fn open_policy() -> SsoIdentityPolicy {
    SsoIdentityPolicy {
        allowed_email_domains: vec!["example.test".to_owned()],
        auto_provision: true,
        link_by_email: true,
        group_roles: BTreeMap::from([("admins".to_owned(), vec!["admin".to_owned()])]),
        ..SsoIdentityPolicy::default()
    }
}

fn provider(id: &'static str) -> FakeProvider {
    match id {
        OPEN => FakeProvider {
            id,
            issuer: "https://idp.test/open",
            callback: OPEN_CALLBACK,
            policy: open_policy(),
            unsolicited: false,
            blank_state: false,
        },
        CLOSED => FakeProvider {
            id,
            issuer: "https://idp.test/closed",
            callback: CLOSED_CALLBACK,
            policy: SsoIdentityPolicy::default(),
            unsolicited: true,
            blank_state: false,
        },
        _ => FakeProvider {
            id,
            issuer: "https://idp.test/blank",
            callback: "/api/v1/core/sso/test-sso-blank/callback",
            policy: SsoIdentityPolicy::default(),
            unsolicited: false,
            blank_state: true,
        },
    }
}

pub fn nonce_for(state: &str) -> String {
    format!("nonce-{state}")
}

#[async_trait]
impl SsoProvider for FakeProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn issuer(&self) -> &str {
        self.issuer
    }

    fn protocol(&self) -> SsoProtocol {
        SsoProtocol::Oidc
    }

    fn policy(&self) -> &SsoIdentityPolicy {
        &self.policy
    }

    fn paths(&self) -> SsoPaths {
        SsoPaths {
            start: "/start",
            callback: self.callback,
            metadata: None,
        }
    }

    fn login_path(&self) -> &str {
        "/login"
    }

    fn default_return_to(&self) -> &str {
        DEFAULT_RETURN
    }

    fn allow_unsolicited(&self) -> bool {
        self.unsolicited
    }

    fn return_cookie_same_site(&self) -> CookieSameSite {
        CookieSameSite::None
    }

    async fn start(&self, input: SsoStart<'_>) -> Result<SsoRedirect, SsoError> {
        let state = if self.blank_state {
            String::new()
        } else {
            format!("st-{}", uuid::Uuid::new_v4())
        };
        Ok(SsoRedirect {
            url: format!(
                "https://idp.test/authorize?state={state}&redirect_uri={}",
                input.callback_url
            ),
            seed: SsoStateSeed {
                nonce: Some(nonce_for(&state)),
                state,
                request_id: None,
                pkce_verifier: None,
            },
        })
    }

    async fn finish(
        &self,
        ret: SsoReturn,
        state: Option<SsoStateContext>,
    ) -> Result<ExternalIdentity, SsoError> {
        if ret.param("fail").is_some() {
            return Err(SsoError::InvalidAssertion("idp said no".to_owned()));
        }
        if let Some(ctx) = state.as_ref()
            && let Some(echoed) = ret.param("state")
            && ctx.nonce.as_deref() != Some(nonce_for(echoed).as_str())
        {
            return Err(SsoError::InvalidAssertion("nonce mismatch".to_owned()));
        }
        let groups = ret
            .param("groups")
            .map(|g| g.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        Ok(ExternalIdentity {
            subject: ret.param("sub").unwrap_or_default().to_owned(),
            email: ret.param("email").map(str::to_owned),
            email_verified: ret.param("verified") == Some("1"),
            display_name: ret.param("name").map(str::to_owned),
            groups,
            groups_overage: ret.param("overage").is_some(),
            tenant_id: None,
            assertion_id: ret.param("assertion").map(str::to_owned),
            assertion_expires_at: None,
            raw_claims: serde_json::Value::Null,
        })
    }

    async fn metadata(&self) -> Option<Result<SsoMetadataDoc, SsoError>> {
        (self.id == OPEN).then(|| {
            Ok(SsoMetadataDoc {
                content_type: "application/samlmetadata+xml",
                body: METADATA_BODY.to_owned(),
            })
        })
    }
}

fn open_factory(_ctx: &SsoHookContext) -> Option<Arc<dyn SsoProvider>> {
    Some(Arc::new(provider(OPEN)))
}

fn duplicate_open_factory(_ctx: &SsoHookContext) -> Option<Arc<dyn SsoProvider>> {
    Some(Arc::new(FakeProvider {
        issuer: "https://idp.test/duplicate",
        ..provider(OPEN)
    }))
}

fn closed_factory(_ctx: &SsoHookContext) -> Option<Arc<dyn SsoProvider>> {
    Some(Arc::new(provider(CLOSED)))
}

fn blank_factory(_ctx: &SsoHookContext) -> Option<Arc<dyn SsoProvider>> {
    Some(Arc::new(provider(BLANK)))
}

fn disabled_factory(_ctx: &SsoHookContext) -> Option<Arc<dyn SsoProvider>> {
    None
}

register_sso_provider!(open_factory);
register_sso_provider!(duplicate_open_factory);
register_sso_provider!(closed_factory);
register_sso_provider!(blank_factory);
register_sso_provider!(disabled_factory);

#[derive(Debug)]
struct AppendingResolver;

#[async_trait]
impl RoleResolver for AppendingResolver {
    async fn resolve(
        &self,
        _user_id: &UserId,
        directory_roles: &[String],
    ) -> Result<Vec<String>, SsoError> {
        let mut roles = directory_roles.to_vec();
        roles.push(RESOLVED_ROLE.to_owned());
        Ok(roles)
    }
}

fn resolver_factory(_ctx: &SsoHookContext) -> Arc<dyn RoleResolver> {
    Arc::new(AppendingResolver)
}

register_role_resolver!(resolver_factory);
register_role_resolver!(resolver_factory);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    Success {
        sub: String,
        email: String,
        created: bool,
    },
    Refused {
        sub: Option<String>,
        email: Option<String>,
        reason: &'static str,
    },
}

fn recorded() -> &'static Mutex<Vec<Recorded>> {
    static EVENTS: OnceLock<Mutex<Vec<Recorded>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn events_for_sub(sub: &str) -> Vec<Recorded> {
    recorded()
        .lock()
        .expect("events lock")
        .iter()
        .filter(|event| match event {
            Recorded::Success { sub: s, .. } => s == sub,
            Recorded::Refused { sub: s, .. } => s.as_deref() == Some(sub),
        })
        .cloned()
        .collect()
}

pub fn refusals_with_reason(reason: &str) -> usize {
    recorded()
        .lock()
        .expect("events lock")
        .iter()
        .filter(|event| matches!(event, Recorded::Refused { reason: r, .. } if *r == reason))
        .count()
}

#[derive(Debug)]
struct RecordingSink;

#[async_trait]
impl AuthEventSink for RecordingSink {
    async fn on_login_success(&self, event: &SsoLoginSuccess<'_>) {
        recorded()
            .lock()
            .expect("events lock")
            .push(Recorded::Success {
                sub: event.identity.external_sub.clone(),
                email: event.identity.email.clone(),
                created: event.created,
            });
    }

    async fn on_login_refused(&self, event: &SsoLoginRefused<'_>) {
        recorded()
            .lock()
            .expect("events lock")
            .push(Recorded::Refused {
                sub: event.external_sub.map(str::to_owned),
                email: event.email.map(str::to_owned),
                reason: event.reason,
            });
    }
}

fn sink_factory(_ctx: &SsoHookContext) -> Arc<dyn AuthEventSink> {
    Arc::new(RecordingSink)
}

register_auth_event_sink!(sink_factory);
