//! Mentions that share: who a comment mentions, who among them it must
//! share the document with, and what each of them is told afterwards. The
//! REST routes and the MCP comment tool both reach this through
//! [`DocumentService::create_comment_from`] and
//! [`DocumentService::edit_comment_from`], so a person tagged by an agent is
//! shared with and told exactly as one tagged in the workspace.
//!
//! The mention list is the body's `@[Name](user:<id>)` tokens; an id passed
//! alongside must appear as one. A newly mentioned user must be an active
//! person (never a service account). One who holds no access to the
//! document is given `comment` (the lowest level that reads and answers a
//! thread) in the comment's own statement, which needs the author to hold
//! `share`; without it the comment is refused, and nothing is stored. The
//! agent never holds more than its user: `who` is always the calling user.
//! An agent's mention never grants unless the agent access policy allows the
//! mention-grant capability. Mentioning someone without access is refused as
//! `agent_forbidden`, and a mention of someone who has access only notifies.
//! Once the policy allows it, the share check is the user's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentName, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, DocumentAction, DocumentCommentMentionedEvent, MENTION_EVENT_VERSION,
    MENTION_EXCERPT_CHARS,
};

use super::DocumentService;
use super::ownership::{ShareCause, USER_SUBJECT};
use crate::documents::access::{AccessTarget, SubjectValidation};
use crate::documents::agents::{AgentCapability, DocumentActor, act_as};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentComment, MentionGrants};

const MAX_MENTIONS: usize = 20;

/// Who wrote a comment on the calling user's behalf: `None` for the user
/// themselves, the agent's name when an agent did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentOrigin {
    pub via_agent: Option<AgentName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MentionPlan {
    pub(super) mentions: Vec<UserId>,
    pub(super) grants: MentionGrants,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MentionInput<'a> {
    pub(super) body: &'a str,
    pub(super) passed: &'a [UserId],
    pub(super) existing: &'a [UserId],
}

const TOKEN_OPEN: &str = "@[";
const TOKEN_LINK: &str = "(user:";
const MAX_LABEL_CHARS: usize = 120;

fn token_at(text: &str) -> Option<(&str, usize)> {
    let label_end = text.find(']')?;
    let label = &text[..label_end];
    let label_chars = label.chars().count();
    let clean = !label.contains(['[', '(', ')', '\n']);
    if label_chars == 0 || label_chars > MAX_LABEL_CHARS || !clean {
        return None;
    }
    let after = text[label_end + 1..].strip_prefix(TOKEN_LINK)?;
    let close = after.find(')')?;
    let id = &after[..close];
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    Some((id, label_end + 1 + TOKEN_LINK.len() + close + 1))
}

pub(super) fn token_ids(body: &str) -> Result<Vec<UserId>, DocumentsError> {
    let mut ids: Vec<UserId> = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find(TOKEN_OPEN) {
        rest = &rest[open + TOKEN_OPEN.len()..];
        if let Some((id, span)) = token_at(rest) {
            let id = UserId::new(id);
            if !ids.contains(&id) {
                ids.push(id);
            }
            rest = &rest[span..];
        } else if looks_like_token(rest) {
            return Err(DocumentsError::InvalidComment {
                detail: "a mention token is @[Name](user:<id>) with a name of 1 to 120 \
                         characters and none of [ ] ( ) or a newline",
            });
        }
    }
    Ok(ids)
}

fn looks_like_token(text: &str) -> bool {
    let stop = [text.find(TOKEN_OPEN), text.find('\n')]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(text.len());
    text[..stop].contains("](user:")
}

fn checked(input: &MentionInput<'_>) -> Result<Vec<UserId>, DocumentsError> {
    let mentions = token_ids(input.body)?;
    if input.passed.iter().any(|id| !mentions.contains(id)) {
        return Err(DocumentsError::InvalidComment {
            detail: "every mention must appear in the body as @[Name](user:<id>)",
        });
    }
    if mentions.len() > MAX_MENTIONS {
        return Err(DocumentsError::InvalidComment {
            detail: "a comment mentions at most 20 users",
        });
    }
    Ok(mentions)
}

impl DocumentService {
    pub async fn levels_on(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        users: &[UserId],
        trace: &TraceId,
    ) -> Result<Vec<Option<AccessLevel>>, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::Comment, trace)
            .await?;
        self.access.levels_of(&document, users).await
    }

    pub(super) async fn plan_mentions(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        input: MentionInput<'_>,
        context: (&CommentOrigin, &TraceId),
    ) -> Result<MentionPlan, DocumentsError> {
        let (origin, trace) = context;
        let mentions = checked(&input)?;
        let added: Vec<UserId> = mentions
            .iter()
            .filter(|user| !input.existing.contains(user))
            .cloned()
            .collect();
        for user in &added {
            let validation = self
                .access
                .validate_subject(USER_SUBJECT, user.as_str())
                .await?;
            if validation != SubjectValidation::Valid {
                return Err(DocumentsError::UnknownSubject {
                    kind: USER_SUBJECT.to_owned(),
                    value: user.to_string(),
                });
            }
        }
        let levels = self.access.levels_of(document, &added).await?;
        let users: Vec<UserId> = added
            .into_iter()
            .zip(levels)
            .filter_map(|(user, level)| level.is_none().then_some(user))
            .collect();
        if !users.is_empty() {
            self.access
                .require_agent_capability(who, document, AgentCapability::MentionGrant)
                .await?;
            let share = self.access.authorize(
                who,
                AccessTarget::Document(document.id.clone()),
                DocumentAction::Share,
                trace,
            );
            act_as(DocumentActor::Human, share)
                .await
                .map_err(|_denied| DocumentsError::InvalidComment {
                    detail: "a mentioned user holds no access to the document, and sharing it \
                             with them needs share access: ask an owner to share it",
                })?;
        }
        Ok(MentionPlan {
            mentions,
            grants: MentionGrants {
                users,
                granted_by: who.id.clone(),
                trace_id: trace.clone(),
                via_agent: origin.via_agent.clone(),
            },
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the mention notice names the actor, the document, the comment, the mentioned users and the grants, each distinct"
    )]
    pub(super) async fn announce_mentions(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        comment: &DocumentComment,
        mentioned: &[UserId],
        grants: &MentionGrants,
    ) {
        if !grants.users.is_empty() {
            let cause = ShareCause {
                via_agent: grants.via_agent.clone(),
                comment_id: Some(comment.id.clone()),
            };
            self.announce_mention_grants(who, document, grants, &cause)
                .await;
        }
        let excerpt: String = comment.body.chars().take(MENTION_EXCERPT_CHARS).collect();
        for user in mentioned {
            let event = DocumentCommentMentionedEvent {
                document_id: document.id.clone(),
                comment_id: comment.id.clone(),
                mentioned_user_id: user.clone(),
                author_id: who.id.clone(),
                v: MENTION_EVENT_VERSION,
                via_agent: grants.via_agent.clone(),
                mentions: comment.mentions.clone(),
                granted: grants.users.clone(),
                excerpt: excerpt.clone(),
                anchor_quote: comment.anchor.find.clone(),
                created_at: Some(comment.edited_at.unwrap_or(comment.created_at)),
            };
            if let Err(error) = self
                .events
                .comment_mentioned(std::slice::from_ref(user), &event)
                .await
            {
                tracing::warn!(document_id = %document.id, error = %error, "Mention event not relayed");
            }
        }
    }

    async fn announce_mention_grants(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        grants: &MentionGrants,
        cause: &ShareCause,
    ) {
        let all = match self.repos.grants.list_for_document(&document.id).await {
            Ok(all) => all,
            Err(error) => {
                tracing::warn!(document_id = %document.id, error = %error, "Mention grants were stored but not announced");
                return;
            },
        };
        let granted = all.iter().filter(|grant| {
            grant.subject_kind == USER_SUBJECT
                && grants
                    .users
                    .iter()
                    .any(|user| user.as_str() == grant.subject_value)
        });
        for grant in granted {
            let recipient = UserId::new(grant.subject_value.clone());
            self.announce_share(
                who,
                document,
                grant,
                std::slice::from_ref(&recipient),
                None,
                cause,
            )
            .await;
        }
    }
}
