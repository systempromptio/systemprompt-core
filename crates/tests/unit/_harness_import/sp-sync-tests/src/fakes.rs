use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sp_client::{
    ClientError, CredentialError, CredentialSource, Freshness, ManifestFetch, ManifestKey,
};
use sp_protocol::names::sha256_hex;
use sp_protocol::wire::consumer::{
    Check, InstallationPlan, ReceiptRequest, RuntimeReadback, SessionBindingRequest,
};
use sp_protocol::wire::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use sp_protocol::wire::manifest::{
    Manifest, ManifestSignature, Publication, Sha256Digest, SignedManifestEnvelope,
};
use sp_protocol::{HOST_ID, Jwt};

use sp_sync::cadence::{Clock, Jitter};

pub(crate) const BASE: &str = "https://sp.example.com";
pub(crate) const NOW: i64 = 1_900_000_000;

pub(crate) fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(NOW + seconds, 0).unwrap()
}

pub(crate) fn version(seconds: i64, suffix: &str) -> String {
    format!("{}-{suffix}", at(seconds).format("%Y-%m-%dT%H:%M:%SZ"))
}

pub(crate) fn manifest(patch: Value) -> Manifest {
    let mut base = json!({
        "user_id": "u-1",
        "manifest_version": version(0, "0000000000000001"),
        "not_before": at(0).to_rfc3339(),
        "enabled_hosts": [HOST_ID],
        "managed_mcp_servers": [],
        "plugins": [],
        "skills": [{ "id": "alpha", "name": "alpha", "description": "A skill.", "instructions": "# Alpha\n" }],
    });
    if let (Value::Object(base), Value::Object(patch)) = (&mut base, patch) {
        base.extend(patch);
    }
    serde_json::from_value(base).unwrap()
}

pub(crate) fn fetched(manifest: Manifest, etag: &str) -> ManifestFetch {
    let envelope = SignedManifestEnvelope {
        payload: String::new(),
        signature: ManifestSignature::new("sig"),
    };
    ManifestFetch::Fetched {
        envelope,
        manifest: Box::new(manifest),
        etag: Some(etag.to_owned()),
    }
}

pub(crate) fn unreachable() -> ClientError {
    ClientError::Unreachable {
        endpoint: "GET /x",
        cause: sp_client::TransportFailure::Status { status: 503 },
    }
}

pub(crate) fn status_error(status: u16) -> ClientError {
    if status == 401 {
        return ClientError::Unauthorized {
            endpoint: "POST /x",
            error_key: None,
        };
    }
    ClientError::Protocol {
        endpoint: "POST /x",
        status: Some(status),
        what: "refused".to_owned(),
        detail: None,
    }
}

pub(crate) fn key() -> ManifestKey {
    ManifestKey::from_raw([7u8; 32])
}

pub(crate) fn publication(generation: i64) -> Publication {
    serde_json::from_value(json!({
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79",
        "generation": generation,
        "bundle_digest": sha256_hex(b"bundle"),
    }))
    .unwrap()
}

pub(crate) fn plan_for(publication: &Publication) -> InstallationPlan {
    let skill = b"# Alpha notes\n".to_vec();
    let revision = publication.revision_id.as_str();
    serde_json::from_value(json!({
        "publication_id": publication.publication_id,
        "resource_id": publication.resource_id,
        "revision_id": publication.revision_id,
        "generation": publication.generation,
        "bundle_digest": publication.bundle_digest,
        "host": HOST_ID,
        "canonical_files": [{
            "revision_id": revision, "path": "SKILL.md", "digest": sha256_hex(&skill), "bytes": skill.len(),
            "executable": false, "content_check": "unavailable", "mode_check": "unavailable"
        }],
        "runtime_files": [
            { "path": format!(".systemprompt-source/{revision}/SKILL.md"), "bytes": skill, "executable": false },
            { "path": "SKILL.md", "bytes": [35, 10], "executable": false },
            { "path": "references/a.md", "bytes": [65], "executable": false }
        ]
    }))
    .unwrap()
}

pub(crate) fn readback(plan: &InstallationPlan) -> Vec<RuntimeReadback> {
    plan.runtime_files
        .iter()
        .map(|file| RuntimeReadback {
            path: file.path.clone(),
            digest: Sha256Digest::try_new(sha256_hex(&file.bytes)).unwrap(),
            bytes: file.bytes.len() as u64,
            executable: file.executable,
            content_check: Check::Verified,
            mode_check: Check::Verified,
        })
        .collect()
}

#[derive(Debug)]
pub(crate) struct FakeClock(pub(crate) Mutex<DateTime<Utc>>);

impl FakeClock {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(at(0))))
    }

    pub(crate) fn advance(&self, seconds: i64) {
        let mut now = self.0.lock().unwrap();
        *now += chrono::TimeDelta::seconds(seconds);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

#[derive(Debug)]
pub(crate) struct FakeCredentials;

#[async_trait::async_trait]
impl CredentialSource for FakeCredentials {
    async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
        Ok(Jwt::new("header.payload.signature").unwrap())
    }

    async fn rejected(&self) {}
}

#[derive(Default)]
pub(crate) struct InstanceState {
    pub(crate) manifests: VecDeque<Result<ManifestFetch, ClientError>>,
    pub(crate) manifest_calls: Vec<(Option<String>, bool)>,
    pub(crate) served_key: Option<ManifestKey>,
    pub(crate) catalog_reads: usize,
    pub(crate) files: HashMap<String, Vec<u8>>,
    pub(crate) file_calls: usize,
    pub(crate) plan_calls: usize,
    pub(crate) plan_status: Option<u16>,
    pub(crate) enrolments: usize,
    pub(crate) enrol_fails: bool,
    pub(crate) receipts: Vec<ReceiptRequest>,
    pub(crate) receipt_status: Option<u16>,
    pub(crate) bindings: Vec<SessionBindingRequest>,
    pub(crate) binding_status: Option<u16>,
    pub(crate) heartbeats: Vec<HeartbeatRequest>,
    pub(crate) heartbeat_answers: VecDeque<Result<HeartbeatResponse, ClientError>>,
}

#[derive(Clone, Default)]
pub(crate) struct FakeInstance(pub(crate) Arc<Mutex<InstanceState>>);

impl FakeInstance {
    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, InstanceState> {
        self.0.lock().unwrap()
    }
}

pub(crate) struct Fixed(pub(crate) f64);

impl Jitter for Fixed {
    fn unit(&mut self) -> f64 {
        self.0
    }
}
