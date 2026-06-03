use systemprompt_bridge::cli::args::{has_flag, parse_opt_flag};

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parse_opt_flag_finds_value() {
    let a = args(&["bin", "sub", "--host", "codex-cli"]);
    assert_eq!(parse_opt_flag(&a, "--host"), Some("codex-cli".to_owned()));
}

#[test]
fn parse_opt_flag_absent_returns_none() {
    let a = args(&["bin", "sub", "--other", "value"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_last_arg_without_value_returns_none() {
    let a = args(&["bin", "sub", "--host"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_ignores_index_below_two() {
    let a = args(&["bin", "--host", "ignored"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_returns_first_match_when_repeated() {
    let a = args(&["bin", "sub", "--host", "first", "--host", "second"]);
    assert_eq!(parse_opt_flag(&a, "--host"), Some("first".to_owned()));
}

#[test]
fn has_flag_true_when_present() {
    let a = args(&["bin", "sub", "--apply"]);
    assert!(has_flag(&a, "--apply"));
}

#[test]
fn has_flag_false_when_absent() {
    let a = args(&["bin", "sub", "--other"]);
    assert!(!has_flag(&a, "--apply"));
}

#[test]
fn has_flag_false_when_only_at_index_one() {
    let a = args(&["bin", "--apply"]);
    assert!(!has_flag(&a, "--apply"));
}
