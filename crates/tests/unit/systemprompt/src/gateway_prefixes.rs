//! The kernel's built-in gateway prefixes equal the baseline captured from
//! the gateway's prefix list (T22).

use systemprompt_extension::reserved::BUILTIN_GATEWAY_PREFIXES;

const BASELINE: &str = include_str!("../fixtures/baseline/gateway_prefixes.txt");

#[test]
fn builtin_gateway_prefixes_equal_the_baseline() {
    let baseline: Vec<&str> = BASELINE.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(baseline.len(), 31);
    assert_eq!(BUILTIN_GATEWAY_PREFIXES.as_slice(), baseline.as_slice());
}
