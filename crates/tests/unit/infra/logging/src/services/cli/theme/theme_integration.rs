//! Unit tests for Theme and integration tests across all theme types

use systemprompt_logging::services::cli::theme::{
    ActionType, EmphasisType, ItemStatus, MessageLevel, ModuleType, Theme,
};

// ============================================================================
// Theme Tests
// ============================================================================

#[test]
fn test_theme_icon_for_item_status() {
    let icon = Theme::icon(ItemStatus::Valid);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_icon_for_module_type() {
    let icon = Theme::icon(ModuleType::Schema);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_icon_for_message_level() {
    let icon = Theme::icon(MessageLevel::Success);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_icon_for_action_type_install() {
    let icon = Theme::icon(ActionType::Install);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_icon_for_action_type_update() {
    let icon = Theme::icon(ActionType::Update);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_icon_for_action_type_arrow() {
    let icon = Theme::icon(ActionType::Arrow);
    assert!(!icon.to_string().is_empty());
}

#[test]
fn test_theme_color_for_item_status() {
    let styled = Theme::color("test", ItemStatus::Valid);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_theme_color_for_message_level() {
    let styled = Theme::color("test", MessageLevel::Error);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_theme_color_for_emphasis_highlight() {
    let styled = Theme::color("test", EmphasisType::Highlight);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_theme_color_for_emphasis_dim() {
    let styled = Theme::color("test", EmphasisType::Dim);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_theme_color_for_emphasis_bold() {
    let styled = Theme::color("test", EmphasisType::Bold);
    assert!(!styled.to_string().is_empty());
}

#[test]
fn test_theme_color_for_emphasis_underlined() {
    let styled = Theme::color("test", EmphasisType::Underlined);
    assert!(!styled.to_string().is_empty());
}

// ============================================================================
// Theme Integration Tests
// ============================================================================

#[test]
fn test_theme_all_item_statuses() {
    let statuses = [
        ItemStatus::Missing,
        ItemStatus::Applied,
        ItemStatus::Failed,
        ItemStatus::Valid,
        ItemStatus::Disabled,
        ItemStatus::Pending,
    ];

    for status in statuses {
        let icon = Theme::icon(status);
        assert!(!icon.to_string().is_empty());

        let color = Theme::color("test", status);
        assert!(!color.to_string().is_empty());
    }
}

#[test]
fn test_theme_all_module_types() {
    let modules = [
        ModuleType::Schema,
        ModuleType::Seed,
        ModuleType::Module,
        ModuleType::Configuration,
    ];

    for module in modules {
        let icon = Theme::icon(module);
        assert!(!icon.to_string().is_empty());
    }
}

#[test]
fn test_theme_all_message_levels() {
    let levels = [
        MessageLevel::Success,
        MessageLevel::Warning,
        MessageLevel::Error,
        MessageLevel::Info,
    ];

    for level in levels {
        let icon = Theme::icon(level);
        assert!(!icon.to_string().is_empty());

        let color = Theme::color("test", level);
        assert!(!color.to_string().is_empty());
    }
}

#[test]
fn test_theme_all_action_types() {
    let actions = [ActionType::Install, ActionType::Update, ActionType::Arrow];

    for action in actions {
        let icon = Theme::icon(action);
        assert!(!icon.to_string().is_empty());
    }
}

#[test]
fn test_theme_all_emphasis_types() {
    let emphases = [
        EmphasisType::Highlight,
        EmphasisType::Dim,
        EmphasisType::Bold,
        EmphasisType::Underlined,
    ];

    for emphasis in emphases {
        let color = Theme::color("test", emphasis);
        assert!(!color.to_string().is_empty());
    }
}
