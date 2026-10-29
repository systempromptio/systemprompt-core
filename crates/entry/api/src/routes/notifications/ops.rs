//! The notification operations both HTTP surfaces share: the user routes
//! and the bridge routes parse their own request and caller, then call one
//! of these with the service, the user and the surface they record.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationId, NotificationSubscriptionId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::notifications::{
    Attention, Escalate, KeysetCursor, MarkRead, MarkSeen, Notification, NotificationPage,
    StreamDecl, SubscriptionDecl,
};
use systemprompt_notifications::service::{FeedQuery, NotificationsService, SubscriptionView};

use crate::error::ApiHttpError;

const DEFAULT_LIMIT: u32 = 50;

const MAX_LIMIT: u32 = 100;

#[derive(Debug, Default, Deserialize)]
pub struct FeedParams {
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    attention: Option<Attention>,
    #[serde(default)]
    unread: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UnreadCount {
    pub unread: i64,
    pub unread_capped: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SeenResponse {
    pub seen: usize,
}

#[derive(Debug, Serialize)]
pub struct MarkReadResponse {
    pub ids: Vec<NotificationId>,
    pub unread: i64,
}

#[derive(Debug, Serialize)]
pub struct StreamList {
    pub streams: Vec<StreamDecl>,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionList {
    pub subscriptions: Vec<SubscriptionView>,
}

fn feed_query(params: FeedParams) -> Result<FeedQuery, ApiHttpError> {
    let since = params
        .since
        .as_deref()
        .map(str::parse::<KeysetCursor>)
        .transpose()
        .map_err(|e| ApiError::bad_request("invalid since cursor").with_source(e))?;
    Ok(FeedQuery {
        since,
        limit: params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
        attention: params.attention,
        unread_only: params.unread.unwrap_or(false),
    })
}

pub async fn feed(
    service: &NotificationsService,
    user: &UserId,
    params: FeedParams,
) -> Result<NotificationPage, ApiHttpError> {
    Ok(service.feed(user, feed_query(params)?).await?)
}

pub async fn unread(
    service: &NotificationsService,
    user: &UserId,
) -> Result<UnreadCount, ApiHttpError> {
    let (unread, unread_capped) = service.unread_state(user).await?;
    Ok(UnreadCount {
        unread,
        unread_capped,
    })
}

pub async fn seen(
    service: &NotificationsService,
    user: &UserId,
    request: MarkSeen,
) -> Result<SeenResponse, ApiHttpError> {
    Ok(SeenResponse {
        seen: service.seen(user, &request.ids).await?,
    })
}

pub async fn mark_read(
    service: &NotificationsService,
    user: &UserId,
    request: MarkRead,
    surface: &str,
) -> Result<MarkReadResponse, ApiHttpError> {
    let outcome = service.mark_read(user, request, surface).await?;
    Ok(MarkReadResponse {
        ids: outcome.ids,
        unread: outcome.unread,
    })
}

pub async fn escalate(
    service: &NotificationsService,
    user: &UserId,
    id: &str,
    request: Escalate,
    surface: &str,
) -> Result<Notification, ApiHttpError> {
    let id = NotificationId::try_new(id)?;
    Ok(service.escalate(user, &id, request, surface).await?)
}

pub async fn streams(
    service: &NotificationsService,
    user: &UserId,
) -> Result<StreamList, ApiHttpError> {
    Ok(StreamList {
        streams: service.streams(user).await?,
    })
}

pub async fn subscriptions(
    service: &NotificationsService,
    user: &UserId,
) -> Result<SubscriptionList, ApiHttpError> {
    Ok(SubscriptionList {
        subscriptions: service.subscriptions(user).await?,
    })
}

pub async fn subscribe(
    service: &NotificationsService,
    user: &UserId,
    decl: SubscriptionDecl,
) -> Result<SubscriptionView, ApiHttpError> {
    Ok(service.subscribe(user, decl).await?)
}

pub async fn unsubscribe(
    service: &NotificationsService,
    user: &UserId,
    id: &str,
) -> Result<(), ApiHttpError> {
    let id = NotificationSubscriptionId::try_new(id)?;
    Ok(service.unsubscribe(user, &id).await?)
}
