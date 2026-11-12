use systemprompt_manifest::services::{
    AgentCardConfig, AgentMetadataConfig, AgentProviderInfo, AiConfig, CapabilitiesConfig,
    DiskAgentConfig, DiskHookConfig, DiskRuleConfig, HistoryConfig, MarketplaceAccess,
    MarketplaceConfigFile, McpConfig, ModelCapabilities, ModelDefinition, ModelPricing,
    OAuthConfig, PluginAuthor, PluginScript, PluginVariableDef, ResilienceSettings, SamplingConfig,
    SchedulerConfig, SkillConfig, SkillsConfig,
};

const CARD: &str = "protocolVersion: '1.0'\ndisplayName: Agent\ndescription: d\nversion: '1'\n";

fn with_unknown_key(yaml: &str) -> String {
    format!("{yaml}unexpected_key: 1\n")
}

macro_rules! strict_case {
    ($ty:ty, $yaml:expr) => {
        (
            stringify!($ty),
            String::from($yaml),
            (|doc: &str| {
                serde_yaml::from_str::<$ty>(doc)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }) as fn(&str) -> Result<(), String>,
        )
    };
}

#[test]
fn every_operator_config_struct_refuses_an_unknown_key() {
    let disk_agent = format!(
        "name: agent\ndisplay_name: Agent\ndescription: d\nport: 9000\ncard:\n{}",
        CARD.lines().map(|l| format!("  {l}\n")).collect::<String>()
    );
    let cases = [
        strict_case!(AiConfig, "default_max_output_tokens: 10\n"),
        strict_case!(SamplingConfig, "enable_smart_routing: true\n"),
        strict_case!(McpConfig, "resilience:\n  retry_attempts: 2\n"),
        strict_case!(ResilienceSettings, "retry_attempts: 2\n"),
        strict_case!(HistoryConfig, "retention_days: 7\n"),
        strict_case!(ModelCapabilities, "vision: true\n"),
        strict_case!(ModelPricing, "input_per_million: 1.0\n"),
        strict_case!(ModelDefinition, "capabilities:\n  vision: true\n"),
        strict_case!(SkillsConfig, "enabled: true\n"),
        strict_case!(SkillConfig, "id: skill_one\nname: Skill\ndescription: d\n"),
        strict_case!(AgentCardConfig, CARD),
        strict_case!(CapabilitiesConfig, "streaming: true\n"),
        strict_case!(
            AgentProviderInfo,
            "organization: o\nurl: https://example.com\n"
        ),
        strict_case!(AgentMetadataConfig, "model: m\n"),
        strict_case!(OAuthConfig, "required: false\n"),
        strict_case!(DiskAgentConfig, disk_agent.as_str()),
        strict_case!(DiskHookConfig, "event: PreToolUse\n"),
        strict_case!(DiskRuleConfig, "id: rule_one\nname: Rule\ndescription: d\n"),
        strict_case!(PluginAuthor, "name: n\nemail: n@example.com\n"),
        strict_case!(PluginScript, "name: n\nsource: s\n"),
        strict_case!(PluginVariableDef, "name: TOKEN\n"),
        strict_case!(MarketplaceAccess, "default_included: true\n"),
        strict_case!(SchedulerConfig, "enabled: true\n"),
    ];

    for (name, yaml, parse) in cases {
        parse(&yaml).unwrap_or_else(|e| panic!("{name}: the base document must parse: {e}"));
        let err = parse(&with_unknown_key(&yaml))
            .err()
            .unwrap_or_else(|| panic!("{name}: an unknown key must be refused"));
        assert!(err.contains("unexpected_key"), "{name}: {err}");
    }
}

#[test]
fn marketplace_config_file_refuses_an_unknown_top_level_key() {
    let err = serde_yaml::from_str::<MarketplaceConfigFile>("unexpected_key: 1\n")
        .expect_err("unknown key");
    assert!(err.to_string().contains("unexpected_key"), "{err}");
}

#[test]
fn flat_mcp_timeouts_are_refused() {
    let err = serde_yaml::from_str::<AiConfig>("mcp:\n  connect_timeout_ms: 5000\n")
        .expect_err("flat connect_timeout_ms is not an mcp key");
    assert!(err.to_string().contains("connect_timeout_ms"), "{err}");
}

#[test]
fn retired_ai_keys_are_refused() {
    for yaml in [
        "mcp:\n  auto_discover: true\n",
        "history:\n  log_tool_executions: true\n",
    ] {
        serde_yaml::from_str::<AiConfig>(yaml).expect_err(yaml);
    }
    serde_yaml::from_str::<SkillsConfig>("auto_discover: true\n")
        .expect_err("skills.auto_discover is retired");
}

#[test]
fn nested_mcp_resilience_still_parses() {
    let config: AiConfig = serde_yaml::from_str(
        "mcp:\n  resilience:\n    connect_timeout_ms: 1500\n    request_timeout_ms: 9000\n    retry_attempts: 4\n",
    )
    .expect("mcp.resilience parses");
    assert_eq!(config.mcp.resilience.connect_timeout_ms, 1500);
    assert_eq!(config.mcp.resilience.request_timeout_ms, 9000);
    assert_eq!(config.mcp.resilience.retry_attempts, 4);
}
