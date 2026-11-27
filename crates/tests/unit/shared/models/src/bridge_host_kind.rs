use std::collections::BTreeMap;

use systemprompt_models::bridge::host::{ConfigOwner, HostAppKind, HostKind, HostProduct};
use systemprompt_models::bridge::manifest::SignedManifest;
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_models::origin::ClientKind;

#[test]
fn every_host_round_trips_through_its_wire_id() {
    for kind in HostKind::ALL {
        assert_eq!(kind.as_str().parse::<HostKind>(), Ok(kind));
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{}\"", kind.as_str()));
        assert_eq!(serde_json::from_str::<HostKind>(&json).unwrap(), kind);
        assert_eq!(kind.to_string(), kind.as_str());
    }
}

#[test]
fn the_eight_wire_ids_are_the_bridge_host_ids() {
    let ids: Vec<&str> = HostKind::ALL.map(HostKind::as_str).to_vec();
    assert_eq!(
        ids,
        [
            "claude-code",
            "claude-desktop",
            "codex",
            "codex-cli",
            "opencode",
            "opencode-cli",
            "hermes",
            "harness",
        ]
    );
}

#[test]
fn an_unknown_or_legacy_alias_is_rejected() {
    assert!("open-code".parse::<HostKind>().is_err());
    assert!("Codex".parse::<HostKind>().is_err());
    assert!("".parse::<HostKind>().is_err());
    assert!(HostKind::parse_list(&["codex", "cowork"]).is_err());
}

#[test]
fn packaging_and_product_split_the_pairs() {
    assert_eq!(HostKind::Codex.kind(), HostAppKind::DesktopApp);
    assert_eq!(HostKind::CodexCli.kind(), HostAppKind::CliTool);
    assert_eq!(HostKind::OpenCode.kind(), HostAppKind::DesktopApp);
    assert_eq!(HostKind::OpenCodeCli.kind(), HostAppKind::CliTool);
    assert_eq!(HostKind::ClaudeDesktop.kind(), HostAppKind::DesktopApp);
    assert_eq!(HostKind::ClaudeCode.kind(), HostAppKind::CliTool);
    assert_eq!(HostKind::Hermes.kind(), HostAppKind::CliTool);
    assert_eq!(HostKind::Harness.kind(), HostAppKind::DesktopApp);

    assert_eq!(HostKind::Codex.product(), HostProduct::Codex);
    assert_eq!(HostKind::CodexCli.product(), HostProduct::Codex);
    assert_eq!(HostKind::OpenCode.product(), HostProduct::OpenCode);
    assert_eq!(HostKind::OpenCodeCli.product(), HostProduct::OpenCode);
    assert_eq!(HostKind::ClaudeCode.product(), HostProduct::Claude);
    assert_eq!(HostKind::ClaudeDesktop.product(), HostProduct::Claude);
    assert_eq!(HostKind::Hermes.product(), HostProduct::Hermes);
    assert_eq!(HostKind::Harness.product(), HostProduct::Harness);

    let codex: Vec<HostKind> = HostProduct::Codex.hosts().collect();
    assert_eq!(codex, [HostKind::Codex, HostKind::CodexCli]);
    let hermes: Vec<HostKind> = HostProduct::Hermes.hosts().collect();
    assert_eq!(hermes, [HostKind::Hermes]);
    let harness: Vec<HostKind> = HostProduct::Harness.hosts().collect();
    assert_eq!(harness, [HostKind::Harness]);
}

#[test]
fn the_bridge_owns_every_configuration_but_the_harness_apps() {
    assert_eq!(HostKind::Harness.config_owner(), ConfigOwner::Host);
    let host_owned: Vec<HostKind> = HostKind::ALL
        .into_iter()
        .filter(|host| host.config_owner() == ConfigOwner::Host)
        .collect();
    assert_eq!(host_owned, [HostKind::Harness]);
    let bridge_owned: Vec<HostKind> = HostKind::ALL
        .into_iter()
        .filter(|host| host.config_owner() == ConfigOwner::Bridge)
        .collect();
    assert_eq!(
        bridge_owned,
        [
            HostKind::ClaudeCode,
            HostKind::ClaudeDesktop,
            HostKind::Codex,
            HostKind::CodexCli,
            HostKind::OpenCode,
            HostKind::OpenCodeCli,
            HostKind::Hermes,
        ]
    );
    for host in HostKind::ALL {
        if let Some(sibling) = host.shares_config_with() {
            assert_eq!(
                sibling.config_owner(),
                host.config_owner(),
                "{host} and {sibling} read one configuration tree, so one party owns it"
            );
        }
    }
}

#[test]
fn shared_config_is_a_symmetric_relation_over_the_two_pairs() {
    for host in HostKind::ALL {
        match host.shares_config_with() {
            Some(sibling) => {
                assert_ne!(sibling, host);
                assert_eq!(sibling.product(), host.product());
                assert_ne!(sibling.kind(), host.kind());
                assert_eq!(sibling.shares_config_with(), Some(host));
            },
            None => assert!(
                matches!(
                    host,
                    HostKind::ClaudeCode
                        | HostKind::ClaudeDesktop
                        | HostKind::Hermes
                        | HostKind::Harness
                ),
                "{host} has no sibling"
            ),
        }
    }
}

#[test]
fn both_packagings_of_a_product_wear_one_client_kind() {
    assert_eq!(ClientKind::from(HostKind::Codex), ClientKind::Codex);
    assert_eq!(ClientKind::from(HostKind::CodexCli), ClientKind::Codex);
    assert_eq!(ClientKind::from(HostKind::OpenCode), ClientKind::OpenCode);
    assert_eq!(
        ClientKind::from(HostKind::OpenCodeCli),
        ClientKind::OpenCode
    );
    assert_eq!(
        ClientKind::from(HostKind::ClaudeCode),
        ClientKind::ClaudeCode
    );
    assert_eq!(
        ClientKind::from(HostKind::ClaudeDesktop),
        ClientKind::ClaudeDesktop
    );
    assert_eq!(ClientKind::from(HostKind::Hermes), ClientKind::Hermes);
    assert_eq!(ClientKind::from(HostKind::Harness), ClientKind::Harness);

    let codex_hosts: Vec<HostKind> = ClientKind::Codex.hosts().collect();
    assert_eq!(codex_hosts, [HostKind::Codex, HostKind::CodexCli]);
    assert_eq!(ClientKind::Other.hosts().count(), 0);
    for host in HostKind::ALL {
        assert!(
            ClientKind::from(host).hosts().any(|h| h == host),
            "{host} must be listed by its own ClientKind"
        );
    }
}

#[test]
fn evaluator_clients_accept_every_packaging_of_their_product_and_nothing_else() {
    for host in HostKind::ALL {
        let accepting: Vec<EvaluatorClient> = [
            EvaluatorClient::ClaudeCode,
            EvaluatorClient::ClaudeDesktop,
            EvaluatorClient::Codex,
            EvaluatorClient::OpenCode,
            EvaluatorClient::Hermes,
            EvaluatorClient::Harness,
        ]
        .into_iter()
        .filter(|client| client.accepts(host))
        .collect();
        assert_eq!(accepting, [EvaluatorClient::from(host)], "{host}");
    }
    assert!(EvaluatorClient::Codex.accepts(HostKind::Codex));
    assert!(EvaluatorClient::Codex.accepts(HostKind::CodexCli));
    assert!(EvaluatorClient::OpenCode.accepts(HostKind::OpenCodeCli));
    assert!(!EvaluatorClient::ClaudeCode.accepts(HostKind::ClaudeDesktop));
}

#[test]
fn the_opencode_alias_is_gone_from_the_evaluator_wire() {
    assert!(serde_json::from_str::<EvaluatorClient>("\"opencode\"").is_err());
    assert_eq!(
        serde_json::from_str::<EvaluatorClient>("\"open-code\"").unwrap(),
        EvaluatorClient::OpenCode
    );
}

#[test]
fn hook_host_execution_source_follows_the_product() {
    assert_eq!(
        ExecutionSource::from_hook_host(HostKind::OpenCode),
        Some(ExecutionSource::HookOpenCode)
    );
    assert_eq!(
        ExecutionSource::from_hook_host(HostKind::OpenCodeCli),
        Some(ExecutionSource::HookOpenCode)
    );
    assert_eq!(
        ExecutionSource::from_hook_host(HostKind::ClaudeCode),
        Some(ExecutionSource::HookClaudeCode)
    );
    assert_eq!(
        ExecutionSource::from_hook_host(HostKind::CodexCli),
        Some(ExecutionSource::HookClaudeCode)
    );
}

#[test]
fn a_host_with_no_hook_vantage_point_borrows_no_other_hosts_source() {
    assert_eq!(ExecutionSource::from_hook_host(HostKind::Harness), None);
    for host in HostKind::ALL {
        if host.config_owner() == ConfigOwner::Bridge {
            assert!(
                ExecutionSource::from_hook_host(host).is_some(),
                "{host} reports tool results through a hook the platform records"
            );
        }
    }
}

#[test]
fn harness_is_its_own_declarable_client_and_evaluator_on_the_wire() {
    assert_eq!(HostKind::Harness.as_str(), "harness");
    assert_eq!(ClientKind::Harness.as_str(), "harness");
    assert!(ClientKind::DECLARABLE.contains(&ClientKind::Harness));
    assert_eq!(
        ClientKind::from_ua_product("harness"),
        Some(ClientKind::Harness)
    );
    assert_eq!(ClientKind::Harness.product(), Some(HostProduct::Harness));
    assert_eq!(
        serde_json::to_string(&EvaluatorClient::Harness).unwrap(),
        "\"harness\""
    );
    assert_eq!(
        EvaluatorClient::try_from(ClientKind::Harness),
        Ok(EvaluatorClient::Harness)
    );
    assert_eq!(
        ClientKind::from(EvaluatorClient::Harness),
        ClientKind::Harness
    );
    assert_eq!(HostKind::Harness.shares_config_with(), None);
}

fn manifest_json(enabled_hosts: &str, protocols: &str) -> String {
    format!(
        r#"{{
            "min_schema_version": 1,
            "manifest_version": "2026-10-22T00:00:00Z-00000001",
            "issued_at": "2026-10-22T00:00:00Z",
            "not_before": "2026-10-22T00:00:00Z",
            "user_id": "user-1",
            "plugins": [],
            "managed_mcp_servers": [],
            "revocations": [],
            "enabled_hosts": {enabled_hosts},
            "host_model_protocols": {protocols}
        }}"#
    )
}

#[test]
fn a_manifest_naming_an_unknown_host_id_does_not_deserialise() {
    let err =
        serde_json::from_str::<SignedManifest>(&manifest_json(r#"["codex", "cursor"]"#, "{}"))
            .expect_err("an unknown host id is a wire error, not a host nobody serves");
    assert!(err.to_string().contains("cursor"), "{err}");
    let err = serde_json::from_str::<SignedManifest>(&manifest_json(
        "[]",
        r#"{"open-code": ["openai"]}"#,
    ))
    .expect_err("an unknown protocol-override key is a wire error");
    assert!(err.to_string().contains("open-code"), "{err}");
}

#[test]
fn a_manifest_carries_typed_hosts_and_an_empty_list_enables_none() {
    let manifest: SignedManifest = serde_json::from_str(&manifest_json(
        r#"["opencode-cli", "codex"]"#,
        r#"{"codex": ["openai"]}"#,
    ))
    .expect("eight-id vocabulary");
    assert_eq!(
        manifest.enabled_hosts,
        [HostKind::OpenCodeCli, HostKind::Codex]
    );
    let mut expected = BTreeMap::new();
    expected.insert(HostKind::Codex, vec!["openai".to_owned()]);
    assert_eq!(manifest.host_model_protocols, expected);

    let none: SignedManifest =
        serde_json::from_str(&manifest_json("[]", "{}")).expect("empty list parses");
    assert!(none.enabled_hosts.is_empty());
    assert!(
        HostKind::ALL
            .iter()
            .all(|host| !none.enabled_hosts.contains(host)),
        "an empty enabled list enables no host"
    );
    let round = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        round["enabled_hosts"],
        serde_json::json!(["opencode-cli", "codex"])
    );
    assert_eq!(
        round["host_model_protocols"]["codex"],
        serde_json::json!(["openai"])
    );
}

#[test]
fn host_extensions_are_absent_from_the_wire_when_empty() {
    let manifest: SignedManifest =
        serde_json::from_str(&manifest_json(r#"["harness"]"#, "{}")).expect("parses");
    assert!(manifest.host_extensions.is_empty());
    let round = serde_json::to_value(&manifest).unwrap();
    assert!(
        round.get("host_extensions").is_none(),
        "an empty map must not change the signed bytes: {round}"
    );
}

#[test]
fn host_extensions_map_a_host_id_to_its_extension_ids() {
    let mut value: serde_json::Value =
        serde_json::from_str(&manifest_json(r#"["harness"]"#, "{}")).unwrap();
    value["host_extensions"] = serde_json::json!({"harness": ["drive", "brain"]});
    let manifest: SignedManifest = serde_json::from_value(value).expect("parses");
    assert_eq!(
        manifest.host_extensions.get(&HostKind::Harness),
        Some(&vec!["drive".to_owned(), "brain".to_owned()])
    );
    let round = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        round["host_extensions"],
        serde_json::json!({"harness": ["drive", "brain"]})
    );

    let mut unknown: serde_json::Value = serde_json::from_str(&manifest_json("[]", "{}")).unwrap();
    unknown["host_extensions"] = serde_json::json!({"cursor": ["drive"]});
    assert!(serde_json::from_value::<SignedManifest>(unknown).is_err());
}
