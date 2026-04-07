use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use systemprompt_events::{Broadcaster, GenericBroadcaster};
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_models::A2AEvent;
use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

fn test_event() -> A2AEvent {
    A2AEvent::TaskStatusUpdate {
        timestamp: chrono::Utc::now(),
        payload: TaskStatusUpdatePayload {
            task_id: TaskId::generate(),
            context_id: ContextId::generate(),
            state: systemprompt_models::a2a::TaskState::Working,
            message: None,
        },
    }
}

fn bench_broadcast(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("broadcast");

    for conn_count in [1, 10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("connections", conn_count),
            &conn_count,
            |b, &count| {
                let broadcaster: GenericBroadcaster<A2AEvent> = GenericBroadcaster::new();
                let user = UserId::new("bench-user");
                let mut receivers = Vec::new();

                rt.block_on(async {
                    for i in 0..count {
                        let (tx, rx) = mpsc::unbounded_channel();
                        broadcaster.register(&user, &format!("conn-{i}"), tx).await;
                        receivers.push(rx);
                    }
                });

                b.iter(|| {
                    rt.block_on(async {
                        broadcaster.broadcast(&user, test_event()).await
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_register_unregister(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("register_unregister", |b| {
        let broadcaster: GenericBroadcaster<A2AEvent> = GenericBroadcaster::new();
        let user = UserId::new("bench-user");

        b.iter(|| {
            rt.block_on(async {
                let (tx, _rx) = mpsc::unbounded_channel();
                broadcaster.register(&user, "bench-conn", tx).await;
                broadcaster.unregister(&user, "bench-conn").await;
            })
        });
    });
}

criterion_group!(benches, bench_broadcast, bench_register_unregister);
criterion_main!(benches);
