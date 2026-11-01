use std::time::Duration;

use systemprompt_extension::brain::{SourceAccess, SourceAccessError, SourceVerdict};
use systemprompt_extension::notifications::{
    Membership, NotificationAudienceError, NotificationAudienceResolver,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::notifications::{AudienceSubject, SubjectRef};

#[derive(Clone, Copy)]
pub(crate) enum Access {
    Verdict(SourceVerdict),
    Fails,
    Hangs,
}

pub(crate) struct FakeAccess(pub(crate) Access);

#[async_trait::async_trait]
impl SourceAccess for FakeAccess {
    async fn readable(
        &self,
        _subject: &SubjectRef,
        _reader: &UserId,
        _actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        match self.0 {
            Access::Verdict(verdict) => Ok(verdict),
            Access::Fails => Err(SourceAccessError::Other {
                kind: "document".to_owned(),
                source: "boom".into(),
            }),
            Access::Hangs => {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(SourceVerdict::Readable)
            },
        }
    }
}

pub(crate) struct FakeResolver {
    pub(crate) member: UserId,
}

#[async_trait::async_trait]
impl NotificationAudienceResolver for FakeResolver {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(
            subject,
            AudienceSubject::Role(_) | AudienceSubject::Dimension { .. }
        )
    }

    async fn members(
        &self,
        _subject: &AudienceSubject,
        _limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(vec![self.member.clone()])
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(if user == &self.member {
            Membership::Member
        } else {
            Membership::NotMember
        })
    }
}
