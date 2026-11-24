use systemprompt_manifest::profile::{MAX_MEMORY_CAPACITY, RateLimitsConfig};

#[test]
fn an_omitted_capacity_is_sixty_four_shards_of_four_thousand_ninety_six() {
    let limits: RateLimitsConfig = serde_yaml::from_str("disabled: false").expect("parse");
    assert_eq!(limits.memory_capacity, 64 * 4096);
    assert_eq!(RateLimitsConfig::default().memory_capacity, 64 * 4096);
    assert!(limits.memory_capacity_error().is_none());
}

#[test]
fn a_configured_capacity_round_trips() {
    let limits: RateLimitsConfig = serde_yaml::from_str("memory_capacity: 1000").expect("parse");
    assert_eq!(limits.memory_capacity, 1000);
    let written = serde_yaml::to_string(&limits).expect("write");
    let read: RateLimitsConfig = serde_yaml::from_str(&written).expect("reparse");
    assert_eq!(read.memory_capacity, 1000);
}

#[test]
fn zero_and_values_past_the_bound_are_refused() {
    let mut limits = RateLimitsConfig::default();
    limits.memory_capacity = 0;
    assert!(limits.memory_capacity_error().is_some());
    limits.memory_capacity = MAX_MEMORY_CAPACITY + 1;
    assert!(limits.memory_capacity_error().is_some());
    limits.memory_capacity = MAX_MEMORY_CAPACITY;
    assert!(limits.memory_capacity_error().is_none());
}
