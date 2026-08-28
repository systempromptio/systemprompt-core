use serde_json::json;
use systemprompt_bridge::validate::{CheckLevel, CheckLine, ValidationReport};

fn line(level: CheckLevel, label: &str, value: &str) -> CheckLine {
    CheckLine {
        level,
        label: label.into(),
        value: value.into(),
    }
}

#[test]
fn report_serialises_every_level_in_lowercase() {
    let report = ValidationReport {
        lines: vec![
            line(CheckLevel::Ok, "gateway_url", "http://127.0.0.1:8081"),
            line(CheckLevel::Warn, "cached token", "absent"),
            line(CheckLevel::Fail, "pinned manifest pubkey", "not pinned"),
            line(CheckLevel::Info, "binary", "v0.31.0"),
        ],
        any_failed: true,
    };

    let value = serde_json::to_value(&report).expect("report serialises");

    assert_eq!(value["any_failed"], json!(true));
    let levels: Vec<&str> = value["lines"]
        .as_array()
        .expect("lines is an array")
        .iter()
        .map(|l| l["level"].as_str().expect("level is a string"))
        .collect();
    assert_eq!(levels, vec!["ok", "warn", "fail", "info"]);
}

// The panel keys off `label`/`value` verbatim, so a rename here silently
// empties every row rather than failing to compile.
#[test]
fn each_line_carries_its_label_and_value() {
    let report = ValidationReport {
        lines: vec![line(
            CheckLevel::Fail,
            "gateway /health",
            "connection refused",
        )],
        any_failed: true,
    };

    let value = serde_json::to_value(&report).expect("report serialises");

    assert_eq!(value["lines"][0]["label"], json!("gateway /health"));
    assert_eq!(value["lines"][0]["value"], json!("connection refused"));
}
