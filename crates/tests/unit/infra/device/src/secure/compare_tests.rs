use systemprompt_device::secure::constant_time_eq;

#[test]
fn equal_slices_compare_equal() {
    assert!(constant_time_eq(b"", b""));
    assert!(constant_time_eq(b"sp_device_abc", b"sp_device_abc"));
}

#[test]
fn differing_slices_compare_unequal() {
    assert!(!constant_time_eq(b"sp_device_abc", b"sp_device_abd"));
    assert!(!constant_time_eq(b"abc", b"abcd"));
    assert!(!constant_time_eq(b"", b"a"));
}
