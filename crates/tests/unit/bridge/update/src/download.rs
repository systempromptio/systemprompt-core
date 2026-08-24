use systemprompt_bridge::update::{DownloadProgress, hex_lower};

#[test]
fn hex_is_lowercase_and_zero_padded() {
    assert_eq!(hex_lower(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
}

#[test]
fn fraction_is_clamped_and_safe_at_zero_total() {
    assert!(
        (DownloadProgress {
            received: 5,
            total: 0
        }
        .fraction()
            - 0.0)
            .abs()
            < f64::EPSILON
    );
    assert!(
        (DownloadProgress {
            received: 99,
            total: 10
        }
        .fraction()
            - 1.0)
            .abs()
            < f64::EPSILON
    );
}
