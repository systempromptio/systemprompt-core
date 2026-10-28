//! Mentions from an agent: each entry of `mentions` is a user id or the
//! email of an active person, resolved here to an id and written into the
//! body as an `@[Name](user:<id>)` token (the service reads the mention list
//! from the body's tokens), and the comment is
//! written through the same service call the workspace uses, marked as the
//! agent's. The service then shares the document with anyone mentioned who
//! cannot see it, under the calling user's own share rights.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::documents::service::CommentOrigin;
use systemprompt_identifiers::{ActorKind, UserId};
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};
use systemprompt_users::{User, UserRepository, normalise_email};

use super::refusal::refusal;
use crate::documents::refusal::RefusalCode;

const UNNAMED_AGENT: &str = "agent";
const MAX_MENTIONS: usize = 20;

const MAX_AGENT_NAME: usize = 64;

pub(super) fn agent_origin(ctx: &RequestContext) -> CommentOrigin {
    let name = ctx.agent_name().as_str();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/');
    let valid = !name.is_empty() && name.len() <= MAX_AGENT_NAME && name.chars().all(allowed);
    CommentOrigin {
        via_agent: Some(if valid { name } else { UNNAMED_AGENT }.to_owned()),
    }
}

/// The actor every documents tool call runs as: always an agent, since
/// this surface is driven by one, named as [`agent_origin`] records it.
/// `attended` is `false` for a headless call: a request marked
/// non-interactive, or one whose actor is a scheduled job or the system.
/// Nothing else marks a run headless today, so an agent run that reaches
/// the tools with a user's token counts as attended.
pub(super) fn document_actor(ctx: &RequestContext) -> DocumentActor {
    let headless_mode =
        ctx.settings.user_interaction_mode == Some(UserInteractionMode::NonInteractive);
    let headless_actor = matches!(ctx.actor().kind, ActorKind::Job { .. } | ActorKind::System);
    let name = agent_origin(ctx)
        .via_agent
        .unwrap_or_else(|| UNNAMED_AGENT.to_owned());
    DocumentActor::agent(name, !(headless_mode || headless_actor))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedMentions {
    pub(super) body: String,
    pub(super) ids: Vec<UserId>,
}

const MAX_LABEL_CHARS: usize = 120;

fn label_of(user: &User) -> String {
    let clean = |name: &str| -> String {
        name.replace(['[', ']', '(', ')', '\n'], " ")
            .trim()
            .chars()
            .take(MAX_LABEL_CHARS)
            .collect()
    };
    [
        user.display_name.as_deref(),
        user.full_name.as_deref(),
        Some(user.name.as_str()),
        Some(user.email.as_str()),
    ]
    .into_iter()
    .flatten()
    .map(clean)
    .find(|label| !label.is_empty())
    .unwrap_or_else(|| user.id.as_str().to_owned())
}

fn plain_mention(body: &str, mention: &str) -> Option<std::ops::Range<usize>> {
    let needle = format!("@{}", mention.to_ascii_lowercase());
    let lower = body.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(&needle) {
        let start = from + found;
        let end = start + needle.len();
        let boundary = lower[end..].chars().next().is_none_or(|c| {
            c.is_whitespace() || (c.is_ascii_punctuation() && !matches!(c, '.' | '@' | '-' | '_'))
        });
        if boundary {
            return Some(start..end);
        }
        from = end;
    }
    None
}

async fn find_person(users: &UserRepository, mention: &UserId) -> Result<User, McpError> {
    let found = if mention.as_str().contains('@') {
        users.find_by_email(mention.as_str()).await
    } else {
        users.find_by_id(mention).await
    };
    found
        .map_err(|error| {
            tracing::warn!(error = %error, "a mentioned user could not be looked up");
            refusal(
                RefusalCode::UnknownSubject,
                format!("could not look up {mention}"),
            )
        })?
        .filter(|user| user.is_active() && !user.is_service_account())
        .ok_or_else(|| {
            refusal(
                RefusalCode::UnknownSubject,
                format!(
                    "{mention} is not an active person in this directory; mention people only \
                     (documents_people finds them)"
                ),
            )
        })
}

pub(super) async fn resolve_mentions(
    users: &UserRepository,
    mentions: Vec<UserId>,
    body: String,
) -> Result<ResolvedMentions, McpError> {
    let mut unique: Vec<UserId> = Vec::with_capacity(mentions.len());
    for mention in mentions {
        let mention = if mention.as_str().contains('@') {
            UserId::new(normalise_email(mention.as_str()))
        } else {
            mention
        };
        if !unique.contains(&mention) {
            unique.push(mention);
        }
    }
    if unique.len() > MAX_MENTIONS {
        return Err(refusal(
            RefusalCode::InvalidParams,
            "a comment mentions at most 20 users",
        ));
    }
    let mut body = body;
    let mut ids = Vec::with_capacity(unique.len());
    for mention in unique {
        let person = find_person(users, &mention).await?;
        let token = format!("@[{}](user:{})", label_of(&person), person.id);
        if !body.contains(&format!("(user:{})", person.id)) {
            match plain_mention(&body, mention.as_str()) {
                Some(range) => body.replace_range(range, &token),
                None => body = format!("{body} {token}"),
            }
        }
        if !ids.contains(&person.id) {
            ids.push(person.id);
        }
    }
    Ok(ResolvedMentions { body, ids })
}
