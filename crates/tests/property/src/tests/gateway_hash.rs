use proptest::prelude::*;
use systemprompt_models::gateway_hash::{conversation_prefix_hash, fnv1a_segments};

proptest! {
    // Determinism: identical inputs always yield identical hashes.
    #[test]
    fn conversation_prefix_hash_is_deterministic(
        system in proptest::option::of("[\\PC]{0,128}"),
        role in "[a-z]{1,16}",
        content in "[\\PC]{0,512}",
    ) {
        let a = conversation_prefix_hash(system.as_deref(), &role, &content);
        let b = conversation_prefix_hash(system.as_deref(), &role, &content);
        prop_assert_eq!(a, b);
    }

    // Domain separation: shifting a byte across the role/content boundary
    // must not collide. This is the regression guard against a naive
    // concatenated hash implementation.
    #[test]
    fn role_content_boundary_is_preserved(
        role_head in "[a-z]{1,8}",
        suffix in "[a-z]{1,8}",
        content_tail in "[a-z]{1,8}",
    ) {
        let role_a = role_head.clone();
        let content_a = format!("{suffix}{content_tail}");
        let role_b = format!("{role_head}{suffix}");
        let content_b = content_tail.clone();
        let h_a = conversation_prefix_hash(None, &role_a, &content_a);
        let h_b = conversation_prefix_hash(None, &role_b, &content_b);
        prop_assert_ne!(
            h_a,
            h_b,
            "moving bytes from content into role must change the hash"
        );
    }

    // Domain separation across system/role boundary.
    #[test]
    fn system_role_boundary_is_preserved(
        sys_head in "[a-z]{1,8}",
        suffix in "[a-z]{1,8}",
        role_tail in "[a-z]{1,8}",
        content in "[a-z]{0,16}",
    ) {
        let sys_a = sys_head.clone();
        let role_a = format!("{suffix}{role_tail}");
        let sys_b = format!("{sys_head}{suffix}");
        let role_b = role_tail.clone();
        let h_a = conversation_prefix_hash(Some(&sys_a), &role_a, &content);
        let h_b = conversation_prefix_hash(Some(&sys_b), &role_b, &content);
        prop_assert_ne!(h_a, h_b);
    }

    // Length-prefix mixing: appending bytes never produces the same hash
    // as the shorter prefix (modulo astronomically improbable collisions,
    // which proptest will not synthesise with these inputs).
    #[test]
    fn appending_content_changes_hash(
        prefix in "[a-z]{1,32}",
        extra in "[a-z]{1,16}",
    ) {
        let appended = format!("{prefix}{extra}");
        let h_a = conversation_prefix_hash(None, "user", &prefix);
        let h_b = conversation_prefix_hash(None, "user", &appended);
        prop_assert_ne!(h_a, h_b);
    }

    // Segment-order sensitivity at the fnv1a layer.
    #[test]
    fn segment_order_matters(
        a in "[a-z]{1,12}",
        b in "[a-z]{1,12}",
    ) {
        prop_assume!(a != b);
        let h_ab = fnv1a_segments(&[("x", a.as_bytes()), ("x", b.as_bytes())]);
        let h_ba = fnv1a_segments(&[("x", b.as_bytes()), ("x", a.as_bytes())]);
        prop_assert_ne!(h_ab, h_ba);
    }
}
