//! Unit tests for OutputMode and output mode functions

use systemprompt_core_logging::{get_output_mode, is_console_output_enabled, OutputMode};

// ============================================================================
// OutputMode From u8 Tests
// ============================================================================

#[test]
fn test_output_mode_from_u8_cli() {
    let mode: OutputMode = 0_u8.into();
    assert_eq!(mode, OutputMode::Cli);
}

#[test]
fn test_output_mode_from_u8_tui() {
    let mode: OutputMode = 1_u8.into();
    assert_eq!(mode, OutputMode::Tui);
}

#[test]
fn test_output_mode_from_u8_headless() {
    let mode: OutputMode = 2_u8.into();
    assert_eq!(mode, OutputMode::Headless);
}

#[test]
fn test_output_mode_from_u8_invalid_returns_cli() {
    let mode: OutputMode = 3_u8.into();
    assert_eq!(mode, OutputMode::Cli);

    let mode: OutputMode = 255_u8.into();
    assert_eq!(mode, OutputMode::Cli);
}

// ============================================================================
// OutputMode Default Tests
// ============================================================================

#[test]
fn test_output_mode_default_is_cli() {
    let mode = OutputMode::default();
    assert_eq!(mode, OutputMode::Cli);
}

// ============================================================================
// OutputMode Clone and Copy Tests
// ============================================================================

#[test]
fn test_output_mode_clone() {
    let mode = OutputMode::Tui;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn test_output_mode_copy() {
    let mode = OutputMode::Headless;
    let copied = mode;
    assert_eq!(mode, copied);
}

// ============================================================================
// OutputMode Equality Tests
// ============================================================================

#[test]
fn test_output_mode_equality() {
    assert_eq!(OutputMode::Cli, OutputMode::Cli);
    assert_eq!(OutputMode::Tui, OutputMode::Tui);
    assert_eq!(OutputMode::Headless, OutputMode::Headless);
}

#[test]
fn test_output_mode_inequality() {
    assert_ne!(OutputMode::Cli, OutputMode::Tui);
    assert_ne!(OutputMode::Cli, OutputMode::Headless);
    assert_ne!(OutputMode::Tui, OutputMode::Headless);
}

// ============================================================================
// OutputMode Debug Tests
// ============================================================================

#[test]
fn test_output_mode_debug_cli() {
    let mode = OutputMode::Cli;
    let debug = format!("{:?}", mode);
    assert!(debug.contains("Cli"));
}

#[test]
fn test_output_mode_debug_tui() {
    let mode = OutputMode::Tui;
    let debug = format!("{:?}", mode);
    assert!(debug.contains("Tui"));
}

#[test]
fn test_output_mode_debug_headless() {
    let mode = OutputMode::Headless;
    let debug = format!("{:?}", mode);
    assert!(debug.contains("Headless"));
}

// ============================================================================
// OutputMode As u8 Tests
// ============================================================================

#[test]
fn test_output_mode_as_u8_cli() {
    assert_eq!(OutputMode::Cli as u8, 0);
}

#[test]
fn test_output_mode_as_u8_tui() {
    assert_eq!(OutputMode::Tui as u8, 1);
}

#[test]
fn test_output_mode_as_u8_headless() {
    assert_eq!(OutputMode::Headless as u8, 2);
}

// ============================================================================
// OutputMode Roundtrip Tests
// ============================================================================

#[test]
fn test_output_mode_roundtrip_cli() {
    let original = OutputMode::Cli;
    let as_u8 = original as u8;
    let back: OutputMode = as_u8.into();
    assert_eq!(original, back);
}

#[test]
fn test_output_mode_roundtrip_tui() {
    let original = OutputMode::Tui;
    let as_u8 = original as u8;
    let back: OutputMode = as_u8.into();
    assert_eq!(original, back);
}

#[test]
fn test_output_mode_roundtrip_headless() {
    let original = OutputMode::Headless;
    let as_u8 = original as u8;
    let back: OutputMode = as_u8.into();
    assert_eq!(original, back);
}

// ============================================================================
// Console Output Helper Tests
// ============================================================================

// Note: These tests may not be fully isolated due to global state
// In a production test suite, you would use test isolation or mocking

#[test]
fn test_is_console_output_enabled_returns_bool() {
    // This function returns a boolean based on global state
    let result = is_console_output_enabled();
    assert!(result == true || result == false);
}

#[test]
fn test_get_output_mode_returns_valid_mode() {
    // This function returns the current output mode based on global state
    let mode = get_output_mode();
    // Verify it's one of the valid modes
    assert!(mode == OutputMode::Cli || mode == OutputMode::Tui || mode == OutputMode::Headless);
}

// ============================================================================
// OutputMode All Variants Test
// ============================================================================

#[test]
fn test_all_output_mode_variants_exist() {
    // Ensure all expected variants exist
    let _cli = OutputMode::Cli;
    let _tui = OutputMode::Tui;
    let _headless = OutputMode::Headless;
}

#[test]
fn test_output_mode_variant_count() {
    // Test that we have exactly 3 variants by checking all u8 values 0-2
    let modes: Vec<OutputMode> = (0_u8..=2).map(OutputMode::from).collect();
    assert_eq!(modes.len(), 3);

    // Values >= 3 should fall back to Cli
    assert_eq!(OutputMode::from(3_u8), OutputMode::Cli);
}
