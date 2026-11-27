use systemprompt_device::protocol::ids::AccountId;

use crate::accounts::support::{account, open_plain, store_path, write_store};

fn keys_for(accounts: &[serde_json::Value]) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let ids: Vec<String> = accounts
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    write_store(
        &path,
        &serde_json::json!({ "version": 1, "accounts": accounts }),
    );
    let opened = open_plain(&path);
    ids.iter()
        .map(|id| {
            opened
                .instance_key(&AccountId::new(id.as_str()))
                .unwrap()
                .into_inner()
        })
        .collect()
}

fn labelled(labels: &[&str]) -> Vec<serde_json::Value> {
    labels
        .iter()
        .enumerate()
        .map(|(index, label)| account(&format!("a{index}"), label))
        .collect()
}

fn keyed(id: &str, label: &str, key: &str) -> serde_json::Value {
    let mut entry = account(id, label);
    entry["instance_key"] = serde_json::json!(key);
    entry
}

fn key_of(label: &str) -> String {
    keys_for(&labelled(&[label])).remove(0)
}

#[test]
fn labels_become_lowercase_dashed_keys() {
    assert_eq!(key_of("Astound Digital"), "astound-digital");
    assert_eq!(key_of("  sp.example.com:8443 "), "sp-example-com-8443");
    assert_eq!(key_of("Ünïcode — only"), "n-code-only");
    assert_eq!(key_of("!!!"), "instance");
    assert_eq!(key_of(""), "instance");
    let long = key_of(&"abc ".repeat(20));
    assert!(long.len() <= 32);
    assert!(long.starts_with("abc-abc"));
    assert!(!long.ends_with('-'));
}

#[test]
fn keys_are_unique_among_the_accounts() {
    assert_eq!(
        keys_for(&labelled(&["Alpha", "Alpha", "Alpha"])),
        ["alpha", "alpha-2", "alpha-3"]
    );
    assert_eq!(keys_for(&labelled(&["", "!"])), ["instance", "instance-2"]);
    assert_eq!(keys_for(&labelled(&["alpha", "Beta"])), ["alpha", "beta"]);
    let taken = vec![
        keyed("k1", "One", "alpha"),
        keyed("k2", "Two", "alpha-2"),
        keyed("k3", "Three", "alpha-3"),
        account("k4", "Alpha"),
    ];
    assert_eq!(keys_for(&taken), ["alpha", "alpha-2", "alpha-3", "alpha-4"]);
}

#[test]
fn a_duplicate_or_empty_stored_key_is_replaced() {
    let stored = vec![
        keyed("d1", "Alpha", "alpha"),
        keyed("d2", "Alpha", "alpha"),
        keyed("d3", "Beta", ""),
    ];
    assert_eq!(keys_for(&stored), ["alpha", "alpha-2", "beta"]);
}
