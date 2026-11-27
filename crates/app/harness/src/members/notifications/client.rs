//! [`NotificationsClient`]: one account's notification feed over the bridge
//! routes.
//!
//! Every call takes a fresh enough JWT from the account's [`CredentialSource`]
//! through [`with_jwt`], which mints once more and retries when the instance
//! refuses the token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use reqwest::Method;
use serde::Serialize;
use serde::de::DeserializeOwned;
use systemprompt_device::client::{
    ClientError, CredentialSource, HttpClient, SpClient, TransportFailure, with_jwt,
};
use systemprompt_device::protocol::origin::resolve_instance_path;
use systemprompt_models::ApiPaths;
use systemprompt_models::api::KeysetCursor;
use systemprompt_models::notifications::feed::{
    FEED_PAGE_MAX, FEED_QUERY_LIMIT, FEED_QUERY_SINCE, FEED_QUERY_UNREAD, SeenAnswer,
};
use systemprompt_models::notifications::{
    MarkRead, MarkSeen, NotificationPage, NotificationReadPayload,
};

use crate::members::notifications::error::NotificationsError;
use crate::members::notifications::http::{Answer, Call, send_once};

const FEED_BODY: usize = 4 * 1024 * 1024;

const ACK_BODY: usize = 256 * 1024;

const FEED_ENDPOINT: &str = "GET /v1/bridge/notifications";

const READ_ENDPOINT: &str = "POST /v1/bridge/notifications/read";

const SEEN_ENDPOINT: &str = "POST /v1/bridge/notifications/seen";

/// One page request of the feed: newest first, continuing below `since` when
/// given, at most `limit` items (1..=100, clamped), optionally unread only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedQuery {
    since: Option<KeysetCursor>,
    limit: u32,
    unread_only: bool,
}

impl FeedQuery {
    pub const fn newest(limit: u32) -> Self {
        let limit = if limit == 0 {
            1
        } else if limit > FEED_PAGE_MAX {
            FEED_PAGE_MAX
        } else {
            limit
        };
        Self {
            since: None,
            limit,
            unread_only: false,
        }
    }

    pub const fn since(mut self, cursor: KeysetCursor) -> Self {
        self.since = Some(cursor);
        self
    }

    pub const fn unread_only(mut self) -> Self {
        self.unread_only = true;
        self
    }

    pub const fn limit(&self) -> u32 {
        self.limit
    }
}

/// One account's notifications. Cheap to clone; the credential stays with the
/// [`CredentialSource`].
#[derive(Clone)]
pub struct NotificationsClient {
    inner: Arc<Inner>,
}

struct Inner {
    base: String,
    http: HttpClient,
    credentials: Arc<dyn CredentialSource>,
}

impl fmt::Debug for NotificationsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationsClient")
            .field("base", &self.inner.base)
            .finish_non_exhaustive()
    }
}

impl NotificationsClient {
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

    pub fn base(&self) -> &str {
        &self.inner.base
    }

    pub async fn feed(&self, query: &FeedQuery) -> Result<NotificationPage, NotificationsError> {
        let mut url = self.url(ApiPaths::BRIDGE_NOTIFICATIONS)?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(since) = &query.since {
                pairs.append_pair(FEED_QUERY_SINCE, &since.to_string());
            }
            pairs.append_pair(FEED_QUERY_LIMIT, &query.limit.to_string());
            if query.unread_only {
                pairs.append_pair(FEED_QUERY_UNREAD, "true");
            }
        }
        let call = Call {
            method: Method::GET,
            url,
            endpoint: FEED_ENDPOINT,
            body: None,
            limit: FEED_BODY,
        };
        self.exchange(&call).await
    }

    pub async fn mark_read(
        &self,
        request: &MarkRead,
    ) -> Result<NotificationReadPayload, NotificationsError> {
        let call = self.post(ApiPaths::BRIDGE_NOTIFICATIONS_READ, READ_ENDPOINT, request)?;
        self.exchange(&call).await
    }

    pub async fn mark_seen(&self, request: &MarkSeen) -> Result<SeenAnswer, NotificationsError> {
        let call = self.post(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN, SEEN_ENDPOINT, request)?;
        self.exchange(&call).await
    }

    fn url(&self, path: &str) -> Result<url::Url, NotificationsError> {
        resolve_instance_path(&self.inner.base, path)
            .map_err(|error| NotificationsError::Client(ClientError::Origin(error)))
    }

    fn post<T: Serialize>(
        &self,
        path: &str,
        endpoint: &'static str,
        body: &T,
    ) -> Result<Call, NotificationsError> {
        let body = serde_json::to_vec(body).map_err(NotificationsError::Encode)?;
        Ok(Call {
            method: Method::POST,
            url: self.url(path)?,
            endpoint,
            body: Some(body),
            limit: ACK_BODY,
        })
    }

    async fn exchange<T: DeserializeOwned>(&self, call: &Call) -> Result<T, NotificationsError> {
        let http = self.inner.http.reqwest();
        let answer = with_jwt(&*self.inner.credentials, |jwt| send_once(http, call, jwt)).await?;
        let answer = classify(call.endpoint, answer)?;
        serde_json::from_slice(&answer.body).map_err(|source| NotificationsError::Decode {
            endpoint: call.endpoint,
            source,
        })
    }
}

fn classify(endpoint: &'static str, answer: Answer) -> Result<Answer, NotificationsError> {
    match answer.status {
        200..=299 => Ok(answer),
        404 => Err(NotificationsError::NotServed { endpoint }),
        429 => Err(NotificationsError::Client(ClientError::RateLimited {
            endpoint,
            retry_after: answer.retry_after,
        })),
        status @ 500.. => Err(NotificationsError::Client(ClientError::Unreachable {
            endpoint,
            cause: TransportFailure::Status { status },
        })),
        status => Err(NotificationsError::Rejected { endpoint, status }),
    }
}
