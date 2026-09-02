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

#[test]
fn fraction_reports_the_ratio_received() {
    let progress = DownloadProgress {
        received: 25,
        total: 100,
    };
    assert!((progress.fraction() - 0.25).abs() < f64::EPSILON);
}

#[test]
fn fraction_is_zero_before_any_bytes_arrive() {
    let progress = DownloadProgress {
        received: 0,
        total: 4096,
    };
    assert!((progress.fraction() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn fraction_scales_to_the_percentage_the_cli_prints() {
    let progress = DownloadProgress {
        received: 3,
        total: 8,
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "mirrors the CLI progress reporter"
    )]
    let pct = (progress.fraction() * 100.0) as u8;
    assert_eq!(pct, 37);
}

#[test]
fn hex_of_an_empty_slice_is_empty() {
    assert_eq!(hex_lower(&[]), "");
}

#[test]
fn hex_of_a_sha256_digest_is_sixty_four_lowercase_chars() {
    let digest = [0xabu8; 32];
    let hex = hex_lower(&digest);
    assert_eq!(hex.len(), 64);
    assert_eq!(hex, "ab".repeat(32));
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );
}

#[test]
fn hex_output_compares_case_insensitively_against_an_uppercase_manifest_digest() {
    let hex = hex_lower(&[0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(hex, "deadbeef");
    assert!(hex.eq_ignore_ascii_case("DEADBEEF"));
    assert!(!hex.eq_ignore_ascii_case("deadbeee"));
}
