use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::{DurableOutbox, OutboxConsumer};
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::notifications::{
    ChannelError, DynNotificationSink, NotificationChannel, ResourceVisibility, Visibility,
    VisibilityError,
};
use systemprompt_identifiers::{
    DocumentCommentId, DocumentId, EventOutboxId, NotificationId, NotificationRuleId,
    NotificationStreamId, UserId,
};
use systemprompt_manifest::profile::{ConsumerProfile, NotificationsProfile};
use systemprompt_models::notifications::{
    Attention, AudienceSubject, NOTIFICATIONS_CONSUMER, Notification, NotificationEvent,
    NotificationKind, RuleAction, RuleDecl, RuleMatch, StreamAttention, StreamDecl, SubjectRef,
    SubscriptionDecl, SubscriptionTarget,
};
use systemprompt_notifications::defaults::{DefaultPublishPolicy, DefaultRouter};
use systemprompt_notifications::pipeline::{
    BatchContext, DeliverOutcome, NotificationPipeline, PipelineParts, ProcessOutcome,
};
use systemprompt_notifications::repository::{
    NotificationRuleRepository, NotificationStreamRepository, NotificationSubscriptionRepository,
};
use systemprompt_notifications::service::{NotificationsService, NotificationsServiceParts};
use systemprompt_notifications::sink::OutboxNotificationSink;
use systemprompt_notifications::{NotificationRepositories, NotificationsConsumer};
use systemprompt_runtime::notifications::UserRoleAudienceResolver;
use systemprompt_test_fixtures::{DisposableDb, unique_instance};
use systemprompt_users::UserRepository;

#[derive(Default)]
pub(crate) struct Hidden {
    readers: Mutex<HashSet<UserId>>,
}

impl Hidden {
    pub(crate) fn hide_from(&self, reader: &UserId) {
        self.readers.lock().expect("lock").insert(reader.clone());
    }
}

#[async_trait]
impl ResourceVisibility for Hidden {
    fn kinds(&self) -> &'static [&'static str] {
        &["document", "comment"]
    }

    async fn visible(
        &self,
        _subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        if self.readers.lock().expect("lock").contains(reader) {
            Ok(Visibility::Hidden)
        } else {
            Ok(Visibility::Visible {
                excerpt_allowed: true,
            })
        }
    }
}

#[derive(Default)]
pub(crate) struct Recorded {
    delivered: Mutex<Vec<(NotificationId, UserId, i64)>>,
}

impl Recorded {
    pub(crate) fn deliveries(&self) -> Vec<(NotificationId, UserId, i64)> {
        self.delivered.lock().expect("lock").clone()
    }
}

#[async_trait]
impl NotificationChannel for Recorded {
    fn surface(&self) -> &'static str {
        "test"
    }

    fn delivers(&self, _attention: Attention) -> bool {
        true
    }

    async fn deliver(&self, notification: &Notification, unread: i64) -> Result<(), ChannelError> {
        self.delivered.lock().expect("lock").push((
            notification.id.clone(),
            notification.recipient.clone(),
            unread,
        ));
        Ok(())
    }
}

pub(crate) struct Drained {
    pub(crate) ids: Vec<EventOutboxId>,
    pub(crate) outcomes: Vec<ProcessOutcome>,
    pub(crate) pending: Vec<NotificationId>,
    pub(crate) delivered: DeliverOutcome,
}

impl Drained {
    pub(crate) fn persisted(&self) -> usize {
        self.outcomes.iter().map(|o| o.persisted.len()).sum()
    }
}

pub(crate) const CODE_SOURCE: &str = "code:notifications/subscriptions.yaml";

pub(crate) fn profile() -> NotificationsProfile {
    NotificationsProfile {
        max_audience: 50,
        ..NotificationsProfile::default()
    }
}

pub(crate) struct Harness {
    db: DisposableDb,
    pub(crate) pool: DbPool,
    pub(crate) repos: Arc<NotificationRepositories>,
    pub(crate) pipeline: Arc<NotificationPipeline>,
    pub(crate) service: NotificationsService,
    pub(crate) sink: DynNotificationSink,
    pub(crate) hidden: Arc<Hidden>,
    pub(crate) channel: Arc<Recorded>,
}

impl Harness {
    pub(crate) async fn new(prefix: &str) -> Self {
        Self::with_profile(prefix, profile()).await
    }

    pub(crate) async fn with_profile(prefix: &str, profile: NotificationsProfile) -> Self {
        let db = DisposableDb::with_schema(prefix).await;
        let pool = db.test_pool().await;
        let repos = Arc::new(NotificationRepositories::new(&pool));
        let users = Arc::new(UserRepository::new(&pool));
        let audience: Arc<dyn systemprompt_extension::notifications::NotificationAudienceResolver> =
            Arc::new(UserRoleAudienceResolver::new(users));
        let hidden = Arc::new(Hidden::default());
        let channel = Arc::new(Recorded::default());
        let pipeline = Arc::new(NotificationPipeline::new(PipelineParts {
            routers: vec![Arc::new(DefaultRouter)],
            audiences: vec![Arc::clone(&audience)],
            visibilities: vec![Arc::clone(&hidden) as Arc<dyn ResourceVisibility>],
            channels: vec![Arc::clone(&channel) as Arc<dyn NotificationChannel>],
            repos: Arc::clone(&repos),
            profile,
        }));
        let outbox = DurableOutbox::new(pool.write_pool().as_ref().clone(), unique_instance());
        let sink: DynNotificationSink = Arc::new(OutboxNotificationSink::new(Arc::new(outbox)));
        let service = NotificationsService::new(NotificationsServiceParts {
            repos: Arc::clone(&repos),
            router: Arc::new(EventRouter::local_only(Arc::new(EventBroadcasters::new()))),
            sink: Arc::clone(&sink),
            pipeline: Arc::clone(&pipeline),
            publish_policies: vec![Arc::new(DefaultPublishPolicy::new(vec![audience]))],
            db: Arc::clone(&pool),
        });
        Self {
            db,
            pool,
            repos,
            pipeline,
            service,
            sink,
            hidden,
            channel,
        }
    }

    pub(crate) async fn finish(self) {
        self.pool.write_pool().close().await;
        self.db.drop_now().await;
    }

    pub(crate) async fn user(&self, name: &str, roles: &[&str]) -> UserId {
        let id = UserId::generate();
        let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
        sqlx::query("INSERT INTO users (id, name, email, roles) VALUES ($1, $2, $3, $4)")
            .bind(id.as_str())
            .bind(name)
            .bind(format!("{name}-{}@test.invalid", id.as_str()))
            .bind(&roles)
            .execute(self.pool.write_pool().as_ref())
            .await
            .expect("insert user");
        id
    }

    pub(crate) async fn emit(&self, event: &NotificationEvent) {
        let mut tx = self.pool.write_pool().begin().await.expect("begin");
        self.sink.emit(&mut tx, event).await.expect("emit");
        tx.commit().await.expect("commit");
    }

    pub(crate) async fn process_pending(&self) -> Drained {
        let consumer = OutboxConsumer::new(self.pool.write_pool().as_ref().clone());
        let mut batch = consumer
            .claim_batch(NOTIFICATIONS_CONSUMER, 100, &[])
            .await
            .expect("claim")
            .expect("a pending notification fact");
        let facts = batch
            .facts::<NotificationEvent>()
            .expect("every fact decodes");
        let mut ctx = BatchContext::load(batch.connection())
            .await
            .expect("batch context");
        let mut ids = Vec::new();
        let mut outcomes = Vec::new();
        let mut pending = Vec::new();
        for (id, fact) in facts {
            let outcome = self
                .pipeline
                .process(&id, &fact.data, batch.connection(), &mut ctx)
                .await
                .expect("process");
            pending.extend(outcome.pending.clone());
            ids.push(id);
            outcomes.push(outcome);
        }
        batch.acknowledge_all().await.expect("acknowledge");
        Drained {
            ids,
            outcomes,
            pending,
            delivered: DeliverOutcome::default(),
        }
    }

    pub(crate) async fn drain(&self) -> Drained {
        let mut drained = self.process_pending().await;
        drained.delivered = self.pipeline.deliver(&drained.pending).await;
        drained
    }

    pub(crate) async fn reprocess(
        &self,
        id: &EventOutboxId,
        event: &NotificationEvent,
    ) -> ProcessOutcome {
        let mut tx = self.pool.write_pool().begin().await.expect("begin");
        let mut ctx = BatchContext::load(&mut tx).await.expect("batch context");
        let outcome = self
            .pipeline
            .process(id, event, &mut tx, &mut ctx)
            .await
            .expect("process");
        tx.commit().await.expect("commit");
        outcome
    }

    pub(crate) fn consumer(&self, settings: ConsumerProfile) -> NotificationsConsumer {
        let pool = self.pool.write_pool().as_ref().clone();
        NotificationsConsumer::new(Arc::clone(&self.pipeline), pool.clone(), pool, settings)
    }

    pub(crate) async fn declare_stream(
        &self,
        name: &str,
        audience: Vec<AudienceSubject>,
        publishers: Vec<AudienceSubject>,
        attention: StreamAttention,
    ) -> NotificationStreamId {
        let id = NotificationStreamId::try_new(name).expect("stream id");
        let stream = StreamDecl {
            id: id.clone(),
            name: name.to_owned(),
            description: None,
            audience,
            publishers,
            attention,
            retention_days: None,
            coalesce_window_secs: None,
        };
        NotificationStreamRepository::upsert(self.repos.write_pool(), &stream, "code:streams.yaml")
            .await
            .expect("declare stream");
        id
    }

    pub(crate) async fn declare_subscription(&self, decl: &SubscriptionDecl) {
        NotificationSubscriptionRepository::upsert(
            self.repos.write_pool(),
            decl,
            CODE_SOURCE,
            None,
        )
        .await
        .expect("declare subscription")
        .expect("a declared subscription row");
    }

    pub(crate) async fn failures(&self, reason: &str) -> Vec<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT recipient_user_id FROM notification_delivery_failures WHERE reason = $1 \
             ORDER BY id",
        )
        .bind(reason)
        .fetch_all(self.pool.write_pool().as_ref())
        .await
        .expect("failures")
    }

    pub(crate) async fn receipts(&self, state: &str) -> i64 {
        self.count(
            "SELECT COUNT(*) FROM notification_receipts WHERE state = $1",
            state,
        )
        .await
    }

    pub(crate) async fn declare_rule(
        &self,
        matcher: RuleMatch,
        action: RuleAction,
    ) -> NotificationRuleId {
        let rule = RuleDecl {
            id: NotificationRuleId::generate(),
            name: "test rule".to_owned(),
            priority: 0,
            r#match: matcher,
            action,
        };
        NotificationRuleRepository::upsert_declared(self.repos.write_pool(), &rule, "test")
            .await
            .expect("declare rule");
        rule.id
    }

    pub(crate) async fn count(&self, sql: &'static str, bind: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .bind(bind)
            .fetch_one(self.pool.write_pool().as_ref())
            .await
            .expect("count")
    }

    pub(crate) async fn rows_for(&self, user: &UserId) -> Vec<Notification> {
        let mut rows = self
            .repos
            .notifications
            .list_feed(
                user,
                systemprompt_notifications::FeedFilter {
                    since: None,
                    limit: 100,
                    attention: None,
                    unread_only: false,
                },
            )
            .await
            .expect("list");
        rows.reverse();
        rows
    }
}

pub(crate) fn mention(
    actor: &UserId,
    recipient: &UserId,
    document: &DocumentId,
    at: DateTime<Utc>,
) -> (NotificationEvent, DocumentCommentId) {
    let comment = DocumentCommentId::generate();
    let event = NotificationEvent::new(
        NotificationKind::new(NotificationKind::MENTIONED).expect("kind"),
        actor.clone(),
        at,
    )
    .with_subjects([
        SubjectRef::new("document", document.as_str()),
        SubjectRef::new("comment", comment.as_str()),
    ])
    .with_direct_recipient(recipient.clone(), None)
    .with_excerpt("please review the pricing table");
    (event, comment)
}

pub(crate) fn stream_subscription(
    subject: AudienceSubject,
    stream: &NotificationStreamId,
) -> SubscriptionDecl {
    SubscriptionDecl {
        subject,
        target: SubscriptionTarget::Stream(stream.clone()),
        attention_ceiling: None,
        quiet_hours: None,
        digest: None,
    }
}

pub(crate) fn share(
    actor: &UserId,
    recipient: &UserId,
    document: &DocumentId,
) -> NotificationEvent {
    NotificationEvent::new(
        NotificationKind::new(NotificationKind::SHARED).expect("kind"),
        actor.clone(),
        Utc::now(),
    )
    .with_subject(SubjectRef::new("document", document.as_str()))
    .with_direct_recipient(recipient.clone(), None)
}
