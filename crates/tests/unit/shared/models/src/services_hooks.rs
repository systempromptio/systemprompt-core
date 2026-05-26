use std::str::FromStr;

use systemprompt_models::services::{
    HookAction, HookCategory, HookEvent, HookEventsConfig, HookMatcher, HookType,
};

#[test]
fn hook_event_as_str_round_trips_all_variants() {
    for v in HookEvent::ALL_VARIANTS {
        let s = v.as_str();
        let parsed = HookEvent::from_str(s).unwrap();
        assert_eq!(parsed, *v);
        assert_eq!(format!("{v}"), s);
    }
}

#[test]
fn hook_event_from_str_rejects_unknown() {
    assert!(HookEvent::from_str("Bogus").is_err());
}

#[test]
fn hook_category_round_trips() {
    for v in [HookCategory::System, HookCategory::Custom] {
        let s = v.as_str();
        let parsed = HookCategory::from_str(s).unwrap();
        assert_eq!(parsed, v);
        assert_eq!(format!("{v}"), s);
    }
}

#[test]
fn hook_category_from_str_rejects_unknown() {
    assert!(HookCategory::from_str("invalid").is_err());
}

#[test]
fn hook_category_default_is_custom() {
    assert_eq!(HookCategory::default(), HookCategory::Custom);
}

#[test]
fn hook_events_config_default_is_empty() {
    let cfg = HookEventsConfig::default();
    assert!(cfg.is_empty());
    for v in HookEvent::ALL_VARIANTS {
        assert!(cfg.matchers_for_event(*v).is_empty());
    }
}

#[test]
fn hook_events_config_is_empty_false_when_event_populated() {
    let mut cfg = HookEventsConfig::default();
    cfg.pre_tool_use.push(HookMatcher {
        matcher: "*".to_owned(),
        hooks: vec![],
    });
    assert!(!cfg.is_empty());
    assert_eq!(cfg.matchers_for_event(HookEvent::PreToolUse).len(), 1);
}

#[test]
fn hook_events_config_matchers_for_event_routes_each_variant() {
    let mut cfg = HookEventsConfig::default();
    let m = HookMatcher {
        matcher: "*".to_owned(),
        hooks: vec![],
    };
    cfg.pre_tool_use.push(m.clone());
    cfg.post_tool_use.push(m.clone());
    cfg.post_tool_use_failure.push(m.clone());
    cfg.session_start.push(m.clone());
    cfg.session_end.push(m.clone());
    cfg.user_prompt_submit.push(m.clone());
    cfg.notification.push(m.clone());
    cfg.stop.push(m.clone());
    cfg.subagent_start.push(m.clone());
    cfg.subagent_stop.push(m);

    for v in HookEvent::ALL_VARIANTS {
        assert_eq!(cfg.matchers_for_event(*v).len(), 1);
    }
}

fn command_action(cmd: Option<&str>) -> HookAction {
    HookAction {
        hook_type: HookType::Command,
        command: cmd.map(str::to_owned),
        prompt: None,
        r#async: false,
        timeout: None,
        status_message: None,
    }
}

fn prompt_action(prompt: Option<&str>) -> HookAction {
    HookAction {
        hook_type: HookType::Prompt,
        command: None,
        prompt: prompt.map(str::to_owned),
        r#async: false,
        timeout: None,
        status_message: None,
    }
}

#[test]
fn hook_events_config_validate_accepts_well_formed() {
    let mut cfg = HookEventsConfig::default();
    cfg.pre_tool_use.push(HookMatcher {
        matcher: "Bash".to_owned(),
        hooks: vec![command_action(Some("echo hi"))],
    });
    cfg.session_start.push(HookMatcher {
        matcher: "*".to_owned(),
        hooks: vec![prompt_action(Some("Welcome"))],
    });
    cfg.stop.push(HookMatcher {
        matcher: "*".to_owned(),
        hooks: vec![HookAction {
            hook_type: HookType::Agent,
            command: None,
            prompt: None,
            r#async: false,
            timeout: None,
            status_message: None,
        }],
    });

    cfg.validate().expect("validation should pass");
}

#[test]
fn hook_events_config_validate_rejects_command_without_command() {
    let mut cfg = HookEventsConfig::default();
    cfg.pre_tool_use.push(HookMatcher {
        matcher: "Bash".to_owned(),
        hooks: vec![command_action(None)],
    });
    let err = cfg.validate().unwrap_err();
    assert!(format!("{err}").contains("command"));
}

#[test]
fn hook_events_config_validate_rejects_prompt_without_prompt() {
    let mut cfg = HookEventsConfig::default();
    cfg.session_start.push(HookMatcher {
        matcher: "*".to_owned(),
        hooks: vec![prompt_action(None)],
    });
    let err = cfg.validate().unwrap_err();
    assert!(format!("{err}").contains("prompt"));
}

#[test]
fn hook_event_serde_round_trip() {
    let json = serde_json::to_string(&HookEvent::PreToolUse).unwrap();
    assert_eq!(json, "\"PreToolUse\"");
    let parsed: HookEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, HookEvent::PreToolUse);
}

#[test]
fn hook_category_serde_round_trip() {
    let json = serde_json::to_string(&HookCategory::System).unwrap();
    assert_eq!(json, "\"system\"");
    let parsed: HookCategory = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, HookCategory::System);
}

#[test]
fn disk_hook_config_yaml_round_trip_with_defaults() {
    let yaml = r"
event: SessionStart
";
    let cfg: systemprompt_models::services::DiskHookConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.name, "");
    assert_eq!(cfg.version, "1.0.0");
    assert!(cfg.enabled);
    assert_eq!(cfg.matcher, "*");
    assert!(matches!(cfg.category, HookCategory::Custom));
    assert!(cfg.tags.is_empty());
}
