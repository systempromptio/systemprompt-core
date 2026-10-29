use std::sync::{Arc, Mutex};

use bytes::Bytes;
use systemprompt_gateway::GatewayAudit;
use systemprompt_gateway::protocol::CanonicalContent;
use systemprompt_gateway::protocol::canonical::{CanonicalResponse, CanonicalUsage};
use systemprompt_gateway::quota::AccountingOutcome;
use systemprompt_identifiers::AiRequestId;
use tracing_subscriber::layer::SubscriberExt;

use crate::quota_accounting::{
    gateway_repos_in, private_journal_dir, request_ctx, reservation_for,
};
use crate::support::{minimal_request, seed_user, setup_db};

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_user_key_request_settles_tokens_at_zero_cost_and_logs_completion() {
    let logs = LogBuffer::default();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(logs.clone()),
    );
    let _guard = tracing::subscriber::set_default(subscriber);

    let db = setup_db().await;
    let owner = seed_user(&db).await;
    let id = AiRequestId::generate();
    let repos = gateway_repos_in(&db, &private_journal_dir());
    let audit = GatewayAudit::new(&repos, request_ctx(owner.clone(), id.clone()));
    audit
        .open(
            &minimal_request(None, "user key"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");
    audit.commit_admission().await.expect("commit admission");
    assert!(!audit.billed_to_user_key());
    audit.bill_to_user_key();
    assert!(audit.billed_to_user_key());
    audit.record_billing_source().await;

    let usage = CanonicalUsage {
        input_tokens: 4,
        output_tokens: 6,
        ..Default::default()
    };
    let response = CanonicalResponse {
        model: "claude-test".to_owned(),
        content: vec![CanonicalContent::text("ok".to_owned())],
        usage,
        raw_finish_reason: Some("end_turn".to_owned()),
        ..Default::default()
    };
    audit
        .pin_pricing(systemprompt_manifest::services::ModelPricing {
            input_per_million: 1.0,
            output_per_million: 2.0,
            ..Default::default()
        })
        .expect("pin pricing");
    audit
        .complete(
            usage,
            vec![],
            &response,
            &Bytes::from_static(b"{\"text\":\"ok\"}"),
        )
        .await
        .expect("complete");

    audit.set_quota_reservation(reservation_for(&owner));
    assert!(matches!(
        audit.settle_quota(&usage, 999).await,
        AccountingOutcome::Counted
    ));

    let pool = db.pool();
    let billing: String =
        sqlx::query_scalar("SELECT billing_source FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.as_ref())
            .await
            .expect("billing source");
    assert_eq!(billing, "user_key");
    let (input, output, cost): (i64, i64, i64) = sqlx::query_as(
        "SELECT input_tokens, output_tokens, cost_microdollars FROM ai_quota_buckets \
         WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(owner.as_str())
    .fetch_one(pool.as_ref())
    .await
    .expect("bucket");
    assert_eq!(
        (input, output, cost),
        (4, 6, 0),
        "tokens count, cost does not"
    );

    let text = String::from_utf8(logs.0.lock().expect("logs").clone()).expect("utf8");
    let line = text
        .lines()
        .find(|l| l.contains("Gateway audit: request completed"))
        .unwrap_or_else(|| panic!("completion line missing:\n{text}"));
    assert!(line.contains(&format!("ai_request_id={id}")), "{line}");
    assert!(
        line.contains("finish_reason=\"end_turn\"") || line.contains("finish_reason=end_turn"),
        "{line}"
    );
    assert!(line.contains("output_tokens=6"), "{line}");
}
