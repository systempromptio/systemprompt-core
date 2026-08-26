use serde_json::json;
use systemprompt_identifiers::{CallId, McpToolName, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_security::policy::secrets::{self, SECRET_PATTERNS};
use systemprompt_security::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_security::policy::{
    ChainEntryResult, EntropyConfig, GovernanceConfig, GovernanceEngine, detect_secrets,
    detect_secrets_with, scan_str_for_secret,
};

fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap())
}

struct Call {
    session: SessionId,
    user: UserId,
    call: CallId,
}

impl Call {
    fn new(user: &str) -> Self {
        Self {
            session: SessionId::generate(),
            user: UserId::new(user),
            call: CallId::generate(),
        }
    }

    fn ctx<'a>(
        &'a self,
        target: &GovernedTarget,
        scope: AccessScope,
        input: &'a GovernedInput,
    ) -> PolicyContext<'a> {
        PolicyContext {
            target: target.clone(),
            agent_scope: AgentScope::User {
                user_id: self.user.clone(),
            },
            access_scope: scope,
            session_id: &self.session,
            user_id: &self.user,
            input,
            call_id: &self.call,
        }
    }
}

fn tool(name: &str) -> GovernedTarget {
    GovernedTarget::Tool {
        tool: McpToolName::new(name),
    }
}

fn args(v: serde_json::Value) -> GovernedInput {
    GovernedInput::tool_arguments(McpToolInput::new(v))
}

// --- secret scanner ---

#[test]
fn a_prefixless_base64_key_is_caught_by_the_entropy_detector() {
    let input = GovernedInput::prompt_text(
        "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM here is my api key".to_owned(),
    );
    let hit = detect_secrets(&input).expect("entropy detector must fire");
    assert_eq!(hit.pattern.id, "high-entropy-token");
    assert!(hit.redacted.ends_with("...[REDACTED]"));
    assert!(!hit.redacted.contains("FIyuuM"));
}

// Why: the blob that triggered the incident — a protobuf record id returned by
// a Salesforce MCP tool. Structurally a serialised message, not key material.
const PROTOBUF_TOOL_PAYLOAD: &str =
    "CAISLwoLaGVsbG8gd29ybGQSC2NvbnRhY3QtaWQxGhNSb3NlIEdvbnphbGV6IE93bmVy";

#[test]
fn a_serialised_protobuf_payload_is_not_reported_as_a_credential() {
    let input =
        GovernedInput::prompt_text(format!("tool returned {PROTOBUF_TOOL_PAYLOAD} for you"));
    assert!(
        detect_secrets(&input).is_none(),
        "structured tool output must not read as key material"
    );
}

#[test]
fn a_base64_json_envelope_is_not_reported_as_a_credential() {
    let envelope = "eyJ1c2VyIjoiZWR3YXJkIiwicm9sZSI6ImFkbWluIiwidGVuYW50IjoiYXN0b3VuZCJ9";
    assert!(
        scan_str_for_secret(envelope).is_none(),
        "base64-encoded JSON must not read as key material"
    );
}

#[test]
fn the_entropy_allowlist_suppresses_a_named_token_shape() {
    let text = "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM";
    let config = EntropyConfig {
        allowlist: vec![regex::Regex::new("^PHL").expect("test regex compiles")],
        ..EntropyConfig::default()
    };
    let input = GovernedInput::prompt_text(text.to_owned());
    assert!(detect_secrets(&input).is_some(), "unconfigured, it fires");
    assert!(
        detect_secrets_with(&input, &config).is_none(),
        "an allowlisted shape is exempt"
    );
}

#[test]
fn disabling_the_entropy_heuristic_leaves_the_vendor_patterns_live() {
    let config = EntropyConfig {
        enabled: false,
        ..EntropyConfig::default()
    };
    let prefixless = GovernedInput::prompt_text(
        "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM here is my api key".to_owned(),
    );
    assert!(detect_secrets_with(&prefixless, &config).is_none());

    let vendor = GovernedInput::prompt_text("AKIAIOSFODNN7EXAMPLE".to_owned());
    let hit = detect_secrets_with(&vendor, &config).expect("vendor patterns are not tunable");
    assert_eq!(hit.pattern.id, "aws-access-key");
}

#[test]
fn an_absent_entropy_block_keeps_the_built_in_behaviour() {
    let yaml = "governance:\n  policies:\n    - id: secret_scan\n      enabled: true\n";
    let engine = engine(yaml);
    let call = Call::new("u1");
    let input = GovernedInput::prompt_text(
        "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM here is my api key".to_owned(),
    );
    let target = tool("read_file");
    let evaluation = engine.evaluate(&call.ctx(&target, AccessScope::Unknown, &input));
    assert!(
        matches!(evaluation.decision, Decision::Deny { .. }),
        "defaults must still deny an unprefixed key"
    );
}

#[test]
fn a_configured_entropy_allowlist_reaches_the_policy() {
    let yaml = "governance:\n  policies:\n    - id: secret_scan\n      enabled: true\n      \
                entropy:\n        allowlist:\n          - '^PHL'\n";
    let engine = engine(yaml);
    let call = Call::new("u1");
    let input = GovernedInput::prompt_text(
        "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM here is my api key".to_owned(),
    );
    let target = tool("read_file");
    let evaluation = engine.evaluate(&call.ctx(&target, AccessScope::Unknown, &input));
    assert!(
        matches!(evaluation.decision, Decision::Allow { .. }),
        "the allowlist must reach the policy from YAML"
    );
}

#[test]
fn every_builtin_pattern_compiles() {
    assert_eq!(secrets::compiled_pattern_count(), SECRET_PATTERNS.len());
}

// Why: fixtures are assembled at runtime so no credential-shaped literal
// exists in the source — GitHub push protection scans this file too.
#[test]
fn full_length_vendor_keys_match_their_patterns() {
    let mailgun = format!("key-{}", "0123456789abcdef".repeat(2));
    let cases = [
        ("AKIAIOSFODNN7EXAMPLE", "aws-access-key"),
        (
            "ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            "github-token-classic",
        ),
        ("sk-ant-api03-AbCdEfGhIjKlMnOpQrStUv", "anthropic-api-key"),
        (mailgun.as_str(), "mailgun-api-key"),
        (
            "postgresql://admin:hunter2@db.internal:5432/prod",
            "postgres-url-with-password",
        ),
    ];
    for (text, id) in cases {
        let input = GovernedInput::prompt_text(text.to_owned());
        let hit = detect_secrets(&input);
        assert!(
            matches!(&hit, Some(h) if h.pattern.id == id),
            "{id} should match {text}, got {hit:?}"
        );
    }
}

#[test]
fn prose_fragments_and_benign_urls_do_not_match() {
    for text in [
        "keys start with sk-ant- and AKIA is the AWS marker",
        "connect to redis://localhost:6379 and mysql://db.local/app",
        "the SG. abbreviation, a key-value store, and password= as a concept",
        "postgresql://readonly@db.internal/metrics",
    ] {
        assert!(
            scan_str_for_secret(text).is_none(),
            "false positive on: {text}"
        );
    }
}

#[test]
fn shas_uuids_and_identifiers_do_not_trip_the_entropy_detector() {
    for text in [
        "commit c0196f2a4b8d9e1f2a3b4c5d6e7f8091a2b3c4d5 on main",
        "trace 03f06137-5eb1-4ed9-9b0b-ee6899baa5fa completed",
        "call list_requests_paged_with_total_and_filters_applied please",
        "https://example.com/docs/getting-started/installation-guide-linux",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA1aB",
    ] {
        assert!(
            scan_str_for_secret(text).is_none(),
            "false positive on: {text}"
        );
    }
}

// Why: SRI integrity hashes are public metadata that ride along in page
// markup and tool content; each is dense base64 behind an `algo-` prefix and
// used to deny every request whose forwarded surface carried one.
#[test]
fn sri_integrity_hashes_do_not_trip_the_entropy_detector() {
    for text in [
        "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=",
        "sha384-+mbV2IY1Zk/X1p/nWllGySJSUN8uMs+gUAN10Or95UBH0fpj6GfKgPmgC5EXieXG",
        "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==",
        "integrity=\"sha384-+mbV2IY1Zk/X1p/nWllGySJSUN8uMs+gUAN10Or95UBH0fpj6GfKgPmgC5EXieXG\"",
    ] {
        assert!(
            scan_str_for_secret(text).is_none(),
            "false positive on: {text}"
        );
    }
}

// Why: the digest exoneration is length-verified, not prefix-trusted — a
// credential smuggled behind `sha384-` decodes to the wrong byte count and
// must still be reported.
#[test]
fn a_digest_prefix_with_a_wrong_length_payload_still_denies() {
    let smuggled = "sha384-PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM";
    let hit = scan_str_for_secret(smuggled);
    assert!(
        hit.is_some(),
        "a 32-byte payload behind a sha384 prefix is not a sha384 digest"
    );
}

#[test]
fn a_mixed_alphabet_credential_shaped_token_still_denies() {
    let token = "PHL+ERIbxzlQOeii-RybQwgV7GvYmIclsJe1zsFIyuuM";
    assert!(
        scan_str_for_secret(token).is_some(),
        "mixing base64 alphabets must not launder a credential-shaped token"
    );
}

#[test]
fn a_mistyped_entropy_tunable_falls_back_to_the_default_loudly() {
    let yaml = "governance:\n  policies:\n    - id: secret_scan\n      enabled: true\n      \
                entropy:\n        threshold: not-a-number\n";
    let engine = engine(yaml);
    let call = Call::new("u1");
    let input = GovernedInput::prompt_text(
        "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM here is my api key".to_owned(),
    );
    let target = tool("read_file");
    let evaluation = engine.evaluate(&call.ctx(&target, AccessScope::Unknown, &input));
    assert!(
        matches!(evaluation.decision, Decision::Deny { .. }),
        "a typo must fall back to the default threshold, not disable detection"
    );
}

// --- secret_scan policy ---

#[test]
fn secret_scan_denies_a_credential_in_tool_arguments() {
    let e = engine("governance:\n  policies:\n    - id: secret_scan\n");
    let call = Call::new("u-secret");
    let input = args(json!({ "content": "token ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789" }));
    let evaluation = e.evaluate(&call.ctx(&tool("write_note"), AccessScope::User, &input));
    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { pattern_id, .. }
        } if pattern_id.as_str() == "github-token-classic"
    ));
}

#[test]
fn secret_scan_extra_patterns_deny_on_configured_prefix() {
    let e = engine(
        "governance:\n  policies:\n    - id: secret_scan\n      extra_patterns:\n        - name: Demo Key\n          prefix: \"XDEMO-\"\n",
    );
    let call = Call::new("u-extra");
    let input = args(json!({ "note": "XDEMO-1234" }));
    let evaluation = e.evaluate(&call.ctx(&tool("write_note"), AccessScope::User, &input));
    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { pattern_id, .. }
        } if pattern_id.as_str() == "demo-key"
    ));
}

#[test]
fn secret_scan_allows_clean_input() {
    let e = engine("governance:\n  policies:\n    - id: secret_scan\n");
    let call = Call::new("u-clean");
    let input = args(json!({ "path": "/tmp/notes.txt" }));
    let evaluation = e.evaluate(&call.ctx(&tool("read_file"), AccessScope::User, &input));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

// --- scope_check policy ---

#[test]
fn scope_check_denies_admin_prefixed_tools_for_plain_users() {
    let e = engine("governance:\n  policies:\n    - id: scope_check\n");
    let call = Call::new("u-scope");
    let input = args(json!({}));
    let evaluation = e.evaluate(&call.ctx(
        &tool("mcp__systemprompt__users_delete"),
        AccessScope::User,
        &input,
    ));
    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::ScopeViolation { required, .. }
        } if *required == AccessScope::Admin
    ));
}

#[test]
fn scope_check_admin_scope_short_circuits_to_allow() {
    let e = engine("governance:\n  policies:\n    - id: scope_check\n");
    let call = Call::new("u-admin");
    let input = args(json!({}));
    let evaluation = e.evaluate(&call.ctx(
        &tool("mcp__systemprompt__users_delete"),
        AccessScope::Admin,
        &input,
    ));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

#[test]
fn scope_check_allows_prompt_targets() {
    let e = engine("governance:\n  policies:\n    - id: scope_check\n");
    let call = Call::new("u-prompt");
    let input = GovernedInput::prompt_text("hello".to_owned());
    let evaluation = e.evaluate(&call.ctx(&GovernedTarget::Prompt, AccessScope::Unknown, &input));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

#[test]
fn scope_check_honours_configured_prefixes() {
    let e = engine(
        "governance:\n  policies:\n    - id: scope_check\n      admin_only_prefixes:\n        - \"mcp__custom__\"\n",
    );
    let call = Call::new("u-custom");
    let input = args(json!({}));
    let denied = e.evaluate(&call.ctx(&tool("mcp__custom__wipe"), AccessScope::User, &input));
    assert!(matches!(denied.decision, Decision::Deny { .. }));
    let allowed =
        e.evaluate(&call.ctx(&tool("mcp__systemprompt__list"), AccessScope::User, &input));
    assert!(matches!(allowed.decision, Decision::Allow { .. }));
}

// --- tool_blocklist policy ---

#[test]
fn tool_blocklist_denies_destructive_names_for_non_admin() {
    let e = engine("governance:\n  policies:\n    - id: tool_blocklist\n");
    let call = Call::new("u-block");
    let input = args(json!({}));
    let evaluation = e.evaluate(&call.ctx(&tool("delete_everything"), AccessScope::User, &input));
    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::ToolBlocked { list_id, .. }
        } if list_id == "delete"
    ));
}

#[test]
fn tool_blocklist_admin_bypasses_the_list() {
    let e = engine("governance:\n  policies:\n    - id: tool_blocklist\n");
    let call = Call::new("u-block-admin");
    let input = args(json!({}));
    let evaluation = e.evaluate(&call.ctx(&tool("delete_everything"), AccessScope::Admin, &input));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

// --- rate_limit policy ---

const RATE_LIMIT_2: &str = "governance:\n  policies:\n    - id: rate_limit\n      requests_per_window: 2\n      window_secs: 60\n";

#[test]
fn rate_limit_denies_after_the_configured_limit() {
    let e = engine(RATE_LIMIT_2);
    let session = SessionId::generate();
    let user = UserId::new("u-rate");
    let input = args(json!({}));
    let target = tool("read_file");

    let mut last = None;
    for _ in 0..3 {
        let call = CallId::generate();
        let ctx = PolicyContext {
            target: target.clone(),
            agent_scope: AgentScope::User {
                user_id: user.clone(),
            },
            access_scope: AccessScope::User,
            session_id: &session,
            user_id: &user,
            input: &input,
            call_id: &call,
        };
        last = Some(e.evaluate(&ctx).decision);
    }
    assert!(matches!(
        last,
        Some(Decision::Deny {
            reason: DenyReason::RateLimitExceeded { .. }
        })
    ));
}

#[test]
fn rate_limit_is_idempotent_per_call_id() {
    let e = engine(RATE_LIMIT_2);
    let session = SessionId::generate();
    let user = UserId::new("u-idem");
    let input = args(json!({}));
    let target = tool("read_file");
    let call = CallId::generate();

    let ctx = PolicyContext {
        target: target.clone(),
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
    };

    let first = e.evaluate(&ctx);
    for _ in 0..5 {
        let again = e.evaluate(&ctx);
        assert_eq!(
            entry_detail(&first, "rate_limit"),
            entry_detail(&again, "rate_limit"),
            "re-evaluating one call must reproduce the count it first saw"
        );
    }
}

#[test]
fn rate_limit_state_is_isolated_per_engine() {
    let a = engine(RATE_LIMIT_2);
    let b = engine(RATE_LIMIT_2);
    let session = SessionId::generate();
    let user = UserId::new("u-iso");
    let input = args(json!({}));
    let target = tool("read_file");

    for _ in 0..3 {
        let call = CallId::generate();
        let ctx = PolicyContext {
            target: target.clone(),
            agent_scope: AgentScope::User {
                user_id: user.clone(),
            },
            access_scope: AccessScope::User,
            session_id: &session,
            user_id: &user,
            input: &input,
            call_id: &call,
        };
        let _ = a.evaluate(&ctx);
    }

    let call = CallId::generate();
    let ctx = PolicyContext {
        target,
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
    };
    assert!(matches!(a.evaluate(&ctx).decision, Decision::Deny { .. }));
    assert!(matches!(b.evaluate(&ctx).decision, Decision::Allow { .. }));
}

fn entry_detail(evaluation: &systemprompt_security::policy::Evaluation, id: &str) -> String {
    evaluation
        .chain
        .iter()
        .find(|e| {
            e.policy_id.as_str() == id
                && !matches!(
                    e.result,
                    ChainEntryResult::Skip | ChainEntryResult::Disabled
                )
        })
        .map(|e| e.detail.clone())
        .unwrap_or_default()
}
