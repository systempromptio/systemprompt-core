//! [`Members`]: audience resolver calls under the profile's resolver timeout.
//!
//! An expansion asks the resolver for `limit + 1` members; more than `limit`
//! withholds the whole subject, never a truncated list. A membership check
//! answers one user against one subject without expanding it; a subject no
//! resolver handles is [`Membership::Unknown`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::{
    Membership, NotificationAudienceError, NotificationAudienceResolver,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::AudienceSubject;

use super::withheld::Withheld;

#[derive(Clone, Copy)]
pub(crate) struct Members<'a> {
    pub(crate) resolvers: &'a [Arc<dyn NotificationAudienceResolver>],
    pub(crate) limit: usize,
    pub(crate) timeout: Duration,
}

impl Members<'_> {
    fn resolver(
        &self,
        subject: &AudienceSubject,
    ) -> Option<&Arc<dyn NotificationAudienceResolver>> {
        self.resolvers.iter().find(|r| r.handles(subject))
    }

    pub(crate) async fn expand(&self, subject: &AudienceSubject) -> Result<Vec<UserId>, Withheld> {
        if let AudienceSubject::User(user) = subject {
            return Ok(vec![user.clone()]);
        }
        let resolver = self.resolver(subject).ok_or(Withheld::NoResolver)?;
        let asked = self.limit.saturating_add(1);
        let members = tokio::time::timeout(self.timeout, resolver.members(subject, asked))
            .await
            .map_err(|_elapsed| Withheld::Timeout)?
            .map_err(|error| match error {
                NotificationAudienceError::TooLarge { .. } => {
                    Withheld::OverLimit { limit: self.limit }
                },
                other => Withheld::Resolver(Arc::new(other)),
            })?;
        if members.len() > self.limit {
            return Err(Withheld::OverLimit { limit: self.limit });
        }
        Ok(members)
    }

    pub(crate) async fn is_member(
        &self,
        subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, Withheld> {
        if let AudienceSubject::User(member) = subject {
            return Ok(if member == user {
                Membership::Member
            } else {
                Membership::NotMember
            });
        }
        let Some(resolver) = self.resolver(subject) else {
            return Ok(Membership::Unknown);
        };
        tokio::time::timeout(self.timeout, resolver.is_member(subject, user))
            .await
            .map_err(|_elapsed| Withheld::Timeout)?
            .map_err(|error| Withheld::Resolver(Arc::new(error)))
    }

    pub(crate) async fn gate(
        &self,
        audience: &[AudienceSubject],
        user: &UserId,
    ) -> Result<(), Withheld> {
        let mut blocked: Option<Withheld> = None;
        for subject in audience {
            match self.is_member(subject, user).await {
                Ok(Membership::Member) => return Ok(()),
                Ok(Membership::NotMember) => {},
                Ok(Membership::Unknown) => {
                    if blocked.is_none() {
                        blocked = Some(Withheld::AudienceUnknown);
                    }
                },
                Err(withheld) => blocked = Some(withheld),
            }
        }
        Err(blocked.unwrap_or(Withheld::NotInAudience))
    }
}
