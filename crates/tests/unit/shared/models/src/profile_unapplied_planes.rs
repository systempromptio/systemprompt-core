//! `governance.unapplied_planes`: absent means `warn`, `deny` parses, and an
//! unknown value is refused when the profile is read.

use systemprompt_manifest::profile::{GovernanceConfig, UnappliedPlaneMode};

fn parse(yaml: &str) -> Result<GovernanceConfig, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

#[test]
fn an_absent_setting_is_warn() {
    let governance = parse("audit: {}\n").expect("parses");
    assert_eq!(governance.unapplied_planes, UnappliedPlaneMode::Warn);
    assert_eq!(UnappliedPlaneMode::default().as_str(), "warn");
}

#[test]
fn deny_parses_and_denies() {
    let governance = parse("unapplied_planes: deny\n").expect("parses");
    assert_eq!(governance.unapplied_planes, UnappliedPlaneMode::Deny);
    assert!(governance.unapplied_planes.denies());
}

#[test]
fn an_unknown_value_is_refused() {
    let error = parse("unapplied_planes: open\n").expect_err("unknown value");
    let text = error.to_string();
    assert!(text.contains("unknown variant `open`"), "{text}");
    assert!(text.contains("warn") && text.contains("deny"), "{text}");
}
