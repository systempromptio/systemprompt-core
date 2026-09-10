use systemprompt_models::services::{DEFAULT_RULE_CONTENT_FILE, DiskRuleConfig};

fn parse(yaml: &str) -> DiskRuleConfig {
    serde_yaml::from_str(yaml).expect("rule config parses")
}

#[test]
fn an_omitted_file_key_resolves_to_the_default_content_file() {
    let config = parse("id: security\nname: Security\ndescription: d\n");

    assert!(config.file.is_empty());
    assert_eq!(config.content_file(), DEFAULT_RULE_CONTENT_FILE);
}

#[test]
fn an_explicit_file_key_wins_over_the_default() {
    let config = parse("id: security\nname: Security\ndescription: d\nfile: RULE.md\n");

    assert_eq!(config.content_file(), "RULE.md");
}

#[test]
fn enabled_defaults_to_true_and_the_lists_default_to_empty() {
    let config = parse("id: security\nname: Security\ndescription: d\n");

    assert!(config.enabled);
    assert!(config.tags.is_empty());
    assert!(config.hosts.is_empty());
}

#[test]
fn enabled_false_is_honoured() {
    let config = parse("id: security\nname: Security\ndescription: d\nenabled: false\n");

    assert!(!config.enabled);
}

#[test]
fn empty_optional_fields_are_omitted_from_the_serialised_descriptor() {
    let config = parse("id: security\nname: Security\ndescription: d\n");
    let text = serde_yaml::to_string(&config).expect("serialises");

    assert!(!text.contains("file:"), "{text}");
    assert!(!text.contains("tags:"), "{text}");
    assert!(!text.contains("hosts:"), "{text}");
    assert!(text.contains("id: security"), "{text}");
}

#[test]
fn a_populated_descriptor_round_trips_through_yaml() {
    let config = parse(
        "id: security\nname: Security\ndescription: d\nfile: RULE.md\ntags: [a]\nhosts: \
         [claude-code]\n",
    );
    let text = serde_yaml::to_string(&config).expect("serialises");
    let back = parse(&text);

    assert_eq!(back.id.as_str(), "security");
    assert_eq!(back.content_file(), "RULE.md");
    assert_eq!(back.tags, vec!["a"]);
    assert_eq!(back.hosts, vec!["claude-code"]);
}
