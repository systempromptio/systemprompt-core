//! The inbox task: folds stream signals and GUI requests into the shared
//! inbox, runs the newest-first catch-up, loads older pages on request and
//! reports what a human has seen.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::NotificationId;
use systemprompt_models::ApiPaths;
use systemprompt_models::notifications::{
    Escalate, MarkRead, MarkSeen, Notification, NotificationCreatedPayload,
};

use super::catch_up::{CATCH_UP_PAGE, catch_up};

use super::handle::{InboxHandle, InboxRequest, InboxRequestReceiver};
use super::signal::{NotificationSignal, NotificationSignalReceiver};
use super::toast::{CATCH_UP_TOASTS, InboxToast, TOAST_FLOOR, plan_toasts};
use crate::config::SharedRuntimeConfig;
use crate::gateway::{GatewayClient, GatewayError};
use crate::ids::BearerToken;
use crate::proxy::token_cache::TokenCache;

const AUTH_THRESHOLD_SECS: u64 = 300;

pub struct InboxDeps {
    pub handle: Arc<InboxHandle>,
    pub runtime_config: SharedRuntimeConfig,
    pub token_cache: Arc<TokenCache>,
    pub http: reqwest::Client,
    pub signals: NotificationSignalReceiver,
    pub requests: InboxRequestReceiver,
}

impl std::fmt::Debug for InboxDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxDeps")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
enum InboxError {
    #[error("token: {0}")]
    Token(#[from] crate::proxy::forward::ForwardError),
    #[error(transparent)]
    Gateway(#[from] GatewayError),
}

struct Inbox {
    handle: Arc<InboxHandle>,
    runtime_config: SharedRuntimeConfig,
    token_cache: Arc<TokenCache>,
    http: reqwest::Client,
}

pub async fn run_loop(deps: InboxDeps) {
    let InboxDeps {
        handle,
        runtime_config,
        token_cache,
        http,
        mut signals,
        mut requests,
    } = deps;
    let inbox = Inbox {
        handle,
        runtime_config,
        token_cache,
        http,
    };
    loop {
        tokio::select! {
            Some(signal) = signals.recv() => inbox.on_signal(signal).await,
            Some(request) = requests.recv() => inbox.on_request(request).await,
            else => return,
        }
    }
}

impl Inbox {
    async fn on_signal(&self, signal: NotificationSignal) {
        match signal {
            NotificationSignal::Created(payload) => {
                let NotificationCreatedPayload {
                    notification,
                    unread,
                } = *payload;
                let toasts = self.handle.update(|state| {
                    state.unread = unread;
                    let through = state.toasted_through.unwrap_or(DateTime::<Utc>::MIN_UTC);
                    let (toasts, watermark) =
                        plan_toasts([&notification], Some(through), CATCH_UP_TOASTS);
                    state.toasted_through = watermark;
                    state.upsert(notification);
                    toasts
                });
                self.handle.push_toasts(toasts);
            },
            NotificationSignal::Read(payload) => {
                self.handle
                    .update(|state| state.apply_read(&payload.ids, payload.unread, Utc::now()));
            },
            NotificationSignal::Escalated(payload) => {
                let toast = self.handle.update(|state| {
                    let raised = state.apply_escalated(&payload.id, payload.attention);
                    state
                        .find(&payload.id)
                        .filter(|item| raised && item.attention >= TOAST_FLOOR)
                        .map(InboxToast::for_item)
                });
                self.handle.push_toasts(toast.into_iter().collect());
            },
            NotificationSignal::StreamOpened => {
                self.handle.set_available(true);
                self.run(|| self.catch_up()).await;
            },
        }
    }

    async fn on_request(&self, request: InboxRequest) {
        match request {
            InboxRequest::Refresh => self.run(|| self.catch_up()).await,
            InboxRequest::LoadOlder => self.run(|| self.load_older()).await,
            InboxRequest::MarkRead(body) => self.run(|| self.mark_read(&body)).await,
            InboxRequest::MarkSeen(body) => self.run(|| self.mark_seen(&body)).await,
            InboxRequest::Escalate { id, note } => {
                self.run(|| self.escalate(&id, Escalate { note })).await;
            },
        }
    }

    async fn run<F, Fut>(&self, op: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), InboxError>>,
    {
        if self.token_cache.sign_in_required() || !self.handle.available() {
            return;
        }
        match op().await {
            Ok(()) => self.handle.record_error(None),
            Err(InboxError::Gateway(GatewayError::Rejected { status, .. }))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                tracing::debug!("gateway serves no notifications; inbox unavailable");
                self.handle.record_error(None);
                self.handle.set_available(false);
            },
            Err(error) => {
                if let InboxError::Gateway(GatewayError::Rejected { status, .. }) = &error
                    && status.as_u16() == 401
                {
                    self.token_cache
                        .reject_upstream(ApiPaths::BRIDGE_NOTIFICATIONS)
                        .await;
                }
                tracing::warn!(%error, "inbox gateway call failed");
                self.handle.record_error(Some(error.to_string()));
            },
        }
    }

    async fn client(&self) -> Result<(GatewayClient, BearerToken), InboxError> {
        let base = self
            .runtime_config
            .load_full()
            .gateway_base
            .as_ref()
            .clone();
        let token = self.token_cache.current(AUTH_THRESHOLD_SECS).await?;
        Ok((GatewayClient::new(base, self.http.clone()), token.token))
    }

    async fn catch_up(&self) -> Result<(), InboxError> {
        let (client, bearer) = self.client().await?;
        let fresh = catch_up(&self.handle, |cursor, limit| {
            let client = &client;
            let bearer = &bearer;
            async move {
                client
                    .fetch_notifications(bearer, cursor.as_ref(), limit, false)
                    .await
            }
        })
        .await?;
        let toasts = self.handle.update(|state| {
            let arrived: Vec<&Notification> =
                fresh.iter().filter_map(|id| state.find(id)).collect();
            let (toasts, watermark) = plan_toasts(arrived, state.toasted_through, CATCH_UP_TOASTS);
            if watermark.is_some() {
                state.toasted_through = watermark;
            }
            toasts
        });
        self.handle.push_toasts(toasts);
        Ok(())
    }

    async fn load_older(&self) -> Result<(), InboxError> {
        let Some(cursor) = self.handle.snapshot().older_cursor else {
            return Ok(());
        };
        let (client, bearer) = self.client().await?;
        let page = client
            .fetch_notifications(&bearer, Some(&cursor), CATCH_UP_PAGE, false)
            .await?;
        self.handle.update(|state| state.merge_older(page));
        Ok(())
    }

    async fn mark_seen(&self, body: &MarkSeen) -> Result<(), InboxError> {
        let ids = self.handle.claim_unseen(&body.ids);
        if ids.is_empty() {
            return Ok(());
        }
        let (client, bearer) = match self.client().await {
            Ok(pair) => pair,
            Err(error) => {
                self.handle.release_seen(&ids);
                return Err(error);
            },
        };
        let claimed = MarkSeen { ids };
        if let Err(error) = client.mark_notifications_seen(&bearer, &claimed).await {
            self.handle.release_seen(&claimed.ids);
            return Err(error.into());
        }
        Ok(())
    }

    async fn mark_read(&self, body: &MarkRead) -> Result<(), InboxError> {
        let (client, bearer) = self.client().await?;
        let read = client.mark_notifications_read(&bearer, body).await?;
        self.handle
            .update(|state| state.apply_read(&read.ids, read.unread, Utc::now()));
        Ok(())
    }

    async fn escalate(&self, id: &NotificationId, body: Escalate) -> Result<(), InboxError> {
        let (client, bearer) = self.client().await?;
        let escalated = client.escalate_notification(&bearer, id, &body).await?;
        self.handle.update(|state| state.upsert(escalated));
        Ok(())
    }
}
