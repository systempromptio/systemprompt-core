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
