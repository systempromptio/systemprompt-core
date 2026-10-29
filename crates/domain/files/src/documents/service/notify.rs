//! Durable notifications the document store records inside the transaction
//! of the write that causes them: a mention (`document.comment.mentioned`)
//! with the comment insert or edit, a share (`document.shared`) with the
//! grant. Each write records exactly one event carrying every direct
//! recipient with its attention; the event goes to the outbox through the
//! injected sink on the write's own connection, so a sink failure rolls the
//! write back: a comment or grant never commits without its notification. The
//! live `DocumentEvents` frames are still sent after commit.
//!
//! A mention may carry a chosen [`Attention`] per mentioned user
//! ([`MentionOption`]); an option for a user the comment does not mention is
//! refused. The author mentioning themselves is never notified.
//!
//! When the injected sink reports itself unavailable (a server built without
//! the notifications fabric) the write commits and no event is recorded,
//! except that a request carrying mention options is refused with
//! [`NotificationSinkError::Unavailable`] before any write: the author asked
//! for an attention level that cannot be honoured.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_extension::NotificationSinkError;
use systemprompt_identifiers::{DocumentCommentId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::AccessLevel;
use systemprompt_models::notifications::{
    Attention, DirectRecipient, NotificationEvent, NotificationKind, SubjectRef,
};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;

const EXCERPT_CHARS: usize = 280;

/// The attention a mentioned user is notified at, chosen by the author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionOption {
    pub user_id: UserId,
    pub attention: Attention,
}

pub(super) fn check_mention_options(
    service: &DocumentService,
    options: &[MentionOption],
    mentions: &[UserId],
) -> Result<(), DocumentsError> {
    if !options.is_empty() && !service.notifications.is_available() {
        return Err(NotificationSinkError::Unavailable.into());
    }
    if options
        .iter()
        .any(|option| !mentions.contains(&option.user_id))
    {
        return Err(DocumentsError::InvalidParams {
            detail: "every mention option must name a user the comment mentions",
        });
    }
    Ok(())
}

pub(super) struct MentionContext<'a> {
    pub(super) who: &'a AuthenticatedUser,
    pub(super) document: &'a Document,
    pub(super) granted: &'a [UserId],
    pub(super) via_agent: Option<&'a str>,
    pub(super) options: &'a [MentionOption],
}

pub(super) struct MentionNotice<'a> {
    pub(super) context: MentionContext<'a>,
    pub(super) comment: &'a DocumentCommentId,
    pub(super) body: &'a str,
    pub(super) anchor_quote: Option<&'a str>,
    pub(super) mentions: &'a [UserId],
    pub(super) notify: &'a [UserId],
    pub(super) occurred_at: DateTime<Utc>,
}

pub(super) struct ShareNotice<'a> {
    pub(super) who: &'a AuthenticatedUser,
    pub(super) document: &'a Document,
    pub(super) recipients: &'a [UserId],
    pub(super) level: AccessLevel,
    pub(super) subject_kind: &'a str,
    pub(super) subject_value: &'a str,
    pub(super) message: Option<&'a str>,
    pub(super) via_agent: Option<&'a str>,
    pub(super) occurred_at: DateTime<Utc>,
}

fn attributed(event: NotificationEvent, via_agent: Option<&str>) -> NotificationEvent {
    let event = event.with_actor_is_agent(via_agent.is_some());
    match via_agent {
        Some(agent) => event.with_via_agent(agent),
        None => event,
    }
}

impl DocumentService {
    pub(super) async fn record_mentions(
        &self,
        conn: &mut PgConnection,
        notice: MentionNotice<'_>,
    ) -> Result<(), DocumentsError> {
        if !self.notifications.is_available() {
            tracing::debug!(
                comment_id = %notice.comment,
                "Notifications unavailable; mention not recorded"
            );
            return Ok(());
        }
        let MentionContext {
            who,
            document,
            granted,
            via_agent,
            options,
        } = notice.context;
        let kind = NotificationKind::new(NotificationKind::MENTIONED)?;
        let excerpt: String = notice.body.chars().take(EXCERPT_CHARS).collect();
        // JSON: the notification event's payload is its free-form contract field.
        let payload = serde_json::json!({
            "document_name": document.name,
            "anchor_quote": notice.anchor_quote,
            "mentions": notice.mentions,
            "granted": granted,
        });
        let recipients: Vec<DirectRecipient> = notice
            .notify
            .iter()
            .filter(|user| **user != who.id)
            .map(|user| DirectRecipient {
                user: user.clone(),
                attention: options
                    .iter()
                    .find(|option| option.user_id == *user)
                    .map(|option| option.attention),
            })
            .collect();
        if recipients.is_empty() {
            return Ok(());
        }
        let event = NotificationEvent::new(kind, who.id.clone(), notice.occurred_at)
            .with_subjects([
                SubjectRef::new("document", document.id.as_str()),
                SubjectRef::new("comment", notice.comment.as_str()),
            ])
            .with_direct_recipients(recipients)
            .with_excerpt(excerpt)
            .with_payload(payload);
        self.notifications
            .emit(conn, &attributed(event, via_agent))
            .await?;
        Ok(())
    }

    pub(super) async fn record_share(
        &self,
        conn: &mut PgConnection,
        notice: ShareNotice<'_>,
    ) -> Result<(), DocumentsError> {
        if !self.notifications.is_available() {
            tracing::debug!(
                document_id = %notice.document.id,
                "Notifications unavailable; share not recorded"
            );
            return Ok(());
        }
        let kind = NotificationKind::new(NotificationKind::SHARED)?;
        // JSON: the notification event's payload is its free-form contract field.
        let payload = serde_json::json!({
            "document_name": notice.document.name,
            "level": notice.level,
            "subject_kind": notice.subject_kind,
            "subject_value": notice.subject_value,
            "message": notice.message,
            "shared_by": notice.who.username,
        });
        if notice.recipients.is_empty() {
            return Ok(());
        }
        let event = NotificationEvent::new(kind, notice.who.id.clone(), notice.occurred_at)
            .with_subject(SubjectRef::new("document", notice.document.id.as_str()))
            .with_direct_recipients(notice.recipients.iter().map(|user| DirectRecipient {
                user: user.clone(),
                attention: None,
            }))
            .with_payload(payload);
        self.notifications
            .emit(conn, &attributed(event, notice.via_agent))
            .await?;
        Ok(())
    }

    pub(super) async fn share_recipients(
        &self,
        who: &AuthenticatedUser,
        subject_kind: &str,
        subject_value: &str,
    ) -> Result<Vec<UserId>, DocumentsError> {
        let members = if subject_kind == super::ownership::USER_SUBJECT {
            vec![UserId::new(subject_value)]
        } else {
            self.access
                .subject_members(subject_kind, subject_value)
                .await?
        };
        Ok(members.into_iter().filter(|user| *user != who.id).collect())
    }
}
