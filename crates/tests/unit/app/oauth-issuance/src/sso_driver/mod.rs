//! The SSO driver end to end against fake providers registered through the
//! public inventory macros: `start` stores and binds an attempt, `finish`
//! correlates it, enforces replay and the identity policy, resolves or
//! provisions the local user, mints the session and audits every outcome.

mod flow;
mod identity;
mod providers;
mod stores;

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_oauth_issuance::sso::{FinishOutcome, SsoCallback, SsoDriver};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, install_test_signing_key, test_db_pool,
};
use systemprompt_traits::SessionAnalytics;
use systemprompt_traits::sso::{SsoReturn, SsoReturnBinding};

use stores::{CountingSessions, MemoryUsers};

struct Harness {
    driver: SsoDriver,
    users: Arc<MemoryUsers>,
    sessions: Arc<CountingSessions>,
}

fn harness_on(db: &DbPool, api_external_url: &str) -> Harness {
    ensure_test_bootstrap();
    install_test_signing_key();
    let users = Arc::new(MemoryUsers::default());
    let sessions = Arc::new(CountingSessions::default());
    let service = SessionCreationService::new(
        Arc::clone(&sessions) as Arc<dyn systemprompt_traits::SessionProvider>,
        Arc::clone(&users) as Arc<dyn systemprompt_traits::UserProvider>,
    );
    let mut config = fixture_config("postgres://unused/unused");
    config.api_external_url = api_external_url.to_owned();
    Harness {
        driver: SsoDriver::discover(db, Arc::new(service), Arc::new(config)),
        users,
        sessions,
    }
}

async fn harness() -> Harness {
    harness_on(&test_db_pool().await, "https://sso.test")
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}

fn state_of(redirect_url: &str) -> String {
    redirect_url
        .split_once("state=")
        .and_then(|(_, rest)| rest.split('&').next())
        .expect("redirect carries a state")
        .to_owned()
}

fn sso_return(params: &[(&str, &str)]) -> SsoReturn {
    SsoReturn {
        binding: SsoReturnBinding::Query,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

async fn finish(
    driver: &SsoDriver,
    id: &str,
    params: &[(&str, &str)],
    binder: Option<&str>,
) -> FinishOutcome {
    let analytics = SessionAnalytics::default();
    driver
        .finish(
            id,
            SsoCallback {
                ret: sso_return(params),
                binder_cookie: binder,
                analytics: &analytics,
                ip: Some("203.0.113.9".parse().expect("ip literal")),
            },
        )
        .await
}

struct Started {
    state: String,
    binder: String,
}

async fn start(driver: &SsoDriver, id: &str, return_to: Option<&str>) -> Started {
    let outcome = driver
        .start(id, return_to)
        .await
        .expect("start stores the attempt");
    Started {
        state: state_of(&outcome.redirect_url),
        binder: outcome.binder_cookie.value,
    }
}

fn refusal_reason(outcome: &FinishOutcome) -> &'static str {
    match outcome {
        FinishOutcome::Refused { reason, .. } => *reason,
        FinishOutcome::Success { .. } => panic!("expected a refusal, got {outcome:?}"),
    }
}

fn success_return_to(outcome: &FinishOutcome) -> &str {
    match outcome {
        FinishOutcome::Success { return_to, .. } => return_to,
        FinishOutcome::Refused { reason, .. } => panic!("expected success, refused: {reason}"),
    }
}
