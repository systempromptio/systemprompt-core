//! The elevated policy writer: how a Claude Desktop machine policy is
//! rewritten on Windows without a UAC prompt per change.
//!
//! Claude Desktop reads its connector list from `HKLM\SOFTWARE\Policies\
//! Claude`, which only an elevated process may write. The bridge's sync runs
//! as the user and never self-elevates, so until now every change to the
//! servers a user was entitled to — a connector linked, a group joined —
//! was a new administrator prompt, and until it was approved Cowork ran on
//! the old list. The elevated install registers a Task Scheduler task that
//! runs this binary's admin-owned copy as SYSTEM; a later sync drops a
//! request in a spool, runs the task, and reads the result back.
//!
//! What makes that safe is what the writer will and will not believe. A
//! request carries the gateway-signed manifest envelope verbatim; the writer
//! verifies the signature against the trust anchor the elevated install
//! pinned in the machine hive (`manifestTrust`, which no user can write) and
//! derives the server list from the manifest it verified, never from the
//! request's say-so. The request's own fields are the loopback port and host
//! token, the inference headers, the organisation uuid and the tool names —
//! each already the user's to choose, since the proxy the policy points at
//! is their own process. The binary the task runs lives under `ProgramData`
//! with an administrator-only DACL, so a user cannot swap it; the spool
//! inbox lets a user add a request and read only their own; results are
//! written for their requester alone. No trust anchor, no task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
mod child;
#[cfg(target_os = "windows")]
mod spool;
#[cfg(target_os = "windows")]
mod task;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use systemprompt_models::bridge::manifest::{SignedManifest, SignedManifestEnvelope};
use uuid::Uuid;

use super::elevated_protocol::CompletedStep;
use super::mdm::MdmError;
use super::mdm::tool_catalog::ToolCatalog;
use crate::ids::HostToken;

#[cfg(target_os = "windows")]
pub(crate) use self::child::perform_task;
#[cfg(target_os = "windows")]
pub(crate) use self::spool::{WriterStatus, install, remove, status, write_policy};

pub const REQUEST_VERSION: u32 = 1;
pub const MAX_REQUEST_BYTES: u64 = 256 * 1024;
pub const RESULT_TIMEOUT_SECS: u64 = 60;

/// One request to rewrite the machine policy.
///
/// The manifest is the envelope the gateway signed; the writer verifies it
/// and takes the server list from it, so nothing here can add a server the
/// gateway did not grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyWriteRequest {
    pub version: u32,
    pub job_id: Uuid,
    pub requester_sid: String,
    pub gateway: String,
    pub envelope: SignedManifestEnvelope,
    pub loopback_port: u16,
    pub host_token: HostToken,
    pub headers: BTreeMap<String, String>,
    pub models: Option<String>,
    pub org_uuid: Option<String>,
    pub tool_catalog: ToolCatalog,
}

/// The inference-side facts a request carries beside the manifest.
///
/// The custom headers, the model list (`None` keeps what the hive holds)
/// and the organisation uuid. A sync leaves the models alone; a generate
/// from the GUI carries the list it just fetched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestFacts {
    pub headers: BTreeMap<String, String>,
    pub models: Option<String>,
    pub org_uuid: Option<String>,
}

/// Where the writer's pieces live under a machine-wide root
/// (`%ProgramData%\<brand>`): the admin-owned binary copy, the request inbox
/// users may add to, and the outbox results are written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub root: PathBuf,
    pub bin: PathBuf,
    pub binary: PathBuf,
    pub inbox: PathBuf,
    pub outbox: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn under(program_data: &Path) -> Self {
        let brand = crate::brand::brand();
        let root = program_data.join(brand.working_dir_name).join("policy-writer");
        let bin = root.join("bin");
        Self {
            binary: bin.join(format!("{}.exe", brand.binary_name)),
            bin,
            inbox: root.join("inbox"),
            outbox: root.join("outbox"),
            root,
        }
    }

    #[must_use]
    pub fn request_path(&self, job_id: Uuid) -> PathBuf {
        self.inbox.join(format!("request-{job_id}.json"))
    }

    #[must_use]
    pub fn result_path(&self, job_id: Uuid) -> PathBuf {
        self.outbox.join(format!("result-{job_id}.json"))
    }
}

// Why: SYSTEM and Administrators own everything; users may traverse and
// list, and in the inbox add a file that is then theirs alone (CREATOR
// OWNER). No user reads another user's request, and no user can replace the
// binary the task runs.
pub const BIN_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;AU)";
pub const INBOX_SDDL: &str =
    "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;0x100007;;;AU)(A;OIIO;FA;;;CO)";
pub const OUTBOX_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;0x100005;;;AU)";

// Why: a task registered by an administrator runs for nobody else unless
// its own descriptor says so; read + execute for authenticated users is
// exactly "may run it, may not change what it runs".
pub const TASK_SDDL: &str = "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;AU)";

#[must_use]
pub fn task_name() -> String {
    format!("{}PolicyWriter", crate::brand::brand().schedule_task_name.trim_end_matches("Sync"))
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{binary}/{spool}/{app} are template placeholders consumed by str::replace, not fmt args"
)]
#[must_use]
pub fn render_task_xml(binary: &Path, spool_root: &Path) -> String {
    TASK_XML_TMPL
        .replace("{binary}", &binary.display().to_string())
        .replace("{spool}", &spool_root.display().to_string())
        .replace("{app}", crate::brand::brand().app_name)
}

const TASK_XML_TMPL: &str =
    include_str!("../../schedule/templates/task-scheduler.policy-writer.xml.tmpl");

#[must_use]
pub fn expected_steps() -> Vec<CompletedStep> {
    vec![CompletedStep {
        operation: "policy".to_owned(),
        target: crate::cowork_compat::HKLM_POLICY_KEY.to_owned(),
        policies: Vec::new(),
    }]
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyWriterError {
    #[error("policy write request protocol {actual} is unsupported; expected {REQUEST_VERSION}")]
    Version { actual: u32 },
    #[error("no signing trust anchor in the machine policy; the writer refuses an unverifiable manifest")]
    NoAnchor,
    #[error("the request names gateway {requested} but the machine anchor is pinned for {anchored}")]
    GatewayMismatch { requested: String, anchored: String },
    #[error("manifest signature: {0}")]
    Signature(#[from] crate::gateway::manifest::ManifestError),
    #[error("trust anchor: {0}")]
    Anchor(#[from] crate::config::TrustError),
    #[error("policy: {0}")]
    Policy(#[from] MdmError),
    #[error("policy store: {0}")]
    Store(#[from] crate::config::store::ConfigStoreError),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("the policy writer task is not registered on this computer")]
    NotRegistered,
    #[error("the policy writer task is registered but {0}")]
    Unavailable(String),
    #[error("the policy writer did not answer within {RESULT_TIMEOUT_SECS}s")]
    Timeout,
    #[error("policy writer result: {0}")]
    Protocol(#[from] super::elevated_protocol::ProtocolError),
}

// Why: pure, so the unelevated bridge derives the same values the writer
// does and can compare the hive against them after the writer reports
// success — the report alone is never taken as the write.
pub fn derive_policy(
    request: &PolicyWriteRequest,
    manifest: &SignedManifest,
    existing_models: Option<String>,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    let loopback = crate::proxy::LoopbackEndpoint::new(request.loopback_port, None);
    let registry = crate::mcp_registry::from_servers(&manifest.managed_mcp_servers);
    let servers =
        super::mdm::policy::mcp_entries_with(&loopback, &registry, &request.tool_catalog);
    let policy = super::mdm::policy::claude_desktop_policy(&super::mdm::policy::PolicyInputs {
        base_url: &loopback.origin(),
        host_token: &request.host_token,
        models: request.models.clone().or(existing_models),
        headers: &request.headers,
        egress_allowed_hosts: None,
        org_uuid: request.org_uuid.as_deref(),
        mcp_servers: servers.as_deref(),
    })?;
    Ok(super::mdm::policy::reg_values(&policy))
}

// Why: the anchor's gateway must be the request's and the signature must
// verify with the anchor's key before anything in the request is believed.
pub fn verify_against_anchor(
    request: &PolicyWriteRequest,
    anchor: &crate::config::TrustRecord,
) -> Result<SignedManifest, PolicyWriterError> {
    let requested = systemprompt_identifiers::ValidatedUrl::try_new(&request.gateway)
        .map_err(|e| PolicyWriterError::Anchor(crate::config::TrustError::InvalidPolicy(e.to_string())))?;
    let requested = crate::config::GatewayIdentity::new(&requested)?;
    if requested != anchor.gateway {
        return Err(PolicyWriterError::GatewayMismatch {
            requested: requested.to_string(),
            anchored: anchor.gateway.to_string(),
        });
    }
    crate::gateway::manifest::verify_envelope(&request.envelope, anchor.key.as_str())?;
    Ok(crate::gateway::manifest::decode_payload(&request.envelope)?)
}

// Why: the request is assembled from what the bridge already holds — the
// envelope the last sync verified, the loopback it serves on, the tool names
// it probed — never from a fresh fetch, so a writer request is exactly what
// an elevated sync would have written itself.
#[must_use]
pub fn build_request(
    loopback: Loopback,
    fragment: &crate::mcp_registry::EnvelopeFragment,
    tool_catalog: ToolCatalog,
    facts: RequestFacts,
    requester_sid: String,
) -> PolicyWriteRequest {
    PolicyWriteRequest {
        version: REQUEST_VERSION,
        job_id: Uuid::new_v4(),
        requester_sid,
        gateway: fragment.gateway.as_str().to_owned(),
        envelope: fragment.envelope.clone(),
        loopback_port: loopback.port,
        host_token: loopback.host_token,
        headers: facts.headers,
        models: facts.models,
        org_uuid: facts.org_uuid,
        tool_catalog,
    }
}

/// The proxy a policy points Claude Desktop at: its port and the token
/// scoped to the desktop host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loopback {
    pub port: u16,
    pub host_token: HostToken,
}

impl Loopback {
    pub fn of(endpoint: &crate::proxy::LoopbackEndpoint) -> std::io::Result<Self> {
        let secret = endpoint.secret_or_mint()?;
        Ok(Self {
            port: endpoint.port(),
            host_token: super::mdm::policy::desktop_host_token(&secret),
        })
    }

    // Why: a staged `.reg` profile already names the proxy and carries the
    // host token the GUI derived; the writer request repeats them rather
    // than re-deriving from a secret the install path does not hold.
    pub fn from_entries(entries: &[(String, String)]) -> std::io::Result<Self> {
        let value = |name: &str| entry_value(entries, name);
        let base_url = value("inferenceGatewayBaseUrl")
            .ok_or_else(|| std::io::Error::other("staged profile has no inferenceGatewayBaseUrl"))?;
        let port = url::Url::parse(&base_url)
            .ok()
            .and_then(|u| u.port())
            .ok_or_else(|| {
                std::io::Error::other(format!("staged profile base url has no port: {base_url}"))
            })?;
        let token = value(crate::cowork_compat::POLICY_API_KEY)
            .ok_or_else(|| std::io::Error::other("staged profile has no inferenceGatewayApiKey"))?;
        Ok(Self {
            port,
            host_token: HostToken::new(token),
        })
    }
}

fn entry_value(entries: &[(String, String)], name: &str) -> Option<String> {
    entries
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

// Why: a staged `.reg` profile is what the GUI generated for this user; the
// facts the writer needs from it are the three inference values the
// manifest does not carry. Everything else it derives itself.
#[must_use]
pub fn facts_from_entries(entries: &[(String, String)]) -> RequestFacts {
    RequestFacts {
        headers: entry_value(entries, "inferenceCustomHeaders")
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default(),
        models: entry_value(entries, "inferenceModels"),
        org_uuid: entry_value(entries, "deploymentOrganizationUuid"),
    }
}
