use serde_yaml::{Mapping, Value};

pub(crate) fn authored_frontmatter() -> Mapping {
    let mut map = Mapping::new();
    for (key, value) in [
        ("allowed-tools", "Bash(git:*)"),
        ("title", "ignored"),
        ("model", "sonnet"),
    ] {
        map.insert(Value::String(key.into()), Value::String(value.into()));
    }
    map
}

pub(crate) fn assert_passthrough_block(written: &str, name: &str) {
    let rest = written
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("no opening fence: {written}"));
    let block = &rest[..rest.find("\n---\n").expect("closing fence")];
    let lines: Vec<&str> = block.lines().collect();
    assert_eq!(
        lines.first(),
        Some(&format!("name: {name}").as_str()),
        "{written}"
    );
    assert!(lines[1].starts_with("description: "), "{written}");
    let tail = &lines[2..];
    assert!(tail.contains(&"allowed-tools: Bash(git:*)"), "{written}");
    assert!(tail.contains(&"model: sonnet"), "{written}");
    assert!(
        !block.lines().any(|l| l.starts_with("title:")),
        "platform-owned keys must not reach the host: {written}"
    );
}
