use systemprompt_cli::cloud::tenant::swap_to_external_host;

#[test]
fn swap_to_external_host_replaces_sandbox_host() {
    let url = "postgres://user:pass@db.sandbox.flycast/db?sslmode=disable";
    let swapped = swap_to_external_host(url);
    assert_ne!(swapped, url);
    assert!(!swapped.contains("flycast"));
}

#[test]
fn swap_to_external_host_replaces_production_host() {
    let url = "postgres://user:pass@db.internal.flycast/db";
    let swapped = swap_to_external_host(url);
    assert_ne!(swapped, url);
    assert!(swapped.contains("user"));
    assert!(swapped.contains("pass"));
}

#[test]
fn swap_to_external_host_with_invalid_url_returns_input() {
    let url = "not a url";
    let swapped = swap_to_external_host(url);
    assert_eq!(swapped, url);
}

#[test]
fn swap_to_external_host_preserves_credentials() {
    let url = "postgres://alice:secret@example.com:5432/dbname?option=1";
    let swapped = swap_to_external_host(url);
    assert!(swapped.contains("alice"));
    assert!(swapped.contains("secret"));
    assert!(swapped.contains("5432"));
}
