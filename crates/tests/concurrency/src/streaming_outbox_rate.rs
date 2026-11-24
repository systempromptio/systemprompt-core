//! A measurement, not a gate: the cost of the agent token-streaming path's
//! durable half when every text delta is its own outbox row. Each simulated
//! stream appends one row per delta in its own transaction (the shape
//! `event_outbox` insert + `NOTIFY` takes per AG-UI delta today) and commits;
//! the run reports deltas/s per stream, total commits/s (one `NOTIFY` per
//! commit) and p50/p99 commit latency at 1, 10 and 50 concurrent streams.
//! Ignored by default; run with
//! `cargo nextest run -p systemprompt-concurrency-tests --run-ignored only
//! streaming_outbox_rate`.

use std::time::{Duration, Instant};

use systemprompt_events::services::durable::{DurableOutbox, ReportingFact, SseEvent};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::{test_pg_pool, unique_instance};

const DELTAS_PER_STREAM: usize = 400;
const DELTA_TEXT: &str = "a typical streamed token run of about forty b";

struct Run {
    streams: usize,
    elapsed: Duration,
    latencies: Vec<Duration>,
}

impl Run {
    fn report(&self) -> String {
        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();
        let at = |q: f64| {
            let index = ((sorted.len() as f64 - 1.0) * q).round() as usize;
            sorted[index]
        };
        let commits = sorted.len() as f64;
        let secs = self.elapsed.as_secs_f64();
        format!(
            "streams={} commits={} commits_per_s={:.0} deltas_per_s_per_stream={:.0} \
             p50_ms={:.2} p99_ms={:.2}",
            self.streams,
            sorted.len(),
            commits / secs,
            commits / secs / self.streams as f64,
            at(0.50).as_secs_f64() * 1000.0,
            at(0.99).as_secs_f64() * 1000.0,
        )
    }
}

async fn run(outbox: &DurableOutbox, pool: &sqlx::PgPool, streams: usize) -> Run {
    let started = Instant::now();
    let mut tasks = Vec::with_capacity(streams);
    for stream in 0..streams {
        let outbox = outbox.clone();
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let actor = Actor::user(UserId::new(format!("r1-stream-{stream}")));
            let mut latencies = Vec::with_capacity(DELTAS_PER_STREAM);
            for delta in 0..DELTAS_PER_STREAM {
                let fact = ReportingFact {
                    consumer: "r1-measurement".to_owned(),
                    kind: "text_delta".to_owned(),
                    version: 1,
                    data: serde_json::json!({ "delta": DELTA_TEXT, "seq": delta }),
                };
                let begun = Instant::now();
                let mut tx = pool.begin().await.expect("begin");
                outbox
                    .append(&mut tx, &actor, SseEvent::FactOnly, &fact)
                    .await
                    .expect("append");
                tx.commit().await.expect("commit");
                latencies.push(begun.elapsed());
            }
            latencies
        }));
    }
    let mut latencies = Vec::with_capacity(streams * DELTAS_PER_STREAM);
    for task in tasks {
        latencies.extend(task.await.expect("stream task"));
    }
    Run {
        streams,
        elapsed: started.elapsed(),
        latencies,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "measurement of the streaming outbox path; run on demand"]
async fn streaming_outbox_rate() {
    let pool = test_pg_pool().await;
    let outbox = DurableOutbox::new(pool.clone(), unique_instance());
    for streams in [1, 10, 50] {
        let measured = run(&outbox, &pool, streams).await;
        println!("streaming_outbox_rate {}", measured.report());
    }
}
