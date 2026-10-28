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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, DocumentAction, DocumentCommentMentionedEvent, MENTION_EVENT_VERSION,
    MENTION_EXCERPT_CHARS,
};

use super::DocumentService;
use super::ownership::{ShareCause, USER_SUBJECT};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentComment, MentionGrants};

const MAX_MENTIONS: usize = 20;

/// Who wrote a comment on the calling user's behalf: `None` for the user
/// themselves, the agent's name when an agent did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentOrigin {
    pub via_agent: Option<String>,
}

/// The checked mention list and the mentioned users the comment shares the
/// document with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MentionPlan {
    pub(super) mentions: Vec<UserId>,
    pub(super) grants: MentionGrants,
}

/// What a comment says about its mentions: its body, whose
/// `@[Name](user:<id>)` tokens are the mention list, the ids the caller
/// passed (each must appear as a token), and the users the comment already
/// mentioned before an edit, who are neither re-checked nor re-shared with.
#[derive(Debug, Clone, Copy)]
pub(super) struct MentionInput<'a> {
    pub(super) body: &'a str,
    pub(super) passed: &'a [UserId],
    pub(super) existing: &'a [UserId],
}

const TOKEN_OPEN: &str = "@[";
const TOKEN_LINK: &str = "(user:";
const MAX_LABEL_CHARS: usize = 120;

/// The token at the start of `text` (just after `@[`), as its user id and
/// the length it spans: `Name](user:<id>)` with a label of 1 to 120 chars
/// holding none of `[]()` or a newline, and an id with no whitespace or `)`.
/// The workspace's chip pattern is the same grammar, so every id here
/// renders as a chip.
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

/// The user ids of the body's `@[Name](user:<id>)` tokens, in order, once
/// each. Any other `@[` followed by `](user:` is a token that breaks the
/// grammar, and the body is refused rather than read as text that grants
/// nothing but looks like a mention.
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

/// Whether `text` (just after `@[`) reaches `](user:` before the next `@[`
/// or newline: an attempted token.
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
    /// The level each of `users` holds on `document`, `None` for no access,
    /// for a caller who can comment there (and so could mention them): what
    /// the people picker shows beside each match.
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
            self.access
                .validate_subject(USER_SUBJECT, user.as_str())
                .await?;
        }
        let levels = self.access.levels_of(document, &added).await?;
        let users: Vec<UserId> = added
            .into_iter()
            .zip(levels)
            .filter_map(|(user, level)| level.is_none().then_some(user))
            .collect();
        if !users.is_empty() {
            self.access
                .authorize(
                    who,
                    AccessTarget::Document(document.id.clone()),
                    DocumentAction::Share,
                    trace,
                )
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

    /// After a comment landed: the `document.shared` event for each user it
    /// shared the document with, and the mention event for each user in
    /// `mentioned`.
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
            self.announce_share(who, document, grant, None, cause)
                .await;
        }
    }
}
