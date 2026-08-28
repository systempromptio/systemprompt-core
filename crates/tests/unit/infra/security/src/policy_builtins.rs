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

mod require_approval {
    use super::{Call, args, engine, tool};
    use systemprompt_security::authz::types::Decision;
    use systemprompt_security::policy::types::AccessScope;
    use systemprompt_security::policy::{ApprovalSettings, ChainEntryResult, GovernanceConfig};

    const HOLDS_NOTE_ADD: &str = r"
governance:
  policies:
  - id: require_approval
    enabled: true
    patterns: ['note_add', 'channel_post']
    exempt_scopes: ['admin']
";

    #[test]
    fn a_matching_tool_is_held_not_denied() {
        let call = Call::new("sales");
        let input = args(serde_json::json!({"body": "hi"}));
        let target = tool("mcp__odoo__note_add");
        let evaluation =
            engine(HOLDS_NOTE_ADD).evaluate(&call.ctx(&target, AccessScope::User, &input));

        assert!(
            matches!(evaluation.decision, Decision::Pending { .. }),
            "expected a hold, got {:?}",
            evaluation.decision
        );
    }

    #[test]
    fn a_hold_is_traced_as_hold_rather_than_fail() {
        let call = Call::new("sales");
        let input = args(serde_json::json!({}));
        let target = tool("mcp__odoo__channel_post");
        let evaluation =
            engine(HOLDS_NOTE_ADD).evaluate(&call.ctx(&target, AccessScope::User, &input));

        // Why assert this separately: the audit row's `policy` column is
        // resolved by finding the Hold entry. If a hold were traced as Fail it
        // would be indistinguishable from a denial in every dashboard.
        assert!(
            evaluation
                .chain
                .iter()
                .any(|e| e.result == ChainEntryResult::Hold),
            "expected a Hold entry in the trace, got {:?}",
            evaluation.chain
        );
    }

    const EXTERNAL_ONLY: &str = r#"
governance:
  policies:
  - id: require_approval
    enabled: true
    patterns:
    - channel_post
    - tool: email_send
      name: external_recipient
      when:
      - path: to
        op: domain_suffix
        values: ["systemprompt.io"]
        negate: true
        match: any
    - tool: crm_lead_write
      name: high_value_deal
      when:
      - path: expected_revenue
        op: gt
        value: 50000
    exempt_scopes: ['admin']
"#;

    fn verdict(yaml: &str, name: &str, arguments: serde_json::Value) -> Decision {
        let call = Call::new("sales");
        let input = args(arguments);
        let target = tool(name);
        engine(yaml)
            .evaluate(&call.ctx(&target, AccessScope::User, &input))
            .decision
    }

    fn held_rule(decision: &Decision) -> String {
        match decision {
            Decision::Pending { reason } => format!("{reason:?}"),
            other => panic!("expected a hold, got {other:?}"),
        }
    }

    #[test]
    fn a_bare_pattern_still_ignores_arguments_entirely() {
        // The backward-compatibility guarantee: adding conditions to the schema
        // must not change what a string entry does, however rich the payload.
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__odoo__channel_post",
            serde_json::json!({"to": ["anyone@systemprompt.io"], "expected_revenue": 1}),
        );
        assert!(held_rule(&decision).contains("channel_post"));
    }

    #[test]
    fn every_recipient_internal_runs_unattended() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": ["a@systemprompt.io", "b@mail.systemprompt.io"]}),
        );
        assert!(
            matches!(decision, Decision::Allow { .. }),
            "internal-only mail must not pull in a second human, got {decision:?}"
        );
    }

    #[test]
    fn one_external_recipient_holds_and_names_which() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": ["a@systemprompt.io", "x@gmail.com"]}),
        );
        let rule = held_rule(&decision);
        assert!(rule.contains("external_recipient"), "got {rule}");
        assert!(
            rule.contains("to[1]"),
            "the approver must see which recipient tripped it, got {rule}"
        );
    }

    #[test]
    fn a_scalar_field_matches_the_same_rule_as_an_array() {
        // Index erasure: a tool accepting `string | string[]` must not need two
        // rules, and must not silently escape the one that exists.
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": "x@gmail.com"}),
        );
        assert!(held_rule(&decision).contains("external_recipient"));
    }

    #[test]
    fn a_lookalike_domain_does_not_pass_as_internal() {
        // `ends_with("systemprompt.io")` would accept this. Dotted-label
        // matching is the whole point of the operator.
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": ["evil@systemprompt.io.attacker.com"]}),
        );
        assert!(held_rule(&decision).contains("external_recipient"));
    }

    #[test]
    fn a_display_name_cannot_launder_an_external_address() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": ["\"systemprompt.io\" <attacker@evil.com>"]}),
        );
        assert!(
            held_rule(&decision).contains("external_recipient"),
            "the domain comes from the addr-spec, never the display name"
        );
    }

    #[test]
    fn an_internal_address_with_a_display_name_and_odd_case_is_allowed() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": ["Ed Burton <Ed@SystemPrompt.IO>"]}),
        );
        assert!(matches!(decision, Decision::Allow { .. }), "{decision:?}");
    }

    #[test]
    fn a_numeric_condition_holds_only_above_its_threshold() {
        let over = verdict(
            EXTERNAL_ONLY,
            "mcp__odoo__crm_lead_write",
            serde_json::json!({"expected_revenue": 75000}),
        );
        assert!(held_rule(&over).contains("high_value_deal"));

        let under = verdict(
            EXTERNAL_ONLY,
            "mcp__odoo__crm_lead_write",
            serde_json::json!({"expected_revenue": 25000}),
        );
        assert!(matches!(under, Decision::Allow { .. }), "{under:?}");
    }

    #[test]
    fn a_missing_path_fails_closed() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"subject": "no recipients here"}),
        );
        assert!(held_rule(&decision).contains("fail-closed"));
    }

    #[test]
    fn an_empty_recipient_list_is_not_vacuously_internal() {
        // Under `match: any` an empty set trivially matches nothing, which
        // would read as "all recipients are internal". It must hold instead.
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__email__email_send",
            serde_json::json!({"to": []}),
        );
        assert!(held_rule(&decision).contains("fail-closed"));
    }

    #[test]
    fn a_value_the_operator_cannot_compare_fails_closed() {
        let decision = verdict(
            EXTERNAL_ONLY,
            "mcp__odoo__crm_lead_write",
            serde_json::json!({"expected_revenue": "75000"}),
        );
        assert!(held_rule(&decision).contains("fail-closed"));
    }

    #[test]
    fn a_malformed_rule_is_dropped_without_taking_its_siblings_with_it() {
        // The opposite failure direction from the tests above, and deliberately
        // so: a config typo must not conjure a hold nobody configured.
        const BROKEN: &str = r"
governance:
  policies:
  - id: require_approval
    enabled: true
    patterns:
    - note_add
    - tool: email_send
      when: 'not a list'
    exempt_scopes: ['admin']
";
        assert!(
            held_rule(&verdict(
                BROKEN,
                "mcp__odoo__note_add",
                serde_json::json!({})
            ))
            .contains("note_add")
        );
        assert!(matches!(
            verdict(
                BROKEN,
                "mcp__email__email_send",
                serde_json::json!({"to": ["x@gmail.com"]})
            ),
            Decision::Allow { .. }
        ));
    }

    #[test]
    fn the_same_call_evaluates_identically_every_round() {
        // MRTR retries re-enter with the same derived call id. A verdict that
        // drifted between rounds would never converge on one approval row.
        let call = Call::new("sales");
        let input = args(serde_json::json!({"to": ["a@systemprompt.io", "x@gmail.com"]}));
        let target = tool("mcp__email__email_send");
        let engine = engine(EXTERNAL_ONLY);
        let first = format!(
            "{:?}",
            engine
                .evaluate(&call.ctx(&target, AccessScope::User, &input))
                .decision
        );
        for _ in 0..3 {
            let again = format!(
                "{:?}",
                engine
                    .evaluate(&call.ctx(&target, AccessScope::User, &input))
                    .decision
            );
            assert_eq!(first, again);
        }
    }

    #[test]
    fn an_unmatched_tool_runs_unattended() {
        let call = Call::new("sales");
        let input = args(serde_json::json!({}));
        let target = tool("mcp__odoo__lead_search");
        let evaluation =
            engine(HOLDS_NOTE_ADD).evaluate(&call.ctx(&target, AccessScope::User, &input));

        assert!(matches!(evaluation.decision, Decision::Allow { .. }));
    }

    #[test]
    fn an_exempt_scope_is_never_held() {
        let call = Call::new("admin");
        let input = args(serde_json::json!({}));
        let target = tool("mcp__odoo__note_add");
        let evaluation =
            engine(HOLDS_NOTE_ADD).evaluate(&call.ctx(&target, AccessScope::Admin, &input));

        // The approver must not be able to hold their own call — that would be
        // a rubber stamp rather than a control.
        assert!(matches!(evaluation.decision, Decision::Allow { .. }));
    }

    #[test]
    fn an_unconfigured_policy_holds_nothing() {
        // The whole module's config layer fails toward MORE enforcement on a
        // bad read. This stage must not: a hold with nobody watching blocks a
        // call indefinitely, so no patterns means no holds.
        let yaml = "
governance:
  policies:
  - id: require_approval
    enabled: true
";
        let call = Call::new("sales");
        let input = args(serde_json::json!({}));
        let target = tool("mcp__odoo__note_add");
        let evaluation = engine(yaml).evaluate(&call.ctx(&target, AccessScope::User, &input));

        assert!(matches!(evaluation.decision, Decision::Allow { .. }));
    }

    #[test]
    fn defaults_do_not_enable_the_holding_stage() {
        assert!(
            !GovernanceConfig::defaults()
                .policies
                .iter()
                .any(|p| p.id == "require_approval" && p.enabled),
            "require_approval must never be enabled by a fallback config read"
        );
    }

    #[test]
    fn timings_come_from_the_policy_entry() {
        let config = GovernanceConfig::parse(
            "
governance:
  policies:
  - id: require_approval
    enabled: true
    hold_seconds: 5
    expiry_seconds: 60
",
        )
        .unwrap();
        let settings = ApprovalSettings::from_governance_config(&config);
        assert_eq!(settings.hold_seconds, 5);
        assert_eq!(settings.expiry_seconds, 60);
    }

    #[test]
    fn a_zero_timing_falls_back_to_the_default() {
        // Zero means "hold for no time" / "expire instantly", which is a typo
        // rather than an intent worth honouring.
        let config = GovernanceConfig::parse(
            "
governance:
  policies:
  - id: require_approval
    enabled: true
    hold_seconds: 0
",
        )
        .unwrap();
        let settings = ApprovalSettings::from_governance_config(&config);
        assert_eq!(
            settings.hold_seconds,
            ApprovalSettings::default().hold_seconds
        );
    }
}

mod approval_digest {
    use systemprompt_security::policy::args_digest;

    #[test]
    fn key_order_does_not_change_the_digest() {
        // The digest binds an approval to the payload it authorised. If key
        // order moved it, an identical retry would look like a different call
        // and be re-held forever.
        let a = serde_json::json!({"body": "hi", "lead_id": 7});
        let b = serde_json::json!({"lead_id": 7, "body": "hi"});
        assert_eq!(args_digest(&a), args_digest(&b));
    }

    #[test]
    fn a_changed_value_changes_the_digest() {
        let approved = serde_json::json!({"body": "hi", "lead_id": 7});
        let swapped = serde_json::json!({"body": "hi", "lead_id": 8});
        assert_ne!(args_digest(&approved), args_digest(&swapped));
    }

    #[test]
    fn array_order_is_significant() {
        let a = serde_json::json!({"to": ["a@x.com", "b@x.com"]});
        let b = serde_json::json!({"to": ["b@x.com", "a@x.com"]});
        assert_ne!(args_digest(&a), args_digest(&b));
    }
}
