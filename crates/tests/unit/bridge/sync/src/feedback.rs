use chrono::Utc;
use systemprompt_bridge::feedback::credentials::Enrollment;
use systemprompt_bridge::feedback::outbox::{Delivery, Outbox, OutboxScope};
use systemprompt_bridge::feedback::sessions::native_session;
use systemprompt_bridge::feedback::{FeedbackError, readback};
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, ManagedResourceId, PublicationId,
    ResourceRevisionId, UserId,
};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptResponse, FileReadback, InstallationPlanFile,
    ReadbackStatus, ReceiptAcknowledgement,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

fn plan(host: EvaluatorClient) -> ConsumerInstallationPlan {
    let content = b"echo safe".to_vec();
    ConsumerInstallationPlan {
        publication_id: PublicationId::new("publication"),
        resource_id: ManagedResourceId::new("resource"),
        revision_id: ResourceRevisionId::new("revision"),
        generation: 1,
        bundle_digest: ContentDigest::of(b"bundle"),
        host,
        canonical_files: vec![FileReadback {
            revision_id: ResourceRevisionId::new("revision"),
            path: "scripts/run.sh".to_owned(),
            digest: ContentDigest::of(&content),
            bytes: content.len() as u64,
            executable: true,
            content_check: ReadbackStatus::Unavailable,
            mode_check: ReadbackStatus::Unavailable,
        }],
        runtime_files: vec![
            InstallationPlanFile {
                path: ".systemprompt-source/revision/scripts/run.sh".to_owned(),
                bytes: content.clone(),
                executable: true,
            },
            InstallationPlanFile {
                path: "scripts/run.sh".to_owned(),
                bytes: content,
                executable: true,
            },
            InstallationPlanFile {
                path: "SKILL.md".to_owned(),
                bytes: b"---\nname: test\n---\n# Test\n".to_vec(),
                executable: false,
            },
        ],
    }
}

fn scope(device: &str) -> OutboxScope {
    OutboxScope {
        gateway: "https://example.invalid".to_owned(),
        consumer_id: UserId::new("consumer"),
        device_id: DeviceId::try_new(device).expect("nonempty fixture device"),
    }
}

fn prepared(
    host: EvaluatorClient,
) -> (
    tempfile::TempDir,
    systemprompt_models::feedback::receipts::ConsumerReceiptRequest,
) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("SKILL.md"), b"old").unwrap();
    let plan = plan(host);
    readback::materialize(dir.path(), &plan).unwrap();
    let receipt = readback::verify(
        dir.path(),
        &plan,
        ConsumerInstallationId::new("installation"),
    )
    .unwrap();
    (dir, receipt)
}

fn publication() -> systemprompt_models::bridge::manifest::SkillPublication {
    let plan = plan(EvaluatorClient::Codex);
    systemprompt_models::bridge::manifest::SkillPublication {
        publication_id: plan.publication_id,
        resource_id: plan.resource_id,
        revision_id: plan.revision_id,
        generation: plan.generation,
        bundle_digest: systemprompt_bridge::ids::Sha256Digest::try_new(plan.bundle_digest.as_str())
            .unwrap(),
    }
}


mod opencode_session;
mod pending;
mod readback_outbox;
mod self_enrol;
mod sessions;
mod transport;

mod session_compaction;
