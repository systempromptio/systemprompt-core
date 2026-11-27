//! [`BrainClient`]: one account's brain over the instance's `/api/v1/brain`
//! routes.
//!
//! Every call takes a fresh enough JWT from the account's
//! [`CredentialSource`](systemprompt_device::client::CredentialSource)
//! through [`with_jwt`](systemprompt_device::client::with_jwt), which mints once more and retries
//! when the instance refuses the token. Ids go into the path as single
//! segments; queries are built from typed values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod query;
mod reads;
mod writes;

use std::fmt;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use systemprompt_device::client::{CredentialSource, HttpClient, SpClient, with_jwt};
use url::Url;

pub use query::FactQuery;

use crate::members::brain::error::{BrainError, classify, decode_failure};
use crate::members::brain::http::{Answer, Call, route, send_once};

/// One account's brain. Cheap to clone; the credential stays with the
/// [`CredentialSource`].
#[derive(Clone)]
pub struct BrainClient {
    inner: Arc<Inner>,
}

struct Inner {
    base: String,
    http: HttpClient,
    credentials: Arc<dyn CredentialSource>,
}

impl fmt::Debug for BrainClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainClient")
            .field("base", &self.inner.base)
            .finish_non_exhaustive()
    }
}

impl BrainClient {
    #[must_use]
    pub fn new(
        instance: &SpClient,
        http: HttpClient,
        credentials: Arc<dyn CredentialSource>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                base: instance.base().to_owned(),
                http,
                credentials,
            }),
        }
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.inner.base
    }

    fn url(&self, path: &str, segments: &[&str]) -> Result<Url, BrainError> {
        route(&self.inner.base, path, segments).map_err(BrainError::Client)
    }

    async fn answer(&self, call: Call, expect: u16) -> Result<Answer, BrainError> {
        let http = self.inner.http.reqwest();
        let call = &call;
        let answer = with_jwt(&*self.inner.credentials, |jwt| send_once(http, call, jwt)).await?;
        if answer.status != expect {
            return Err(classify(
                call.endpoint,
                answer.status,
                &answer.headers,
                &answer.body,
            ));
        }
        Ok(answer)
    }

    async fn json<T: DeserializeOwned>(&self, call: Call, expect: u16) -> Result<T, BrainError> {
        let endpoint = call.endpoint;
        let answer = self.answer(call, expect).await?;
        serde_json::from_slice(&answer.body)
            .map_err(|error| BrainError::Client(decode_failure(endpoint, answer.status, &error)))
    }
}
