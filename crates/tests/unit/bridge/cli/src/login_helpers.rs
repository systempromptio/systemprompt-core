//! Parsing what a user pastes back from the device-link page, and resolving
//! the gateway and device name a redeemed token is bound to.

use systemprompt_bridge::cli::login::test_api::{
    code_after_flag, default_device_name, extract_code, resolve_gateway, strip_terminal_noise,
};
use tempfile::TempDir;

fn with_config<R>(body: Option<&str>, f: impl FnOnce() -> R) -> R {
    let dir = TempDir::new().expect("config home");
    let brand = dir.path().join("systemprompt");
    std::fs::create_dir_all(&brand).expect("brand dir");
    if let Some(text) = body {
        std::fs::write(brand.join("systemprompt-bridge.toml"), text).expect("config");
    }
    let out = temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(dir.path().display().to_string())),
            ("HOME", Some(dir.path().display().to_string())),
            ("SP_BRIDGE_CONFIG", None),
            ("SP_BRIDGE_PAT", None),
        ],
        f,
    );
    drop(dir);
    out
}

#[test]
fn a_bare_code_is_taken_as_the_code() {
    assert_eq!(extract_code("abc123").expect("a bare code"), "abc123");
}

#[test]
fn surrounding_whitespace_is_trimmed_off_a_pasted_code() {
    assert_eq!(extract_code("  abc123 \t").expect("trimmed"), "abc123");
}

#[test]
fn pasting_nothing_at_all_is_reported_rather_than_accepted_as_an_empty_code() {
    let err = extract_code("   ").expect_err("blank input");
    assert_eq!(err, "nothing pasted");
    assert_eq!(extract_code("").expect_err("empty input"), "nothing pasted");
}

#[test]
fn a_pasted_command_yields_the_value_after_its_code_flag() {
    assert_eq!(
        extract_code("systemprompt-bridge login --code xyz789").expect("code flag"),
        "xyz789"
    );
    assert_eq!(
        extract_code("systemprompt-bridge login --code=xyz789").expect("joined code flag"),
        "xyz789"
    );
}

#[test]
fn the_code_flag_wins_over_a_gateway_url_in_the_same_pasted_command() {
    let pasted = "systemprompt-bridge login --gateway https://gw.invalid/cb?code=WRONG --code RIGHT";
    assert_eq!(
        extract_code(pasted).expect("the flag is read first"),
        "RIGHT",
        "a query-string parse would otherwise wander into the --gateway url"
    );
}

#[test]
fn a_command_with_no_code_flag_is_reported_as_a_command_rather_than_used_as_a_code() {
    let err = extract_code("systemprompt-bridge login --no-browser").expect_err("no code present");
    assert!(err.contains("--code"), "{err}");
    assert!(err.contains("paste just the code"), "{err}");
}

#[test]
fn a_callback_url_yields_its_code_query_parameter() {
    assert_eq!(
        extract_code("https://gw.invalid/cb?code=abc123").expect("code param"),
        "abc123"
    );
    assert_eq!(
        extract_code("https://gw.invalid/cb?state=s&code=abc123&x=1").expect("code param"),
        "abc123"
    );
}

#[test]
fn a_fragment_after_the_code_is_dropped_rather_than_treated_as_part_of_it() {
    assert_eq!(
        extract_code("https://gw.invalid/cb?code=abc123#section").expect("code param"),
        "abc123"
    );
}

#[test]
fn a_callback_url_carrying_an_error_reports_that_the_sign_in_was_refused() {
    let err = extract_code("https://gw.invalid/cb?error=access_denied")
        .expect_err("an error param is a refusal");
    assert!(err.contains("not approved"), "{err}");
    assert!(err.contains("access_denied"), "{err}");
}

#[test]
fn a_url_with_a_query_but_no_code_says_so_rather_than_using_the_url_as_a_code() {
    let err = extract_code("https://gw.invalid/cb?state=abc").expect_err("no code parameter");
    assert!(err.contains("no `code` parameter"), "{err}");
}

#[test]
fn an_empty_code_parameter_is_not_accepted_as_a_code() {
    let err = extract_code("https://gw.invalid/cb?code=").expect_err("empty code param");
    assert!(err.contains("no `code` parameter"), "{err}");
}

#[test]
fn a_bracketed_paste_wrapper_is_stripped_before_the_code_is_read() {
    let wrapped = "\u{1b}[200~abc123\u{1b}[201~";
    assert_eq!(strip_terminal_noise(wrapped), "abc123");
    assert_eq!(
        extract_code(wrapped).expect("the wrapper is not part of the code"),
        "abc123"
    );
}

#[test]
fn a_non_csi_escape_sequence_drops_both_of_its_characters() {
    assert_eq!(strip_terminal_noise("a\u{1b}Zb"), "ab");
}

#[test]
fn control_characters_are_dropped_but_whitespace_is_kept() {
    assert_eq!(strip_terminal_noise("a\u{7}b"), "ab");
    assert_eq!(strip_terminal_noise("a b\tc\nd"), "a b\tc\nd");
}

#[test]
fn text_with_no_terminal_noise_passes_through_unchanged() {
    assert_eq!(strip_terminal_noise("plain-code-123"), "plain-code-123");
}

#[test]
fn the_code_flag_scan_finds_nothing_when_no_flag_is_present() {
    assert_eq!(code_after_flag("just some words"), None);
    assert_eq!(code_after_flag(""), None);
}

#[test]
fn a_code_flag_with_no_value_after_it_yields_nothing_rather_than_an_empty_code() {
    assert_eq!(code_after_flag("bridge login --code"), None);
    assert_eq!(code_after_flag("bridge login --code="), None);
}

#[test]
fn the_first_code_flag_wins_when_a_command_carries_two() {
    assert_eq!(
        code_after_flag("bridge login --code first --code second"),
        Some("first".to_owned())
    );
}

#[test]
fn with_no_override_the_gateway_comes_from_the_written_config() {
    let resolved = with_config(Some("gateway_url = \"https://gw.invalid:7000\"\n"), || {
        resolve_gateway(None)
    })
    .expect("the configured gateway resolves");
    assert!(
        resolved.as_str().contains("gw.invalid"),
        "got {}",
        resolved.as_str()
    );
}

#[test]
fn an_explicit_gateway_override_wins_over_the_config() {
    let resolved = with_config(Some("gateway_url = \"https://configured.invalid\"\n"), || {
        resolve_gateway(Some("https://override.invalid:8443"))
    })
    .expect("the override resolves");
    assert!(
        resolved.as_str().contains("override.invalid"),
        "got {}",
        resolved.as_str()
    );
}

#[test]
fn surrounding_whitespace_on_a_gateway_override_is_trimmed_before_validation() {
    let resolved = with_config(None, || resolve_gateway(Some("  https://gw.invalid  ")))
        .expect("a padded url still validates");
    assert!(resolved.as_str().contains("gw.invalid"));
}

#[test]
fn a_gateway_override_that_is_not_a_url_is_refused_and_the_error_names_the_flag() {
    let err = with_config(None, || resolve_gateway(Some("not a url")))
        .expect_err("a malformed override is refused");
    assert!(err.starts_with("--gateway: "), "got {err}");
}

#[test]
fn an_empty_gateway_override_is_refused_rather_than_falling_back_to_the_config() {
    let err = with_config(Some("gateway_url = \"https://configured.invalid\"\n"), || {
        resolve_gateway(Some("   "))
    })
    .expect_err("an empty override is refused, not ignored");
    assert!(err.starts_with("--gateway: "), "got {err}");
}

#[test]
fn the_device_name_comes_from_the_hostname_environment_when_it_is_set() {
    let name = temp_env::with_var("HOSTNAME", Some("  workstation-7  "), default_device_name);
    assert_eq!(name, Some("workstation-7".to_owned()));
}

#[test]
fn a_blank_hostname_environment_value_is_not_used_as_a_device_name() {
    let name = temp_env::with_var("HOSTNAME", Some("   "), default_device_name);
    assert_ne!(
        name,
        Some(String::new()),
        "a blank hostname must fall through rather than name the device the empty string"
    );
    if let Some(name) = name {
        assert!(!name.trim().is_empty(), "fell back to a real name: {name}");
    }
}

#[test]
fn with_no_hostname_environment_the_device_name_falls_back_to_etc_hostname_or_nothing() {
    let name = temp_env::with_var("HOSTNAME", None::<&str>, default_device_name);
    match name {
        Some(name) => {
            assert!(!name.is_empty());
            assert_eq!(name, name.trim(), "the fallback is trimmed");
            let on_disk = std::fs::read_to_string("/etc/hostname").expect("read /etc/hostname");
            assert_eq!(name, on_disk.trim());
        },
        None => assert!(
            std::fs::read_to_string("/etc/hostname")
                .map(|h| h.trim().is_empty())
                .unwrap_or(true),
            "None is only correct when /etc/hostname is absent or blank"
        ),
    }
}
