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
use systemprompt_files::documents::service::CommentOrigin;
use systemprompt_identifiers::UserId;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_users::{User, UserRepository, normalise_email};

use super::refusal::refusal;
use crate::documents::refusal::RefusalCode;

const UNNAMED_AGENT: &str = "agent";
const MAX_MENTIONS: usize = 20;

const MAX_AGENT_NAME: usize = 64;

/// The agent named on the request, as the comment and grant audit record
/// it: at most 64 chars of letters, digits, `-`, `_`, `.`, `:` or `/`, with
/// anything else (a free-text name a client sent) recorded as `agent`.
pub(super) fn agent_origin(ctx: &RequestContext) -> CommentOrigin {
    let name = ctx.agent_name().as_str();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/');
    let valid = !name.is_empty() && name.len() <= MAX_AGENT_NAME && name.chars().all(allowed);
    CommentOrigin {
        via_agent: Some(if valid { name } else { UNNAMED_AGENT }.to_owned()),
    }
}

/// A comment body with its mentions written as tokens, and their ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedMentions {
    pub(super) body: String,
    pub(super) ids: Vec<UserId>,
}

const MAX_LABEL_CHARS: usize = 120;

/// A token label the workspace renders as a chip: 1 to 120 chars with no
/// brackets, parentheses or newlines, falling back to the email, then the id.
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

/// The byte range of the first `@<mention>` in `body` that ends at a word
/// boundary (whitespace, punctuation other than `.@-_`, or the end), compared
/// ASCII-case-insensitively (an id or email is ASCII, and ASCII folding keeps
/// every byte offset), so `@bob` never matches inside `@bobby`.
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
            refusal(
                RefusalCode::UnknownSubject,
                format!("could not look up {mention}: {error}"),
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

/// Resolves each mention (a user id or an email, compared lower-cased),
/// once each and at most 20, to an active person and
/// writes it into the body as `@[Name](user:<id>)`: a plain `@<id>` or
/// `@<email>` in the body becomes the token, and a mention the body does not
/// name yet is appended, so the body always carries what it mentions.
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
