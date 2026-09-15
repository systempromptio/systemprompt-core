use super::*;
use systemprompt_analytics::snapshots::{
    FeedbackSnapshot, FeedbackSnapshotsRepository, LatencyHistogram, SnapshotRangeRequest,
};
use systemprompt_identifiers::{DeviceId, NativeSessionId};
use systemprompt_models::feedback::EvaluatorClient;

fn repository(f: &Fixture) -> FeedbackSnapshotsRepository {
    FeedbackSnapshotsRepository::new(
        f.pool.clone(),
        systemprompt_analytics::feedback::FeedbackFactsRepository::new(f.pool.clone()),
    )
}
fn request(id: &str, revision: u64, at: chrono::DateTime<Utc>, micros: u64) -> AnalyticsChange {
    let key = key(AnalyticsFactKind::Request, id);
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: key.clone(),
        revision,
        occurred_at: at,
        recorded_at: Utc::now(),
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Request(NormalizedRequestFact {
                request_key: key,
                occurred_at: at,
                consumer: InvocationConsumerIdentity::HistoricalUnknown,
                succeeded: false,
                spend: RecordedSpend::Known {
                    currency: "USD".to_owned(),
                    amount_micros: micros,
                },
                input_tokens: Some(12),
                output_tokens: Some(3),
                latency_micros: Some(micros),
            }),
        },
    }
}
fn identify(change: &mut AnalyticsChange, user: &str) {
    let identity = InvocationConsumerIdentity::Authenticated {
        consumer_id: UserId::new(user),
        device_id: DeviceId::try_new("device").expect("nonempty fixture device"),
        host: EvaluatorClient::ClaudeCode,
        session_id: NativeSessionId::new("session"),
    };
    match &mut change.operation {
        AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Request(value),
        } => value.consumer = identity,
        AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Invocation(value),
        } => value.consumer = identity,
        _ => panic!("identity-bearing fact"),
    }
}
async fn refresh(f: &Fixture, now: chrono::DateTime<Utc>) -> FeedbackSnapshot {
    f.drain().await;
    let repo = repository(f);
    while repo
        .process(&f.owner, &AnalyticsWorkerId::generate(), now)
        .await
        .expect("snapshot process")
        > 0
    {}
    repo.refresh(&f.owner, &[], now).await.expect("refresh");
    repo.snapshot(&f.owner, None, 30)
        .await
        .expect("read")
        .expect("ready")
}
fn association(
    id: &str,
    request_id: &str,
    resource: &ManagedResourceId,
    at: chrono::DateTime<Utc>,
) -> AnalyticsChange {
    let key = key(AnalyticsFactKind::ResourceAssociation, id);
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: key.clone(),
        revision: 1,
        occurred_at: at,
        recorded_at: Utc::now(),
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::ResourceAssociation(NormalizedResourceAssociationFact {
                association_key: key,
                invocation_key: super::key(AnalyticsFactKind::Invocation, "invocation"),
                request_key: super::key(AnalyticsFactKind::Request, request_id),
                occurred_at: at,
                attribution: InvocationResourceAttribution::Verified {
                    resource_id: resource.clone(),
                    revision_id: ResourceRevisionId::generate(),
                },
            }),
        },
    }
}

#[path = "snapshot_parity.rs"]
mod parity;
#[path = "snapshot_recovery.rs"]
mod recovery;
#[path = "snapshot_retention.rs"]
mod retention;
