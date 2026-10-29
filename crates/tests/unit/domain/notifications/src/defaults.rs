use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_extension::notifications::{
    Membership, NotificationAudienceError, NotificationAudienceResolver, NotificationRouter,
    PublishPolicy, PublishVerdict,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, NotificationKind, RecipientDecision,
    StreamAttention,
};
use systemprompt_notifications::defaults::{DefaultPublishPolicy, DefaultRouter};

use crate::fixtures::{event, stream, stream_id};

const TIMEOUT: Duration = Duration::from_secs(5);

fn user(id: &str) -> UserId {
    UserId::new(id)
}

fn decision(id: &str, attention: Attention, reason: AttentionReason) -> RecipientDecision {
    RecipientDecision {
        user: user(id),
        attention,
        reason,
    }
}

#[tokio::test]
async fn a_mention_defaults_to_human_and_honours_the_senders_choice() {
    let mention = event(NotificationKind::MENTIONED)
        .with_direct_recipient(user("b"), None)
        .with_direct_recipient(user("c"), Some(Attention::Agent));
    let plan = DefaultRouter.route(&mention).await.expect("routes");
    assert_eq!(
        plan.recipients,
        vec![
            decision("b", Attention::Human, AttentionReason::Default),
            decision("c", Attention::Agent, AttentionReason::Sender),
        ]
    );
}

#[tokio::test]
async fn a_mention_sender_attention_applies_to_everyone_unchosen() {
    let mention = event(NotificationKind::MENTIONED)
        .with_direct_recipient(user("b"), None)
        .with_sender_attention(Attention::Silent);
    let plan = DefaultRouter.route(&mention).await.expect("routes");
    assert_eq!(
        plan.recipients,
        vec![decision("b", Attention::Silent, AttentionReason::Sender)]
    );
}

#[tokio::test]
async fn a_share_is_agent_whatever_the_sender_asked() {
    let share = event(NotificationKind::SHARED)
        .with_direct_recipient(user("b"), Some(Attention::Human))
        .with_sender_attention(Attention::Human);
    let plan = DefaultRouter.route(&share).await.expect("routes");
    assert_eq!(
        plan.recipients,
        vec![decision("b", Attention::Agent, AttentionReason::Default)]
    );
}

#[tokio::test]
async fn a_stream_publication_routes_nobody_directly_and_names_its_stream() {
    let published = event(NotificationKind::STREAM_PUBLISHED)
        .with_stream(stream_id("news"))
        .with_direct_recipient(user("b"), Some(Attention::Human));
    let plan = DefaultRouter.route(&published).await.expect("routes");
    assert!(plan.recipients.is_empty());
    assert_eq!(plan.stream, Some(stream_id("news")));
}

#[tokio::test]
async fn an_unknown_kind_is_silent_unless_the_producer_chose() {
    let custom = event("deal.stage_changed").with_direct_recipient(user("b"), None);
    let plan = DefaultRouter.route(&custom).await.expect("routes");
    assert_eq!(
        plan.recipients,
        vec![decision("b", Attention::Silent, AttentionReason::Default)]
    );
    let chosen = event("deal.stage_changed")
        .with_direct_recipient(user("b"), None)
        .with_sender_attention(Attention::Agent);
    let plan = DefaultRouter.route(&chosen).await.expect("routes");
    assert_eq!(
        plan.recipients,
        vec![decision("b", Attention::Agent, AttentionReason::Sender)]
    );
}

#[tokio::test]
async fn an_event_without_direct_recipients_routes_nobody() {
    let plan = DefaultRouter
        .route(&event(NotificationKind::MENTIONED))
        .await
        .expect("routes");
    assert!(plan.is_empty());
    assert_eq!(plan.stream, None);
}

struct Roles {
    members: Vec<(AudienceSubject, Vec<UserId>)>,
    fail: bool,
}

#[async_trait]
impl NotificationAudienceResolver for Roles {
    fn name(&self) -> &'static str {
        "test-roles"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Role(_))
    }

    async fn members(
        &self,
        subject: &AudienceSubject,
        _limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        if self.fail {
            return Err(NotificationAudienceError::TooLarge {
                subject: subject.clone(),
                limit: 0,
            });
        }
        Ok(self
            .members
            .iter()
            .find(|(s, _)| s == subject)
            .map(|(_, users)| users.clone())
            .unwrap_or_default())
    }

    async fn is_member(
        &self,
        subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        let members = self.members(subject, usize::MAX).await?;
        Ok(if members.contains(user) {
            Membership::Member
        } else {
            Membership::NotMember
        })
    }
}

fn policy(fail: bool) -> DefaultPublishPolicy {
    DefaultPublishPolicy::new(
        vec![Arc::new(Roles {
            members: vec![(
                AudienceSubject::Role("leads".to_owned()),
                vec![user("lead")],
            )],
            fail,
        })],
        TIMEOUT,
    )
}

fn publishers(subjects: Vec<AudienceSubject>) -> systemprompt_models::notifications::StreamDecl {
    let mut decl = stream("news", StreamAttention::default());
    decl.publishers = subjects;
    decl
}

#[tokio::test]
async fn a_declared_user_or_role_member_may_publish() {
    let decl = publishers(vec![
        AudienceSubject::User(user("owner")),
        AudienceSubject::Role("leads".to_owned()),
    ]);
    let policy = policy(false);
    assert_eq!(
        policy
            .may_publish(&user("owner"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Admit
    );
    assert_eq!(
        policy
            .may_publish(&user("lead"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Admit
    );
    assert!(matches!(
        policy
            .may_publish(&user("stranger"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Refuse { .. }
    ));
}

#[tokio::test]
async fn everyone_admits_any_publisher_and_an_unhandled_subject_admits_nobody() {
    let policy = policy(false);
    let open = publishers(vec![AudienceSubject::Everyone]);
    assert_eq!(
        policy
            .may_publish(&user("anyone"), &open)
            .await
            .expect("verdict"),
        PublishVerdict::Admit
    );
    let dimension = publishers(vec![AudienceSubject::Dimension {
        kind: "group".to_owned(),
        value: "sales".to_owned(),
    }]);
    assert!(matches!(
        policy
            .may_publish(&user("lead"), &dimension)
            .await
            .expect("verdict"),
        PublishVerdict::Refuse { .. }
    ));
}

#[tokio::test]
async fn a_failing_resolver_is_an_error_not_an_admission() {
    let decl = publishers(vec![AudienceSubject::Role("leads".to_owned())]);
    assert!(
        policy(true)
            .may_publish(&user("lead"), &decl)
            .await
            .is_err()
    );
}

#[derive(Default)]
struct MembershipOnly {
    listed: AtomicUsize,
    checked: AtomicUsize,
}

#[async_trait]
impl NotificationAudienceResolver for MembershipOnly {
    fn name(&self) -> &'static str {
        "test-membership-only"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Role(_))
    }

    async fn members(
        &self,
        subject: &AudienceSubject,
        limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        self.listed.fetch_add(1, Ordering::SeqCst);
        Err(NotificationAudienceError::TooLarge {
            subject: subject.clone(),
            limit,
        })
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        self.checked.fetch_add(1, Ordering::SeqCst);
        Ok(if user == &UserId::new("lead") {
            Membership::Member
        } else {
            Membership::NotMember
        })
    }
}

#[tokio::test]
async fn a_publisher_outside_a_role_is_refused_by_membership_without_listing_the_role() {
    let resolver = Arc::new(MembershipOnly::default());
    let policy = DefaultPublishPolicy::new(
        vec![Arc::clone(&resolver) as Arc<dyn NotificationAudienceResolver>],
        TIMEOUT,
    );
    let decl = publishers(vec![AudienceSubject::Role("everyone-at-scale".to_owned())]);

    assert!(matches!(
        policy
            .may_publish(&user("stranger"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Refuse { .. }
    ));
    assert_eq!(
        policy
            .may_publish(&user("lead"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Admit
    );
    assert_eq!(resolver.checked.load(Ordering::SeqCst), 2);
    assert_eq!(
        resolver.listed.load(Ordering::SeqCst),
        0,
        "a publish check never expands the role"
    );
}

#[tokio::test]
async fn an_unknown_membership_never_admits_a_publisher() {
    struct Unsure;

    #[async_trait]
    impl NotificationAudienceResolver for Unsure {
        fn name(&self) -> &'static str {
            "test-unsure"
        }

        fn handles(&self, _subject: &AudienceSubject) -> bool {
            true
        }

        async fn members(
            &self,
            _subject: &AudienceSubject,
            _limit: usize,
        ) -> Result<Vec<UserId>, NotificationAudienceError> {
            Ok(vec![user("lead")])
        }

        async fn is_member(
            &self,
            _subject: &AudienceSubject,
            _user: &UserId,
        ) -> Result<Membership, NotificationAudienceError> {
            Ok(Membership::Unknown)
        }
    }

    let policy = DefaultPublishPolicy::new(vec![Arc::new(Unsure)], TIMEOUT);
    let decl = publishers(vec![AudienceSubject::Role("leads".to_owned())]);
    assert!(matches!(
        policy
            .may_publish(&user("lead"), &decl)
            .await
            .expect("verdict"),
        PublishVerdict::Refuse { .. }
    ));
}

#[tokio::test]
async fn a_membership_check_that_times_out_refuses_the_publish() {
    struct Stalled;

    #[async_trait]
    impl NotificationAudienceResolver for Stalled {
        fn name(&self) -> &'static str {
            "test-stalled"
        }

        fn handles(&self, _subject: &AudienceSubject) -> bool {
            true
        }

        async fn members(
            &self,
            _subject: &AudienceSubject,
            _limit: usize,
        ) -> Result<Vec<UserId>, NotificationAudienceError> {
            Ok(vec![user("lead")])
        }

        async fn is_member(
            &self,
            _subject: &AudienceSubject,
            _user: &UserId,
        ) -> Result<Membership, NotificationAudienceError> {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok(Membership::Member)
        }
    }

    let policy = DefaultPublishPolicy::new(vec![Arc::new(Stalled)], Duration::from_millis(50));
    let decl = publishers(vec![AudienceSubject::Role("leads".to_owned())]);
    let verdict = tokio::time::timeout(
        Duration::from_secs(5),
        policy.may_publish(&user("lead"), &decl),
    )
    .await
    .expect("the policy answers within its timeout")
    .expect("verdict");
    assert!(
        matches!(verdict, PublishVerdict::Refuse { .. }),
        "{verdict:?}"
    );
}
