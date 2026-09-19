//! The elevated policy writer's trust and derivation, run without Windows:
//! what it refuses, what it derives, and what the task it registers looks
//! like. The Windows-only spool, task and child are exercised on a Windows
//! machine; everything they decide with is here.

use std::collections::BTreeMap;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey};
use systemprompt_bridge::config::{PinSource, TrustRecord};
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManagedMcpServer, SignedManifest, SignedManifestEnvelope,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{HostToken, ManifestSignature};
use systemprompt_bridge::install::policy_writer::{
    BIN_SDDL, INBOX_SDDL, Layout, Loopback, MAX_REQUEST_BYTES, OUTBOX_SDDL, PolicyWriteRequest,
    PolicyWriterError, REQUEST_VERSION, RequestFacts, TASK_SDDL, build_request, derive_policy,
    expected_steps, facts_from_entries, render_task_xml, task_name, verify_against_anchor,
};
use systemprompt_bridge::mcp_registry::EnvelopeFragment;
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::bridge::ids::{ManagedMcpServerName, ToolName, ToolPolicy};

const GATEWAY: &str = "https://gateway.example.test";

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn pubkey_b64(key: &SigningKey) -> String {
    B64.encode(key.verifying_key().to_bytes())
}

fn anchor(key: &SigningKey, gateway: &str) -> TrustRecord {
    TrustRecord::new(
        &ValidatedUrl::try_new(gateway).unwrap(),
        &pubkey_b64(key),
        PinSource::Policy,
    )
    .unwrap()
}

fn server(name: &str, tool_policy: Option<BTreeMap<ToolName, ToolPolicy>>) -> ManagedMcpServer {
    ManagedMcpServer {
        id: systemprompt_identifiers::McpServerId::try_new(name).unwrap(),
        name: ManagedMcpServerName::try_new(name).unwrap(),
        url: ValidatedUrl::try_new(format!("{GATEWAY}/api/v1/mcp/{name}/mcp")).unwrap(),
        transport: Some("http".into()),
        headers: None,
        oauth: None,
        tool_policy,
    }
}

fn wildcard(policy: ToolPolicy) -> Option<BTreeMap<ToolName, ToolPolicy>> {
    let mut map = BTreeMap::new();
    map.insert(
        ToolName::try_new(ManagedMcpServer::TOOL_POLICY_WILDCARD).unwrap(),
        policy,
    );
    Some(map)
}

fn manifest(servers: Vec<ManagedMcpServer>) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-09-19T08:00:00Z-cafecafe").unwrap(),
        issued_at: chrono::DateTime::parse_from_rfc3339("2026-09-19T08:00:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc),
        not_before: chrono::DateTime::parse_from_rfc3339("2026-09-19T08:00:00+00:00")
            .unwrap()
            .with_timezone(&chrono::Utc),
        user_id: systemprompt_identifiers::UserId::new("user_writer_test"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        rules: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: servers,
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: BTreeMap::default(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        auto_update: Default::default(),
        diagnostics: Vec::new(),
        marketplaces: Vec::new(),
    }
}

fn envelope(key: &SigningKey, manifest: &SignedManifest) -> SignedManifestEnvelope {
    let payload = serde_json::to_string(manifest).unwrap();
    let sig = key.sign(payload.as_bytes());
    SignedManifestEnvelope {
        payload,
        signature: ManifestSignature::new(B64.encode(sig.to_bytes())),
    }
}

fn request(
    key: &SigningKey,
    manifest: &SignedManifest,
    catalog: &[(&str, &[&str])],
) -> PolicyWriteRequest {
    let fragment = EnvelopeFragment {
        gateway: ValidatedUrl::try_new(GATEWAY).unwrap(),
        envelope: envelope(key, manifest),
    };
    let tool_catalog = catalog
        .iter()
        .map(|(slug, tools)| {
            (
                (*slug).to_owned(),
                tools.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>(),
            )
        })
        .collect();
    build_request(
        Loopback {
            port: 48217,
            host_token: HostToken::new("host-token-for-desktop"),
        },
        &fragment,
        tool_catalog,
        RequestFacts::default(),
        "S-1-5-21-1-2-3-1001".to_owned(),
    )
}

fn value_of<'a>(values: &'a [(&'static str, &'static str, String)], name: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|(key, _, _)| *key == name)
        .map(|(_, _, value)| value.as_str())
}

#[test]
fn a_request_round_trips_and_refuses_fields_it_does_not_know() {
    let key = signing_key(1);
    let req = request(&key, &manifest(vec![]), &[]);
    assert_eq!(req.version, REQUEST_VERSION);
    let json = serde_json::to_string(&req).unwrap();
    let back: PolicyWriteRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.job_id, req.job_id);
    assert_eq!(back.gateway, GATEWAY);

    let mut loose: serde_json::Value = serde_json::from_str(&json).unwrap();
    loose["extra_servers"] = serde_json::json!(["smuggled"]);
    assert!(
        serde_json::from_value::<PolicyWriteRequest>(loose).is_err(),
        "a field the writer does not know is refused, not ignored"
    );
    assert!(json.len() < MAX_REQUEST_BYTES as usize);
}

// Why: this is the whole trust story. The writer runs as SYSTEM and writes
// the machine hive; the only thing that keeps a user from driving it is
// that the server list comes from a manifest the gateway signed and the
// machine anchor verifies.
#[test]
fn a_manifest_signed_by_the_anchored_gateway_verifies_and_nothing_else_does() {
    let gateway_key = signing_key(1);
    let manifest = manifest(vec![server("atlassian", wildcard(ToolPolicy::Allow))]);
    let req = request(&gateway_key, &manifest, &[]);

    let decoded = verify_against_anchor(&req, &anchor(&gateway_key, GATEWAY))
        .expect("the anchored key signed this envelope");
    assert_eq!(decoded.managed_mcp_servers.len(), 1);

    let other = signing_key(2);
    assert!(
        matches!(
            verify_against_anchor(&req, &anchor(&other, GATEWAY)),
            Err(PolicyWriterError::Signature(_))
        ),
        "a key that is not the anchor's must not verify"
    );

    let mut tampered = req.clone();
    tampered.envelope.payload = tampered.envelope.payload.replace("atlassian", "smuggled");
    assert!(
        matches!(
            verify_against_anchor(&tampered, &anchor(&gateway_key, GATEWAY)),
            Err(PolicyWriterError::Signature(_))
        ),
        "a payload edited after signing must not verify"
    );

    assert!(
        matches!(
            verify_against_anchor(&req, &anchor(&gateway_key, "https://other.example.test")),
            Err(PolicyWriterError::GatewayMismatch { .. })
        ),
        "an anchor pinned for another gateway must not vouch for this one"
    );
}

#[test]
fn the_server_list_comes_from_the_manifest_never_from_the_catalog() {
    let key = signing_key(1);
    let manifest = manifest(vec![
        server("atlassian", wildcard(ToolPolicy::Allow)),
        server("salesforce-crm-dev", wildcard(ToolPolicy::Allow)),
    ]);
    let req = request(
        &key,
        &manifest,
        &[
            ("atlassian", &["getJiraIssue"]),
            ("salesforce-crm-dev", &["soqlQuery"]),
            ("smuggled", &["anything"]),
        ],
    );
    let values = derive_policy(&req, &manifest, None).unwrap();
    let servers = value_of(&values, "managedMcpServers").expect("the list is written");
    let list: Vec<serde_json::Value> = serde_json::from_str(servers).unwrap();
    let names: Vec<&str> = list.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["atlassian", "salesforce-crm-dev"]);
    assert!(
        !servers.contains("smuggled"),
        "a catalog entry with no manifest server behind it is not a server"
    );
    assert_eq!(
        list[0]["url"].as_str().unwrap(),
        "http://127.0.0.1:48217/mcp/atlassian",
        "servers point at the requester's loopback proxy"
    );
    assert_eq!(list[0]["toolPolicy"]["getJiraIssue"], "allow");
    assert_eq!(
        value_of(&values, "inferenceGatewayApiKey"),
        Some("host-token-for-desktop")
    );
    assert_eq!(
        value_of(&values, "inferenceGatewayBaseUrl"),
        Some("http://127.0.0.1:48217")
    );
}

#[test]
fn a_wildcard_the_catalog_cannot_expand_withholds_the_whole_list() {
    let key = signing_key(1);
    let manifest = manifest(vec![server("atlassian", wildcard(ToolPolicy::Allow))]);
    let req = request(&key, &manifest, &[]);
    let values = derive_policy(&req, &manifest, None).unwrap();
    assert_eq!(
        value_of(&values, "managedMcpServers"),
        None,
        "a partial list is one Desktop would resolve to its own default"
    );
}

#[test]
fn a_server_the_manifest_denies_outright_is_left_out() {
    let key = signing_key(1);
    let manifest = manifest(vec![
        server("atlassian", wildcard(ToolPolicy::Allow)),
        server("blocked", wildcard(ToolPolicy::Deny)),
    ]);
    let req = request(
        &key,
        &manifest,
        &[("atlassian", &["x"]), ("blocked", &["y"])],
    );
    let values = derive_policy(&req, &manifest, None).unwrap();
    let servers = value_of(&values, "managedMcpServers").unwrap();
    assert!(servers.contains("atlassian"));
    assert!(!servers.contains("blocked"));
}

#[test]
fn the_model_list_is_the_requests_when_given_and_the_hives_otherwise() {
    let key = signing_key(1);
    let manifest = manifest(vec![]);
    let mut req = request(&key, &manifest, &[]);
    let existing = Some(r#"["claude-opus-4-5"]"#.to_owned());
    let kept = derive_policy(&req, &manifest, existing.clone()).unwrap();
    assert_eq!(
        value_of(&kept, "inferenceModels"),
        Some(r#"["claude-opus-4-5"]"#),
        "a sync leaves the hive's model list alone"
    );
    req.models = Some(r#"["claude-sonnet-5","claude-opus-5"]"#.to_owned());
    let replaced = derive_policy(&req, &manifest, existing).unwrap();
    assert_eq!(
        value_of(&replaced, "inferenceModels"),
        Some(r#"["claude-sonnet-5","claude-opus-5"]"#),
        "a generate from the GUI carries the list it fetched"
    );
}

#[test]
fn a_staged_profile_yields_the_loopback_and_the_inference_facts() {
    let entries = vec![
        ("inferenceProvider".to_owned(), "gateway".to_owned()),
        (
            "inferenceGatewayBaseUrl".to_owned(),
            "http://127.0.0.1:48217".to_owned(),
        ),
        ("inferenceGatewayApiKey".to_owned(), "tok".to_owned()),
        (
            "inferenceCustomHeaders".to_owned(),
            r#"{"x-inference-protocol":"anthropic"}"#.to_owned(),
        ),
        (
            "inferenceModels".to_owned(),
            r#"["claude-opus-5"]"#.to_owned(),
        ),
        (
            "deploymentOrganizationUuid".to_owned(),
            "11111111-2222-4333-8444-555555555555".to_owned(),
        ),
    ];
    let loopback = Loopback::from_entries(&entries).unwrap();
    assert_eq!(loopback.port, 48217);
    assert_eq!(loopback.host_token.as_str(), "tok");
    let facts = facts_from_entries(&entries);
    assert_eq!(
        facts
            .headers
            .get("x-inference-protocol")
            .map(String::as_str),
        Some("anthropic")
    );
    assert_eq!(facts.models.as_deref(), Some(r#"["claude-opus-5"]"#));
    assert_eq!(
        facts.org_uuid.as_deref(),
        Some("11111111-2222-4333-8444-555555555555")
    );
    assert!(
        Loopback::from_entries(&[("inferenceProvider".to_owned(), "gateway".to_owned())]).is_err(),
        "a profile that names no proxy cannot become a request"
    );
}

#[test]
fn the_task_runs_the_admin_owned_copy_as_system_with_no_triggers() {
    let program_data = Path::new(r"C:\ProgramData");
    let layout = Layout::under(program_data);
    assert_eq!(
        layout.root,
        program_data
            .join("systemprompt-bridge")
            .join("policy-writer")
    );
    assert_eq!(layout.bin, layout.root.join("bin"));
    assert_eq!(
        layout.binary,
        layout.bin.join("systemprompt-bridge.exe"),
        "the task runs the copy under ProgramData, never the user's install"
    );
    assert_eq!(layout.inbox, layout.root.join("inbox"));
    assert_eq!(layout.outbox, layout.root.join("outbox"));
    let id = uuid::Uuid::nil();
    assert_eq!(
        layout
            .request_path(id)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        "request-00000000-0000-0000-0000-000000000000.json"
    );
    assert_eq!(
        layout
            .result_path(id)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        "result-00000000-0000-0000-0000-000000000000.json"
    );

    let xml = render_task_xml(&layout.binary, &layout.root);
    assert!(xml.contains("<UserId>S-1-5-18</UserId>"), "{xml}");
    assert!(xml.contains("<LogonType>ServiceAccount</LogonType>"));
    assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    assert!(
        xml.contains("<Triggers />"),
        "a writer with a trigger would run on its own"
    );
    assert!(xml.contains(&format!("<Command>{}</Command>", layout.binary.display())));
    assert!(xml.contains(&format!(
        "<Arguments>__apply-policy-task \"{}\"</Arguments>",
        layout.root.display()
    )));
    assert!(xml.contains("<ExecutionTimeLimit>PT2M</ExecutionTimeLimit>"));
    assert_eq!(task_name(), "SystempromptBridgePolicyWriter");
}

// Why: the descriptors are the security model, spelled out. A user may add a
// request and read only their own; may run the task but not change it; may
// read the binary but not replace it.
#[test]
fn the_descriptors_keep_users_out_of_the_binary_and_each_others_requests() {
    for sddl in [BIN_SDDL, INBOX_SDDL, OUTBOX_SDDL] {
        assert!(
            sddl.starts_with("D:P("),
            "protected, never inherited: {sddl}"
        );
        assert!(sddl.contains("(A;OICI;FA;;;SY)") && sddl.contains("(A;OICI;FA;;;BA)"));
        assert!(
            !sddl.contains("FA;;;AU") && !sddl.contains("FA;;;BU"),
            "{sddl}"
        );
    }
    assert!(
        BIN_SDDL.contains("0x1200a9;;;AU"),
        "users read and execute the copy"
    );
    assert!(
        INBOX_SDDL.contains("0x120007;;;AU"),
        "users add a file and list, no more"
    );
    assert!(
        INBOX_SDDL.contains("(A;OIIO;FA;;;CO)"),
        "a request is its creator's alone"
    );
    assert!(
        OUTBOX_SDDL.contains("0x120005;;;AU"),
        "users list the outbox; results carry their own reader"
    );
    assert!(
        TASK_SDDL.contains("GRGX;;;AU") && !TASK_SDDL.contains("GA;;;AU"),
        "{TASK_SDDL}"
    );
}

#[test]
fn the_writer_answers_with_exactly_one_policy_step_at_the_machine_key() {
    let steps = expected_steps();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].operation, "policy");
    assert_eq!(steps[0].target, r"HKLM\SOFTWARE\Policies\Claude");
}

#[test]
fn request_reader_accepts_complete_input_and_refuses_oversized_or_wrong_protocol_input() {
    use systemprompt_bridge::install::policy_writer::request::read_request;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("request.json");
    let key = signing_key(1);
    let mut req = request(&key, &manifest(vec![]), &[]);
    std::fs::write(&path, serde_json::to_vec(&req).unwrap()).unwrap();
    assert_eq!(read_request(&path).unwrap().job_id, req.job_id);
    req.version = REQUEST_VERSION + 1;
    std::fs::write(&path, serde_json::to_vec(&req).unwrap()).unwrap();
    assert!(matches!(
        read_request(&path),
        Err(PolicyWriterError::Version { .. })
    ));
    std::fs::write(&path, vec![b' '; MAX_REQUEST_BYTES as usize + 1]).unwrap();
    assert!(
        read_request(&path)
            .unwrap_err()
            .to_string()
            .contains("size limit")
    );
    assert!(read_request(dir.path()).is_err());
}

#[cfg(unix)]
#[test]
fn request_reader_never_follows_a_symbolic_link() {
    use systemprompt_bridge::install::policy_writer::request::read_request;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("private.json");
    let path = dir.path().join("request.json");
    let key = signing_key(1);
    let req = request(&key, &manifest(vec![]), &[]);
    std::fs::write(&target, serde_json::to_vec(&req).unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(read_request(&path).is_err());
    assert_eq!(read_request(&target).unwrap().job_id, req.job_id);
}

#[cfg(target_os = "windows")]
#[test]
fn request_reader_refuses_an_existing_writer_handle() {
    use std::io::Write;
    use systemprompt_bridge::install::policy_writer::request::read_request;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("request.json");
    let key = signing_key(1);
    let req = request(&key, &manifest(vec![]), &[]);
    let mut writer = std::fs::File::create(&path).unwrap();
    writer
        .write_all(&serde_json::to_vec(&req).unwrap())
        .unwrap();
    writer.sync_all().unwrap();
    assert!(read_request(&path).is_err());
    drop(writer);
    assert_eq!(read_request(&path).unwrap().job_id, req.job_id);
}
