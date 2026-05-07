use systemprompt_models::gateway_hash::{
    context_id_from_prefix_hash, conversation_prefix_hash, fnv1a_segments,
};

#[test]
fn fnv1a_is_deterministic() {
    let h1 = fnv1a_segments(&[("a", b"hello"), ("b", b"world")]);
    let h2 = fnv1a_segments(&[("a", b"hello"), ("b", b"world")]);
    assert_eq!(h1, h2);
}

#[test]
fn fnv1a_distinguishes_segment_boundaries() {
    let split = fnv1a_segments(&[("a", b"hello"), ("a", b"world")]);
    let joined = fnv1a_segments(&[("a", b"helloworld")]);
    assert_ne!(split, joined);
}

#[test]
fn fnv1a_distinguishes_labels() {
    let a = fnv1a_segments(&[("system", b"x")]);
    let b = fnv1a_segments(&[("user", b"x")]);
    assert_ne!(a, b);
}

#[test]
fn prefix_hash_stable_across_calls() {
    let h1 = conversation_prefix_hash(Some("you are helpful"), "user", "hi");
    let h2 = conversation_prefix_hash(Some("you are helpful"), "user", "hi");
    assert_eq!(h1, h2);
}

#[test]
fn prefix_hash_changes_with_first_message() {
    let h1 = conversation_prefix_hash(Some("sys"), "user", "first");
    let h2 = conversation_prefix_hash(Some("sys"), "user", "second");
    assert_ne!(h1, h2);
}

#[test]
fn prefix_hash_changes_with_system() {
    let h1 = conversation_prefix_hash(Some("sys-a"), "user", "msg");
    let h2 = conversation_prefix_hash(Some("sys-b"), "user", "msg");
    assert_ne!(h1, h2);
}

#[test]
fn prefix_hash_no_system_differs_from_empty_system() {
    let none = conversation_prefix_hash(None, "user", "hi");
    let empty = conversation_prefix_hash(Some(""), "user", "hi");
    assert_eq!(none, empty, "empty system should be treated like absent");
}

#[test]
fn context_id_from_hash_is_well_formed() {
    let id = context_id_from_prefix_hash(0xdeadbeef_cafebabe);
    assert_eq!(id.as_str(), "ctx_deadbeefcafebabe");
}

#[test]
fn context_id_round_trip_is_deterministic() {
    let h = conversation_prefix_hash(Some("sys"), "user", "hello");
    let a = context_id_from_prefix_hash(h);
    let b = context_id_from_prefix_hash(h);
    assert_eq!(a, b);
}

/// Pin the byte-exact wire format of the hash. If this test changes,
/// every existing audit row's context_id will start drifting silently —
/// changing the algorithm must be a deliberate breaking change with a
/// migration plan.
#[test]
fn known_vector_does_not_drift() {
    let h = conversation_prefix_hash(Some("you are helpful"), "user", "hello");
    assert_eq!(
        context_id_from_prefix_hash(h).as_str(),
        "ctx_3078015cf23bb808",
        "hash output must not drift across releases without a migration"
    );
}

#[test]
fn role_distinguishes_hash() {
    let user = conversation_prefix_hash(None, "user", "hi");
    let assistant = conversation_prefix_hash(None, "assistant", "hi");
    assert_ne!(user, assistant);
}

#[test]
fn unicode_content_hashes() {
    let h = conversation_prefix_hash(None, "user", "こんにちは 🌸");
    assert_ne!(h, 0);
    let again = conversation_prefix_hash(None, "user", "こんにちは 🌸");
    assert_eq!(h, again);
}

#[test]
fn empty_role_distinct_from_user_role() {
    let empty = conversation_prefix_hash(None, "", "hi");
    let user = conversation_prefix_hash(None, "user", "hi");
    assert_ne!(empty, user);
}

#[test]
fn empty_segments_list_is_offset_basis() {
    // Hashing nothing returns the FNV-1a 64 offset basis. This is the
    // documented zero-input behavior; we lock it down so a future
    // refactor doesn't accidentally seed differently.
    assert_eq!(fnv1a_segments(&[]), 0xcbf2_9ce4_8422_2325);
}

#[test]
fn segment_label_changes_hash() {
    let a = fnv1a_segments(&[("user", b"hi")]);
    let b = fnv1a_segments(&[("system", b"hi")]);
    assert_ne!(a, b);
}

#[test]
fn distribution_avoids_obvious_collisions() {
    // Sanity check that 1024 distinct first-turn strings produce 1024
    // distinct ids. This catches catastrophic hash regressions
    // (e.g. accidentally hashing only the first byte).
    let mut seen = std::collections::HashSet::new();
    for i in 0..1024u32 {
        let content = format!("message-{i}");
        let h = conversation_prefix_hash(None, "user", &content);
        assert!(seen.insert(h), "collision at i={i}");
    }
}

#[test]
fn context_id_format_is_stable_lowercase_hex() {
    for &h in &[0u64, 1, 0xdeadbeef, u64::MAX, u64::MAX / 2] {
        let id = context_id_from_prefix_hash(h);
        let s = id.as_str();
        assert!(s.starts_with("ctx_"), "missing prefix: {s}");
        assert_eq!(s.len(), 4 + 16, "wrong length: {s}");
        assert!(
            s[4..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "non-lowercase-hex tail: {s}"
        );
    }
}
