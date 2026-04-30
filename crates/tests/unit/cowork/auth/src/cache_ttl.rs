use systemprompt_bridge::auth::cache::is_still_valid;

const NOW: u64 = 1_700_000_000;
const THRESHOLD: u64 = 30;

#[test]
fn token_with_threshold_minus_one_is_invalid() {
    let expires_at = NOW + THRESHOLD - 1;
    assert!(
        !is_still_valid(expires_at, NOW, THRESHOLD),
        "expires_at == now + threshold - 1 must be considered expired"
    );
}

#[test]
fn token_at_exact_threshold_is_invalid() {
    let expires_at = NOW + THRESHOLD;
    assert!(
        !is_still_valid(expires_at, NOW, THRESHOLD),
        "boundary: expires_at == now + threshold must reject (strict >)"
    );
}

#[test]
fn token_one_second_past_threshold_is_valid() {
    let expires_at = NOW + THRESHOLD + 1;
    assert!(
        is_still_valid(expires_at, NOW, THRESHOLD),
        "expires_at == now + threshold + 1 must be considered fresh"
    );
}

#[test]
fn already_expired_token_is_invalid() {
    let expires_at = NOW - 60;
    assert!(!is_still_valid(expires_at, NOW, 0));
}

#[test]
fn fresh_token_with_zero_threshold_is_valid() {
    let expires_at = NOW + 1;
    assert!(is_still_valid(expires_at, NOW, 0));
}

#[test]
fn fresh_token_at_now_with_zero_threshold_is_invalid() {
    assert!(!is_still_valid(NOW, NOW, 0));
}

#[test]
fn saturating_threshold_does_not_panic() {
    assert!(!is_still_valid(0, u64::MAX - 10, 100));
    assert!(!is_still_valid(u64::MAX, u64::MAX, 1));
}
