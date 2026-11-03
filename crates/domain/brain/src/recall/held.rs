//! The working-set keys a principal holds, built once per recall.
//!
//! Every principal holds `org` and `user:<id>`; a request attributed to a
//! project holds `project:<key>`; and every audience subject an injected
//! resolver can enumerate for the user is rendered in its `Display` form
//! (`role:<name>`, `user:<id>`, `<dimension>:<value>`), with `everyone`
//! folded into `org`. A resolver that answers `None` contributes nothing,
//! and its subjects stay on the per-fact membership check, which remains
//! as defence in depth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::ScopeLevel;
use systemprompt_models::notifications::AudienceSubject;

/// The keys a principal holds and the resolvers that enumerated them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldKeys {
    /// Sorted, deduplicated working-set scope keys.
    pub keys: Vec<String>,
    /// Names of the resolvers whose subjects are fully listed in `keys`;
    /// a subject one of them handles but did not list is not held.
    pub enumerated: Vec<&'static str>,
}

impl HeldKeys {
    #[must_use]
    pub fn holds(&self, subject: &AudienceSubject) -> bool {
        self.keys.binary_search(&key_for(subject)).is_ok()
    }

    #[must_use]
    pub fn enumerated_by(&self, resolver: &str) -> bool {
        self.enumerated.contains(&resolver)
    }
}

/// The working-set key an audience subject is indexed under.
#[must_use]
pub fn key_for(subject: &AudienceSubject) -> String {
    match subject {
        AudienceSubject::Everyone => ScopeLevel::Org.as_str().to_owned(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn user_key(user: &UserId) -> String {
    AudienceSubject::User(user.clone()).to_string()
}

#[must_use]
pub fn project_key(project: &str) -> String {
    format!("{}:{project}", ScopeLevel::Project.as_str())
}

pub async fn build(
    user: &UserId,
    project: Option<&str>,
    audiences: &[Arc<dyn NotificationAudienceResolver>],
) -> HeldKeys {
    let mut keys = vec![ScopeLevel::Org.as_str().to_owned(), user_key(user)];
    if let Some(project) = project {
        keys.push(project_key(project));
    }
    let mut enumerated = Vec::new();
    for resolver in audiences {
        if let Some(subjects) = resolver.held(user).await {
            keys.extend(subjects.iter().map(key_for));
            enumerated.push(resolver.name());
        }
    }
    keys.sort();
    keys.dedup();
    HeldKeys { keys, enumerated }
}
