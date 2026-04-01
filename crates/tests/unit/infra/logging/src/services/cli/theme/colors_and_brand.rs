//! Unit tests for Colors and BrandColors

use systemprompt_logging::services::cli::theme::{
    BrandColors, Colors, ItemStatus, MessageLevel,
};

// ============================================================================
// Colors Tests
// ============================================================================

#[test]
fn test_colors_success() {
    let styled = Colors::success("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_warning() {
    let styled = Colors::warning("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_error() {
    let styled = Colors::error("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_info() {
    let styled = Colors::info("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_highlight() {
    let styled = Colors::highlight("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_dim() {
    let styled = Colors::dim("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_bold() {
    let styled = Colors::bold("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_underlined() {
    let styled = Colors::underlined("test");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_valid() {
    let styled = Colors::for_status("valid", ItemStatus::Valid);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_applied() {
    let styled = Colors::for_status("applied", ItemStatus::Applied);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_missing() {
    let styled = Colors::for_status("missing", ItemStatus::Missing);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_pending() {
    let styled = Colors::for_status("pending", ItemStatus::Pending);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_failed() {
    let styled = Colors::for_status("failed", ItemStatus::Failed);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_status_disabled() {
    let styled = Colors::for_status("disabled", ItemStatus::Disabled);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_message_level_success() {
    let styled = Colors::for_message_level("success", MessageLevel::Success);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_message_level_warning() {
    let styled = Colors::for_message_level("warning", MessageLevel::Warning);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_message_level_error() {
    let styled = Colors::for_message_level("error", MessageLevel::Error);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_colors_for_message_level_info() {
    let styled = Colors::for_message_level("info", MessageLevel::Info);
    assert!(!styled.to_string().is_empty());
}

// ============================================================================
// BrandColors Tests
// ============================================================================

#[test]
fn test_brand_colors_primary() {
    let styled = BrandColors::primary("brand");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_primary_bold() {
    let styled = BrandColors::primary_bold("brand");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_white_bold() {
    let styled = BrandColors::white_bold("white");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_white() {
    let styled = BrandColors::white("white");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_dim() {
    let styled = BrandColors::dim("dim");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_highlight() {
    let styled = BrandColors::highlight("highlight");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_running() {
    let styled = BrandColors::running("running");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_stopped() {
    let styled = BrandColors::stopped("stopped");
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_brand_colors_starting() {
    let styled = BrandColors::starting("starting");
    assert!(!styled.to_string().is_empty());
}
