use systemprompt_bridge::proxy::secret;

#[test]
fn fingerprint_of_empty_is_marker() {
    assert_eq!(secret::fingerprint(""), "<empty>");
}

#[test]
fn fingerprint_is_eight_lowercase_hex() {
    let fp = secret::fingerprint("a6ee3c83-some-loopback-secret-value");
    assert_eq!(fp.len(), 8);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn fingerprint_is_deterministic_and_distinguishes() {
    let a = secret::fingerprint("secret-one");
    let b = secret::fingerprint("secret-one");
    let c = secret::fingerprint("secret-two");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn reapply_hint_directs_to_reapply_not_client_restart() {
    let hint = secret::reapply_hint();
    assert!(
        hint.contains("re-apply"),
        "hint must direct to re-apply: {hint}"
    );
    assert!(
        !hint.to_ascii_lowercase().contains("restart claude desktop"),
        "hint must not advise restarting the client: {hint}"
    );
}
