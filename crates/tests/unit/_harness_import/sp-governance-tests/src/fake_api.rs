//! A fake token source and a fake instance behind the hook routes.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::time::Duration;

use chrono::{TimeDelta, Utc};
use serde_json::Value;
use sp_client::{ClientError, CredentialError, CredentialSource, Freshness, HookClient, HookGrant};
use sp_governance::HookApi;
use sp_protocol::ids::{ClientId, PluginId};
use sp_protocol::wire::hooks::{GovernAnswer, HookEvent};
use sp_protocol::{ClientSecret, DeviceCredential, HookToken, Jwt};
use url::Url;

use crate::support::{EXPIRED, HEADER, VALID};

#[derive(Default)]
pub(crate) struct FakeCredentials {
    asked: Mutex<Vec<Freshness>>,
    minted: AtomicUsize,
    pub(crate) rejected: AtomicUsize,
    pub(crate) signed_out: AtomicBool,
    pub(crate) expired: AtomicBool,
}

#[async_trait::async_trait]
impl CredentialSource for FakeCredentials {
    async fn jwt(&self, freshness: Freshness) -> Result<Jwt, CredentialError> {
        self.asked.lock().unwrap().push(freshness);
        if self.signed_out.load(Ordering::SeqCst) {
            return Err(CredentialError::SignedOut);
        }
        let n = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
        let payload = if self.expired.load(Ordering::SeqCst) {
            EXPIRED
        } else {
            VALID
        };
        Ok(Jwt::new(format!("{HEADER}.{payload}.sig{n}")).unwrap())
    }

    async fn rejected(&self) {
        self.rejected.fetch_add(1, Ordering::SeqCst);
    }
}

impl FakeCredentials {
    pub(crate) fn last(&self) -> Option<Freshness> {
        self.asked.lock().unwrap().last().copied()
    }
}

pub(crate) enum Answer {
    Verdict(GovernAnswer),
    Slow,
    Broken,
}

#[derive(Debug, Clone)]
pub(crate) struct Tracked {
    pub(crate) name: &'static str,
    pub(crate) device: bool,
    pub(crate) body: Value,
}

pub(crate) struct FakeApi {
    pub(crate) provisions: AtomicUsize,
    pub(crate) mints: AtomicUsize,
    pub(crate) refuse_mints: AtomicUsize,
    lifetime_secs: AtomicI64,
    tracked: Mutex<Vec<Tracked>>,
    pub(crate) asked: AtomicUsize,
    verdict_tokens: Mutex<Vec<String>>,
    answer: Mutex<Answer>,
}

impl FakeApi {
    pub(crate) fn new(lifetime_secs: i64) -> Self {
        Self {
            provisions: AtomicUsize::new(0),
            mints: AtomicUsize::new(0),
            refuse_mints: AtomicUsize::new(0),
            lifetime_secs: AtomicI64::new(lifetime_secs),
            tracked: Mutex::new(Vec::new()),
            asked: AtomicUsize::new(0),
            verdict_tokens: Mutex::new(Vec::new()),
            answer: Mutex::new(Answer::Verdict(GovernAnswer::default())),
        }
    }

    pub(crate) fn answer(&self, answer: Answer) {
        *self.answer.lock().unwrap() = answer;
    }

    pub(crate) fn verdict_tokens(&self) -> Vec<String> {
        self.verdict_tokens.lock().unwrap().clone()
    }

    pub(crate) fn tracked_now(&self) -> Vec<Tracked> {
        self.tracked.lock().unwrap().clone()
    }

    pub(crate) async fn tracked(&self, count: usize) -> Vec<Tracked> {
        for _ in 0..200 {
            let seen = self.tracked_now();
            if seen.len() >= count {
                return seen;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        self.tracked_now()
    }
}

fn refused() -> ClientError {
    ClientError::Unauthorized {
        endpoint: "POST {token endpoint}",
        error_key: None,
    }
}

#[async_trait::async_trait]
impl HookApi for FakeApi {
    async fn provision_client(&self, jwt: &Jwt) -> Result<HookClient, ClientError> {
        assert!(jwt.expose().starts_with(HEADER));
        self.provisions.fetch_add(1, Ordering::SeqCst);
        Ok(HookClient {
            client_id: ClientId::new("client-1"),
            client_secret: ClientSecret::new("client-secret").unwrap(),
            token_endpoint: Url::parse("https://sp.example.com/api/v1/core/oauth/token").unwrap(),
            scopes: vec!["hook:track".to_owned(), "hook:govern".to_owned()],
        })
    }

    async fn mint_token(
        &self,
        _client: &HookClient,
        _plugin: &PluginId,
    ) -> Result<HookGrant, ClientError> {
        let n = self.mints.fetch_add(1, Ordering::SeqCst) + 1;
        let pending = self.refuse_mints.load(Ordering::SeqCst);
        if pending > 0 {
            self.refuse_mints.store(pending - 1, Ordering::SeqCst);
            return Err(refused());
        }
        let lifetime = self.lifetime_secs.load(Ordering::SeqCst);
        Ok(HookGrant {
            token: HookToken::new(format!("hook-{n}")).unwrap(),
            expires_in: Duration::from_secs(u64::try_from(lifetime).unwrap()),
            expires_at: Utc::now() + TimeDelta::seconds(lifetime),
        })
    }

    async fn send_event(
        &self,
        token: &HookToken,
        _plugin: &PluginId,
        event: &HookEvent,
        device: Option<&DeviceCredential>,
    ) -> Result<(), ClientError> {
        assert!(token.expose().starts_with("hook-"));
        let body = serde_json::to_value(event).unwrap();
        self.tracked.lock().unwrap().push(Tracked {
            name: event.name(),
            device: device.is_some(),
            body,
        });
        Ok(())
    }

    async fn ask_verdict(
        &self,
        token: &HookToken,
        _plugin: &PluginId,
        event: &HookEvent,
    ) -> Result<GovernAnswer, ClientError> {
        assert_eq!(event.name(), "PreToolUse");
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.verdict_tokens
            .lock()
            .unwrap()
            .push(token.expose().to_owned());
        let ready = {
            let answer = self.answer.lock().unwrap();
            match &*answer {
                Answer::Verdict(verdict) => Some(Ok(verdict.clone())),
                Answer::Broken => Some(Err(refused())),
                Answer::Slow => None,
            }
        };
        if let Some(ready) = ready {
            return ready;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(GovernAnswer::default())
    }
}
