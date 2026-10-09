use std::time::Duration;

use sqlx::postgres::{PgListener, PgPoolOptions};
use systemprompt_events::services::durable::{
    DurableOutbox, OutboxConsumer, ReportingFact, SseEvent,
};
use systemprompt_events::{
    ANALYTICS_BROADCASTER, Broadcaster, EventRouter, OUTBOX_CHANNEL, PostgresEventBridge, ToSse,
};
use systemprompt_identifiers::{Actor, ConnectionId, UserId};
use systemprompt_models::AnalyticsEventBuilder;
use systemprompt_test_fixtures::unique_instance;

#[tokio::test]
async fn transactional_delivery_preserves_sse_and_recovers_processing() {
    let admin = crate::setup_test_pool().await;
    verify_upgrade(&admin).await;
    let suffix = ConnectionId::generate().to_string().replace('-', "_");
    let schema = format!("outbox_test_{suffix}");
    assert!(
        schema
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA \"{schema}\"")))
        .execute(admin.as_ref())
        .await
        .unwrap();
    let search_path = schema.clone();
    let application_name = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _| {
            let search_path = search_path.clone();
            let application_name = application_name.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(search_path)
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SELECT set_config('application_name', $1, false)")
                    .bind(application_name)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&systemprompt_test_fixtures::test_database_url())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../infra/events/schema/event_outbox.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../infra/events/schema/migrations/004_durable_consumption.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TABLE projection (id TEXT PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let instance = unique_instance();
    let outbox = DurableOutbox::new(pool.clone(), instance.clone());
    let consumer = OutboxConsumer::new(pool.clone());
    let user = UserId::new(format!("user_{suffix}"));
    let actor = Actor::user(user.clone());
    let event = AnalyticsEventBuilder::heartbeat();
    let fact = ReportingFact {
        consumer: "analytics".into(),
        kind: "test.recorded".into(),
        version: 1,
        data: 42_i64,
    };
    let mut listener = PgListener::connect_with(&pool).await.unwrap();
    listener.listen(OUTBOX_CHANNEL).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let rolled_back = outbox
        .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
        .await
        .unwrap();
    assert!(consumer.claim("analytics").await.unwrap().is_none());
    tx.rollback().await.unwrap();
    assert!(consumer.claim("analytics").await.unwrap().is_none());
    assert!(
        !notified_within(
            &mut listener,
            Duration::from_millis(100),
            rolled_back.as_str()
        )
        .await,
        "a rolled-back append must not notify"
    );

    let connection = ConnectionId::generate();
    let (sender, mut rx) = tokio::sync::mpsc::channel(8);
    assert!(
        ANALYTICS_BROADCASTER
            .register(&user, &connection, sender)
            .await
    );
    let bridge = PostgresEventBridge::new(pool.clone(), instance);
    let router = bridge.router();
    let bridge = bridge.start();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), bridge.listening())
            .await
            .expect("bridge establishes its LISTEN within the bound"),
        "bridge stopped before listening"
    );
    assert_eq!(
        router
            .route_analytics(&user, event.clone())
            .await
            .into_local_logged(),
        1
    );
    let legacy = rx.recv().await.unwrap().unwrap();
    assert_eq!(
        format!("{legacy:?}"),
        format!("{:?}", event.to_sse().unwrap())
    );
    own_notification(&mut listener, &pool).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err()
    );
    assert!(consumer.claim("analytics").await.unwrap().is_none());
    assert_eq!(
        outbox
            .prune_processed_before(chrono::Utc::now())
            .await
            .unwrap(),
        1
    );

    // A row from a different instance still reaches the existing subscriber.
    let remote_id = systemprompt_identifiers::EventOutboxId::generate();
    sqlx::query("INSERT INTO event_outbox (id,channel,user_id,payload,actor_kind,actor_id,origin_instance_id) VALUES ($1,'analytics',$2,$3,'user',$2,'remote')")
        .bind(remote_id.as_str()).bind(user.as_str()).bind(sqlx::types::Json(&event))
        .execute(&pool).await.unwrap();
    sqlx::query("SELECT pg_notify($1,$2)")
        .bind(OUTBOX_CHANNEL)
        .bind(remote_id.as_str())
        .execute(&pool)
        .await
        .unwrap();
    let remote = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        format!("{remote:?}"),
        format!("{:?}", event.to_sse().unwrap())
    );
    assert!(
        notified_within(&mut listener, Duration::from_secs(2), remote_id.as_str()).await,
        "the remote row's notification reaches the listener"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err()
    );
    assert_eq!(
        outbox
            .prune_processed_before(chrono::Utc::now())
            .await
            .unwrap(),
        1
    );
    let mut tx = pool.begin().await.unwrap();
    let id = outbox
        .append(&mut tx, &actor, SseEvent::Analytics(&event), &fact)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_ne!(id, rolled_back);
    assert_eq!(own_notification(&mut listener, &pool).await, id.as_str());
    let delivered = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        format!("{:?}", delivered.unwrap()),
        format!("{:?}", event.to_sse().unwrap())
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err()
    );

    assert_eq!(
        outbox
            .prune_processed_before(chrono::Utc::now())
            .await
            .unwrap(),
        0
    );
    let mut delivery = consumer.claim("analytics").await.unwrap().unwrap();
    assert_eq!(delivery.fact::<i64>().unwrap().data, 42);
    assert!(consumer.claim("analytics").await.unwrap().is_none());
    sqlx::query("INSERT INTO projection VALUES ($1)")
        .bind(id.as_str())
        .execute(delivery.connection())
        .await
        .unwrap();
    delivery.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let mut delivery = consumer.claim("analytics").await.unwrap().unwrap();
    sqlx::query("INSERT INTO projection VALUES ($1)")
        .bind(id.as_str())
        .execute(delivery.connection())
        .await
        .unwrap();
    delivery.acknowledge().await.unwrap();
    assert!(consumer.claim("analytics").await.unwrap().is_none());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err()
    );
    assert_eq!(
        outbox
            .prune_processed_before(chrono::Utc::now())
            .await
            .unwrap(),
        1
    );

    // The later insert commits first; neither claim order nor acknowledgement
    // may hide the earlier insert when it eventually commits.
    let mut early = pool.begin().await.unwrap();
    let early_id = outbox
        .append(&mut early, &actor, SseEvent::Analytics(&event), &fact)
        .await
        .unwrap();
    let mut late = pool.begin().await.unwrap();
    let late_id = outbox
        .append(&mut late, &actor, SseEvent::Analytics(&event), &fact)
        .await
        .unwrap();
    late.commit().await.unwrap();
    let delivery = consumer.claim("analytics").await.unwrap().unwrap();
    assert_eq!(delivery.id(), late_id);
    delivery.acknowledge().await.unwrap();
    early.commit().await.unwrap();
    let delivery = consumer.claim("analytics").await.unwrap().unwrap();
    assert_eq!(delivery.id(), early_id);
    delivery.acknowledge().await.unwrap();

    drop(listener);
    verify_all_channels(&pool, &outbox, &router).await;
    bridge.shutdown().await;
    ANALYTICS_BROADCASTER.unregister(&user, &connection).await;
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP SCHEMA \"{schema}\" CASCADE"
    )))
    .execute(admin.as_ref())
    .await
    .unwrap();
}

// Why: NOTIFY channels are database-wide, so other test processes sharing the
// database publish on the same channel; only payloads naming a row in this
// test's schema belong to it.
async fn own_notification(listener: &mut PgListener, pool: &sqlx::PgPool) -> String {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let notification = listener.recv().await.unwrap();
            let ours: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM event_outbox WHERE id = $1)")
                    .bind(notification.payload())
                    .fetch_one(pool)
                    .await
                    .unwrap();
            if ours {
                return notification.payload().to_owned();
            }
        }
    })
    .await
    .expect("this test's notification arrives within 2s")
}

async fn notified_within(listener: &mut PgListener, window: Duration, id: &str) -> bool {
    tokio::time::timeout(window, async {
        loop {
            if listener.recv().await.unwrap().payload() == id {
                return;
            }
        }
    })
    .await
    .is_ok()
}

async fn verify_upgrade(pool: &sqlx::PgPool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(
        "CREATE TEMP TABLE event_outbox (
            id TEXT PRIMARY KEY, channel TEXT NOT NULL, user_id TEXT NOT NULL,
            payload JSONB NOT NULL, actor_kind TEXT NOT NULL, actor_id TEXT NOT NULL,
            origin_instance_id TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        ) ON COMMIT DROP;
        INSERT INTO event_outbox (id,channel,user_id,payload,actor_kind,actor_id,origin_instance_id)
        VALUES ('legacy','analytics','user','{}','user','user','origin');",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    for _ in 0..2 {
        sqlx::raw_sql(include_str!(
            "../../../../infra/events/schema/migrations/004_durable_consumption.sql"
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    let preserved: bool = sqlx::query_scalar(
        "SELECT consumer IS NULL AND fact IS NULL AND processed_at IS NULL
         AND NOT deliver_to_origin AND payload = '{}'::jsonb
         AND origin_instance_id = 'origin' FROM event_outbox WHERE id = 'legacy'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert!(preserved);
    tx.rollback().await.unwrap();
}

async fn verify_all_channels(pool: &sqlx::PgPool, outbox: &DurableOutbox, router: &EventRouter) {
    use systemprompt_events::{A2A_BROADCASTER, AGUI_BROADCASTER, CONTEXT_BROADCASTER};
    use systemprompt_identifiers::{ContextId, TaskId};
    use systemprompt_models::a2a::TaskState;
    use systemprompt_models::{A2AEventBuilder, AgUiEventBuilder, SystemEventBuilder};

    let actor = Actor::user(crate::unique_user_id("all-channels"));
    let connection = ConnectionId::generate();
    let (sender, mut rx) = tokio::sync::mpsc::channel(16);
    assert!(
        A2A_BROADCASTER
            .register(&actor.user_id, &connection, sender.clone())
            .await
    );
    assert!(
        AGUI_BROADCASTER
            .register(&actor.user_id, &connection, sender.clone())
            .await
    );
    assert!(
        ANALYTICS_BROADCASTER
            .register(&actor.user_id, &connection, sender.clone())
            .await
    );
    let (context_sender, mut context_rx) = tokio::sync::mpsc::channel(16);
    assert!(
        CONTEXT_BROADCASTER
            .register(&actor.user_id, &connection, context_sender)
            .await
    );
    let agui = AgUiEventBuilder::run_started(ContextId::generate(), TaskId::generate(), None);
    let a2a = A2AEventBuilder::task_status_update(
        TaskId::generate(),
        ContextId::generate(),
        TaskState::Working,
        None,
    );
    let system = SystemEventBuilder::heartbeat();
    let analytics = AnalyticsEventBuilder::heartbeat();
    let fact = ReportingFact {
        consumer: "channels".into(),
        kind: "test".into(),
        version: 1,
        data: (),
    };

    for (event, expected) in [
        (SseEvent::AgUi(&agui), agui.to_sse().unwrap()),
        (SseEvent::A2A(&a2a), a2a.to_sse().unwrap()),
        (SseEvent::Analytics(&analytics), analytics.to_sse().unwrap()),
    ] {
        match &event {
            SseEvent::AgUi(value) => {
                router
                    .route_agui(&actor.user_id, (*value).clone())
                    .await
                    .into_local_logged();
            },
            SseEvent::A2A(value) => {
                router
                    .route_a2a(&actor.user_id, (*value).clone())
                    .await
                    .into_local_logged();
            },
            SseEvent::Analytics(value) => {
                router
                    .route_analytics(&actor.user_id, (*value).clone())
                    .await
                    .into_local_logged();
            },
            SseEvent::System(_) => unreachable!(),
        }
        let legacy = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(format!("{legacy:?}"), format!("{expected:?}"));
        let mut tx = pool.begin().await.unwrap();
        outbox.append(&mut tx, &actor, event, &fact).await.unwrap();
        tx.commit().await.unwrap();
        let durable = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(format!("{durable:?}"), format!("{expected:?}"));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .is_err()
        );
    }
    // AG-UI and A2A each fan into the context stream once per publication.
    let mut context_events = Vec::new();
    for _ in 0..4 {
        let event = tokio::time::timeout(Duration::from_secs(2), context_rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        context_events.push(format!("{event:?}"));
    }
    assert_eq!(context_events[0], context_events[1]);
    assert_eq!(context_events[2], context_events[3]);
    assert!(context_rx.try_recv().is_err());
    router
        .route_system(&actor.user_id, system.clone())
        .await
        .into_local_logged();
    let expected = context_rx.recv().await.unwrap().unwrap();
    let mut tx = pool.begin().await.unwrap();
    outbox
        .append(&mut tx, &actor, SseEvent::System(&system), &fact)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let delivered = tokio::time::timeout(Duration::from_secs(2), context_rx.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(format!("{delivered:?}"), format!("{expected:?}"));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), context_rx.recv())
            .await
            .is_err()
    );
    A2A_BROADCASTER
        .unregister(&actor.user_id, &connection)
        .await;
    AGUI_BROADCASTER
        .unregister(&actor.user_id, &connection)
        .await;
    ANALYTICS_BROADCASTER
        .unregister(&actor.user_id, &connection)
        .await;
    CONTEXT_BROADCASTER
        .unregister(&actor.user_id, &connection)
        .await;
}
