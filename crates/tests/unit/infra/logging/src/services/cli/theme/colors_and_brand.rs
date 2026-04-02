//! Unit tests for Colors and BrandColors

use systemprompt_logging::services::cli::theme::{
    BrandColors, Colors, ItemStatus, MessageLevel,
};

// ============================================================================
// Colors Tests
// ============================================================================

#[test]
fn test_colors_success_contains_text() {
    let styled = Colors::success("hello");
    assert!(styled.to_string().contains("hello"));
}

#[test]
fn test_colors_warning_contains_text() {
    let styled = Colors::warning("alert");
    assert!(styled.to_string().contains("alert"));
}

#[test]
fn test_colors_error_contains_text() {
    let styled = Colors::error("fail");
    assert!(styled.to_string().contains("fail"));
}

#[test]
fn test_colors_info_contains_text() {
    let styled = Colors::info("note");
    assert!(styled.to_string().contains("note"));
}

#[test]
fn test_colors_highlight_contains_text() {
    let styled = Colors::highlight("important");
    assert!(styled.to_string().contains("important"));
}

#[test]
fn test_colors_dim_contains_text() {
    let styled = Colors::dim("faded");
    assert!(styled.to_string().contains("faded"));
}

#[test]
fn test_colors_bold_contains_text() {
    let styled = Colors::bold("strong");
    assert!(styled.to_string().contains("strong"));
}

#[test]
fn test_colors_underlined_contains_text() {
    let styled = Colors::underlined("link");
    assert!(styled.to_string().contains("link"));
}

#[test]
fn test_colors_for_status_valid_contains_text() {
    let styled = Colors::for_status("valid", ItemStatus::Valid);
    assert!(styled.to_string().contains("valid"));
}

#[test]
fn test_colors_for_status_applied_contains_text() {
    let styled = Colors::for_status("applied", ItemStatus::Applied);
    assert!(styled.to_string().contains("applied"));
}

#[test]
fn test_colors_for_status_missing_contains_text() {
    let styled = Colors::for_status("missing", ItemStatus::Missing);
    assert!(styled.to_string().contains("missing"));
}

#[test]
fn test_colors_for_status_pending_contains_text() {
    let styled = Colors::for_status("pending", ItemStatus::Pending);
    assert!(styled.to_string().contains("pending"));
}

#[test]
fn test_colors_for_status_failed_contains_text() {
    let styled = Colors::for_status("failed", ItemStatus::Failed);
    assert!(styled.to_string().contains("failed"));
}

#[test]
fn test_colors_for_status_disabled_contains_text() {
    let styled = Colors::for_status("disabled", ItemStatus::Disabled);
    assert!(styled.to_string().contains("disabled"));
}

#[test]
fn test_colors_for_message_level_success_contains_text() {
    let styled = Colors::for_message_level("success", MessageLevel::Success);
    assert!(styled.to_string().contains("success"));
}

#[test]
fn test_colors_for_message_level_warning_contains_text() {
    let styled = Colors::for_message_level("warning", MessageLevel::Warning);
    assert!(styled.to_string().contains("warning"));
}

#[test]
fn test_colors_for_message_level_error_contains_text() {
    let styled = Colors::for_message_level("error", MessageLevel::Error);
    assert!(styled.to_string().contains("error"));
}

#[test]
fn test_colors_for_message_level_info_contains_text() {
    let styled = Colors::for_message_level("info", MessageLevel::Info);
    assert!(styled.to_string().contains("info"));
}

// ============================================================================
// BrandColors Tests
// ============================================================================

#[test]
fn test_brand_colors_primary_contains_text() {
    let styled = BrandColors::primary("brand");
    assert!(styled.to_string().contains("brand"));
}

#[test]
fn test_brand_colors_primary_bold_contains_text() {
    let styled = BrandColors::primary_bold("bold_brand");
    assert!(styled.to_string().contains("bold_brand"));
}

#[test]
fn test_brand_colors_white_bold_contains_text() {
    let styled = BrandColors::white_bold("white_b");
    assert!(styled.to_string().contains("white_b"));
}

#[test]
fn test_brand_colors_white_contains_text() {
    let styled = BrandColors::white("plain_white");
    assert!(styled.to_string().contains("plain_white"));
}

#[test]
fn test_brand_colors_dim_contains_text() {
    let styled = BrandColors::dim("dimmed");
    assert!(styled.to_string().contains("dimmed"));
}

#[test]
fn test_brand_colors_highlight_contains_text() {
    let styled = BrandColors::highlight("highlighted");
    assert!(styled.to_string().contains("highlighted"));
}

#[test]
fn test_brand_colors_running_contains_text() {
    let styled = BrandColors::running("active");
    assert!(styled.to_string().contains("active"));
}

#[test]
fn test_brand_colors_stopped_contains_text() {
    let styled = BrandColors::stopped("halted");
    assert!(styled.to_string().contains("halted"));
}

#[test]
fn test_brand_colors_starting_contains_text() {
    let styled = BrandColors::starting("booting");
    assert!(styled.to_string().contains("booting"));
}

// ============================================================================
// Colors with different Display types
// ============================================================================

#[test]
fn test_colors_success_with_number() {
    let styled = Colors::success(42);
    assert!(styled.to_string().contains("42"));
}

#[test]
fn test_colors_error_with_string_owned() {
    let styled = Colors::error(String::from("owned_error"));
    assert!(styled.to_string().contains("owned_error"));
}

#[test]
fn test_brand_colors_primary_with_number() {
    let styled = BrandColors::primary(100);
    assert!(styled.to_string().contains("100"));
}
