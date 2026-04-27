//! Unit tests for Theme and integration tests across all theme types

use systemprompt_logging::services::cli::theme::{
    ActionType, EmphasisType, Icons, ItemStatus, MessageLevel, ModuleType, Theme,
};

// ============================================================================
// Theme::icon Tests
// ============================================================================

#[test]
fn test_theme_icon_for_item_status_valid_returns_checkmark() {
    let icon = Theme::icon(ItemStatus::Valid);
    assert_eq!(icon.to_string(), Icons::CHECKMARK.to_string());
}

#[test]
fn test_theme_icon_for_item_status_failed_returns_error() {
    let icon = Theme::icon(ItemStatus::Failed);
    assert_eq!(icon.to_string(), Icons::ERROR.to_string());
}

#[test]
fn test_theme_icon_for_module_type_schema_returns_schema() {
    let icon = Theme::icon(ModuleType::Schema);
    assert_eq!(icon.to_string(), Icons::SCHEMA.to_string());
}

#[test]
fn test_theme_icon_for_message_level_success_returns_checkmark() {
    let icon = Theme::icon(MessageLevel::Success);
    assert_eq!(icon.to_string(), Icons::CHECKMARK.to_string());
}

#[test]
fn test_theme_icon_for_action_type_install_returns_install() {
    let icon = Theme::icon(ActionType::Install);
    assert_eq!(icon.to_string(), Icons::INSTALL.to_string());
}

#[test]
fn test_theme_icon_for_action_type_update_returns_update() {
    let icon = Theme::icon(ActionType::Update);
    assert_eq!(icon.to_string(), Icons::UPDATE.to_string());
}

#[test]
fn test_theme_icon_for_action_type_arrow_returns_arrow() {
    let icon = Theme::icon(ActionType::Arrow);
    assert_eq!(icon.to_string(), Icons::ARROW.to_string());
}

// ============================================================================
// Theme::color Tests
// ============================================================================

#[test]
fn test_theme_color_for_item_status_contains_text() {
    let styled = Theme::color("status_text", ItemStatus::Valid);
    assert!(styled.to_string().contains("status_text"));
}

#[test]
fn test_theme_color_for_message_level_contains_text() {
    let styled = Theme::color("msg_text", MessageLevel::Error);
    assert!(styled.to_string().contains("msg_text"));
}

#[test]
fn test_theme_color_for_emphasis_highlight_contains_text() {
    let styled = Theme::color("highlight_text", EmphasisType::Highlight);
    assert!(styled.to_string().contains("highlight_text"));
}

#[test]
fn test_theme_color_for_emphasis_dim_contains_text() {
    let styled = Theme::color("dim_text", EmphasisType::Dim);
    assert!(styled.to_string().contains("dim_text"));
}

#[test]
fn test_theme_color_for_emphasis_bold_contains_text() {
    let styled = Theme::color("bold_text", EmphasisType::Bold);
    assert!(styled.to_string().contains("bold_text"));
}

#[test]
fn test_theme_color_for_emphasis_underlined_contains_text() {
    let styled = Theme::color("ul_text", EmphasisType::Underlined);
    assert!(styled.to_string().contains("ul_text"));
}

// ============================================================================
// Theme Integration Tests
// ============================================================================

#[test]
fn test_theme_all_item_statuses_produce_icons_matching_icons_for_status() {
    let statuses = [
        ItemStatus::Missing,
        ItemStatus::Applied,
        ItemStatus::Failed,
        ItemStatus::Valid,
        ItemStatus::Disabled,
        ItemStatus::Pending,
    ];

    for status in statuses {
        let theme_icon = Theme::icon(status).to_string();
        let icons_icon = Icons::for_status(status).to_string();
        assert_eq!(
            theme_icon, icons_icon,
            "Theme::icon and Icons::for_status should match for {:?}",
            status
        );
    }
}

#[test]
fn test_theme_all_module_types_produce_icons_matching_icons_for_module_type() {
    let modules = [
        ModuleType::Schema,
        ModuleType::Seed,
        ModuleType::Module,
        ModuleType::Configuration,
    ];

    for module in modules {
        let theme_icon = Theme::icon(module).to_string();
        let icons_icon = Icons::for_module_type(module).to_string();
        assert_eq!(
            theme_icon, icons_icon,
            "Theme::icon and Icons::for_module_type should match for {:?}",
            module
        );
    }
}

#[test]
fn test_theme_all_message_levels_produce_icons_matching_icons_for_message_level() {
    let levels = [
        MessageLevel::Success,
        MessageLevel::Warning,
        MessageLevel::Error,
        MessageLevel::Info,
    ];

    for level in levels {
        let theme_icon = Theme::icon(level).to_string();
        let icons_icon = Icons::for_message_level(level).to_string();
        assert_eq!(
            theme_icon, icons_icon,
            "Theme::icon and Icons::for_message_level should match for {:?}",
            level
        );
    }
}

#[test]
fn test_theme_color_preserves_text_for_all_item_statuses() {
    let statuses = [
        ItemStatus::Missing,
        ItemStatus::Applied,
        ItemStatus::Failed,
        ItemStatus::Valid,
        ItemStatus::Disabled,
        ItemStatus::Pending,
    ];

    for status in statuses {
        let styled = Theme::color("preserved", status);
        assert!(
            styled.to_string().contains("preserved"),
            "Text should be preserved for status {:?}",
            status
        );
    }
}

#[test]
fn test_theme_color_preserves_text_for_all_message_levels() {
    let levels = [
        MessageLevel::Success,
        MessageLevel::Warning,
        MessageLevel::Error,
        MessageLevel::Info,
    ];

    for level in levels {
        let styled = Theme::color("preserved", level);
        assert!(
            styled.to_string().contains("preserved"),
            "Text should be preserved for level {:?}",
            level
        );
    }
}

#[test]
fn test_theme_color_preserves_text_for_all_emphasis_types() {
    let emphases = [
        EmphasisType::Highlight,
        EmphasisType::Dim,
        EmphasisType::Bold,
        EmphasisType::Underlined,
    ];

    for emphasis in emphases {
        let styled = Theme::color("preserved", emphasis);
        assert!(
            styled.to_string().contains("preserved"),
            "Text should be preserved for emphasis {:?}",
            emphasis
        );
    }
}
