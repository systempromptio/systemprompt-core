use std::sync::Arc;

use systemprompt_brain::consumer::BrainConsumer;
use systemprompt_brain::pipeline::{BatchContext, BrainPipeline, PipelineSeams, ProcessOutcome};
use systemprompt_brain::recall::{BrainRecallService, BrainRecallStore, PlaneDeclaredSource};
use systemprompt_brain::service::{BrainCaller, BrainService};
use systemprompt_brain::{BrainRepositories, OutboxBrainSink};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::{DurableOutbox, OutboxConsumer};
use systemprompt_extension::brain::{DynBrainSink, EventFilter, FactExtractor, SourceContent};
use systemprompt_identifiers::{BrainFactId, EventOutboxId, UserId};
use systemprompt_manifest::profile::{AutoConfirm, BrainProfile};
use systemprompt_models::brain::{BRAIN_CONSUMER, BrainEvent, BrainFact};
use systemprompt_test_fixtures::{DisposableDb, unique_instance};

use crate::fakes::{
    FakeModelClient, FakeSourceAccess, FakeSourceContent, GatherFilter, TestParser,
};
use crate::plane::{self, PRICING, kind};

pub(crate) struct Setup {
    pub(crate) profile: BrainProfile,
    pub(crate) gather: Option<Vec<&'static str>>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            profile: BrainProfile {
                enabled: true,
                ..BrainProfile::default()
            },
            gather: Some(vec![PRICING]),
        }
    }
}

impl Setup {
    pub(crate) fn confirming() -> Self {
        let mut setup = Self::default();
        setup.profile.capture.auto_confirm.document = AutoConfirm::Always;
        setup.profile.capture.auto_confirm.others = AutoConfirm::Always;
        setup
    }
}

pub(crate) struct Harness {
    db: DisposableDb,
    pub(crate) pool: DbPool,
    pub(crate) pg: sqlx::PgPool,
    pub(crate) repos: Arc<BrainRepositories>,
    pub(crate) model: Arc<FakeModelClient>,
    pub(crate) parser: Arc<TestParser>,
    pub(crate) access: Arc<FakeSourceAccess>,
    pub(crate) sink: DynBrainSink,
    pub(crate) pipeline: Arc<BrainPipeline>,
    pub(crate) service: BrainService,
    pub(crate) profile: BrainProfile,
    seams: PipelineSeams,
}

impl Harness {
    pub(crate) async fn new(prefix: &str) -> Self {
        Self::with(prefix, Setup::default()).await
    }

    pub(crate) async fn with(prefix: &str, setup: Setup) -> Self {
        let db = DisposableDb::with_schema(prefix).await;
        let pool = db.test_pool().await;
        let pg = pool.write_pool().as_ref().clone();
        plane::apply(&pg, &plane::declared()).await;
        let repos = Arc::new(BrainRepositories::new(&pool));
        let model = Arc::new(FakeModelClient::default());
        let parser = Arc::new(TestParser::default());
        let source = Arc::new(FakeSourceContent::default());
        source.set(crate::events::source_text("the enterprise price list"));
        let seams = PipelineSeams {
            filters: setup
                .gather
                .map(|kinds| {
                    let filter: Arc<dyn EventFilter> = Arc::new(GatherFilter {
                        kinds: kinds.into_iter().map(kind).collect(),
                    });
                    vec![filter]
                })
                .unwrap_or_default(),
            extractors: vec![Arc::clone(&parser) as Arc<dyn FactExtractor>],
            sources: vec![Arc::clone(&source) as Arc<dyn SourceContent>],
            ..PipelineSeams::default()
        };
        let pipeline = BrainPipeline::builder(
            Arc::clone(&repos),
            Arc::clone(&model) as _,
            setup.profile.clone(),
        )
        .seams(seams.clone())
        .build();
        let outbox = DurableOutbox::new(pg.clone(), unique_instance());
        let sink: DynBrainSink = Arc::new(OutboxBrainSink::new(Arc::new(outbox)));
        let service = BrainService::builder(
            (*repos).clone(),
            Arc::clone(&sink),
            Arc::new(setup.profile.clone()),
        )
        .build();
        Self {
            db,
            pool,
            pg,
            repos,
            model,
            parser,
            access: Arc::new(FakeSourceAccess::default()),
            sink,
            pipeline: Arc::new(pipeline),
            service,
            profile: setup.profile,
            seams,
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
            .bind(format!("{name}-{}", id.as_str()))
            .bind(format!("{name}-{}@test.invalid", id.as_str()))
            .bind(&roles)
            .execute(&self.pg)
            .await
            .expect("insert user");
        id
    }

    pub(crate) fn caller(user: &UserId, roles: &[&str]) -> BrainCaller {
        BrainCaller::new(
            user.clone(),
            roles.iter().map(|r| (*r).to_owned()).collect(),
        )
    }

    pub(crate) async fn emit(&self, event: &BrainEvent) -> EventOutboxId {
        let mut tx = self.pg.begin().await.expect("begin");
        let id = self.sink.emit(&mut tx, event).await.expect("emit");
        tx.commit().await.expect("commit");
        id
    }

    pub(crate) async fn drain(&self) -> Vec<(EventOutboxId, ProcessOutcome)> {
        let consumer = OutboxConsumer::new(self.pg.clone());
        let mut processed = Vec::new();
        while let Some(mut delivery) = consumer.claim(BRAIN_CONSUMER).await.expect("claim") {
            let id = delivery.id();
            let event: BrainEvent = delivery.fact::<BrainEvent>().expect("fact decodes").data;
            let ctx = BatchContext::load(delivery.connection())
                .await
                .expect("batch context");
            let outcome = self
                .pipeline
                .process(&id, event, delivery.connection(), &ctx)
                .await
                .expect("process");
            delivery.acknowledge().await.expect("acknowledge");
            processed.push((id, outcome));
        }
        processed
    }

    pub(crate) async fn reprocess(&self, id: &EventOutboxId, event: BrainEvent) -> ProcessOutcome {
        let mut tx = self.pg.begin().await.expect("begin");
        let ctx = BatchContext::load(&mut tx).await.expect("batch context");
        let outcome = self
            .pipeline
            .process(id, event, &mut tx, &ctx)
            .await
            .expect("process");
        tx.commit().await.expect("commit");
        outcome
    }

    pub(crate) async fn count(&self, sql: &'static str, bind: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .bind(bind)
            .fetch_one(&self.pg)
            .await
            .expect("count")
    }

    pub(crate) async fn total(&self, table: &'static str) -> i64 {
        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(&self.pg)
            .await
            .expect("total")
    }

    pub(crate) async fn fact_ids(&self, subject: &str) -> Vec<BrainFactId> {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM brain_facts WHERE subject_id = $1 ORDER BY created_at, id",
        )
        .bind(subject)
        .fetch_all(&self.pg)
        .await
        .expect("fact ids")
        .into_iter()
        .map(|id| BrainFactId::try_new(id).expect("fact id"))
        .collect()
    }

    pub(crate) async fn fact(&self, id: &BrainFactId) -> BrainFact {
        self.repos
            .facts
            .find(id)
            .await
            .expect("find fact")
            .expect("fact exists")
    }

    pub(crate) async fn only_fact(&self, subject: &str) -> BrainFact {
        let ids = self.fact_ids(subject).await;
        assert_eq!(ids.len(), 1, "exactly one fact about {subject}: {ids:?}");
        self.fact(&ids[0]).await
    }

    pub(crate) fn consumer(&self) -> BrainConsumer {
        BrainConsumer::builder(
            Arc::clone(&self.repos),
            Arc::new(OutboxConsumer::new(self.pg.clone())),
            Arc::clone(&self.model) as _,
            self.pg.clone(),
        )
        .profile(self.profile.clone())
        .seams(self.seams.clone())
        .build()
    }

    pub(crate) fn recall(&self) -> BrainRecallService {
        let store = BrainRecallStore::new((*self.repos).clone(), self.profile.recall.stale);
        BrainRecallService::builder(
            Arc::new(store),
            Arc::clone(&self.access) as _,
            Arc::new(PlaneDeclaredSource::new((*self.repos).clone())),
            Arc::new(self.profile.clone()),
        )
        .build()
    }
}
