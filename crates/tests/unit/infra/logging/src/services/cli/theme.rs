//! Unit tests for CLI theme components (ServiceStatus, Icons, Colors, Theme, BrandColors)

use systemprompt_logging::services::cli::theme::{
    ActionType, BrandColors, ColorType, Colors, EmphasisType, IconType, Icons, ItemStatus,
    MessageLevel, ModuleType, ServiceStatus, Theme,
};

// ============================================================================
// ServiceStatus Tests
// ============================================================================

#[test]
fn test_service_status_running_symbol() {
    let status = ServiceStatus::Running;
    assert_eq!(status.symbol(), "●");
}

#[test]
fn test_service_status_stopped_symbol() {
    let status = ServiceStatus::Stopped;
    assert_eq!(status.symbol(), "○");
}

#[test]
fn test_service_status_starting_symbol() {
    let status = ServiceStatus::Starting;
    assert_eq!(status.symbol(), "◐");
}

#[test]
fn test_service_status_failed_symbol() {
    let status = ServiceStatus::Failed;
    assert_eq!(status.symbol(), "✗");
}

#[test]
fn test_service_status_unknown_symbol() {
    let status = ServiceStatus::Unknown;
    assert_eq!(status.symbol(), "?");
}

#[test]
fn test_service_status_running_text() {
    let status = ServiceStatus::Running;
    assert_eq!(status.text(), "Running");
}

#[test]
fn test_service_status_stopped_text() {
    let status = ServiceStatus::Stopped;
    assert_eq!(status.text(), "Stopped");
}

#[test]
fn test_service_status_starting_text() {
    let status = ServiceStatus::Starting;
    assert_eq!(status.text(), "Starting");
}

#[test]
fn test_service_status_failed_text() {
    let status = ServiceStatus::Failed;
    assert_eq!(status.text(), "Failed");
}

#[test]
fn test_service_status_unknown_text() {
    let status = ServiceStatus::Unknown;
    assert_eq!(status.text(), "Unknown");
}

#[test]
fn test_service_status_debug() {
    let status = ServiceStatus::Running;
    assert_eq!(format!("{:?}", status), "Running");
}

#[test]
fn test_service_status_clone() {
    let status = ServiceStatus::Failed;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_service_status_copy() {
    let status = ServiceStatus::Starting;
    let copied: ServiceStatus = status;
    assert_eq!(status, copied);
}

#[test]
fn test_service_status_equality() {
    assert_eq!(ServiceStatus::Running, ServiceStatus::Running);
    assert_ne!(ServiceStatus::Running, ServiceStatus::Stopped);
}

#[test]
fn test_service_status_all_variants_have_symbol() {
    let variants = [
        ServiceStatus::Running,
        ServiceStatus::Stopped,
        ServiceStatus::Starting,
        ServiceStatus::Failed,
        ServiceStatus::Unknown,
    ];

    for status in variants {
        assert!(!status.symbol().is_empty());
    }
}

#[test]
fn test_service_status_all_variants_have_text() {
    let variants = [
        ServiceStatus::Running,
        ServiceStatus::Stopped,
        ServiceStatus::Starting,
        ServiceStatus::Failed,
        ServiceStatus::Unknown,
    ];

    for status in variants {
        assert!(!status.text().is_empty());
    }
}

// ============================================================================
// Icons Tests
// ============================================================================

#[test]
fn test_icons_checkmark() {
    let emoji = Icons::CHECKMARK;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_warning() {
    let emoji = Icons::WARNING;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_error() {
    let emoji = Icons::ERROR;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_info() {
    let emoji = Icons::INFO;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_package() {
    let emoji = Icons::PACKAGE;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_schema() {
    let emoji = Icons::SCHEMA;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_seed() {
    let emoji = Icons::SEED;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_config() {
    let emoji = Icons::CONFIG;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_arrow() {
    let emoji = Icons::ARROW;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_update() {
    let emoji = Icons::UPDATE;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_install() {
    let emoji = Icons::INSTALL;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_pause() {
    let emoji = Icons::PAUSE;
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_module_type_schema() {
    let emoji = Icons::for_module_type(ModuleType::Schema);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_module_type_seed() {
    let emoji = Icons::for_module_type(ModuleType::Seed);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_module_type_module() {
    let emoji = Icons::for_module_type(ModuleType::Module);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_module_type_configuration() {
    let emoji = Icons::for_module_type(ModuleType::Configuration);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_valid() {
    let emoji = Icons::for_status(ItemStatus::Valid);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_applied() {
    let emoji = Icons::for_status(ItemStatus::Applied);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_missing() {
    let emoji = Icons::for_status(ItemStatus::Missing);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_pending() {
    let emoji = Icons::for_status(ItemStatus::Pending);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_failed() {
    let emoji = Icons::for_status(ItemStatus::Failed);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_status_disabled() {
    let emoji = Icons::for_status(ItemStatus::Disabled);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_message_level_success() {
    let emoji = Icons::for_message_level(MessageLevel::Success);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_message_level_warning() {
    let emoji = Icons::for_message_level(MessageLevel::Warning);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_message_level_error() {
    let emoji = Icons::for_message_level(MessageLevel::Error);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_for_message_level_info() {
    let emoji = Icons::for_message_level(MessageLevel::Info);
    assert!(!emoji.to_string().is_empty());
}

#[test]
fn test_icons_debug() {
    let icons = Icons;
    assert_eq!(format!("{:?}", icons), "Icons");
}

#[test]
fn test_icons_clone() {
    let icons = Icons;
    let cloned = icons.clone();
    assert_eq!(format!("{:?}", icons), format!("{:?}", cloned));
}

#[test]
fn test_icons_copy() {
    let icons = Icons;
    let copied: Icons = icons;
    assert_eq!(format!("{:?}", icons), format!("{:?}", copied));
}

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

#[test]
fn test_colors_debug() {
    let colors = Colors;
    assert_eq!(format!("{:?}", colors), "Colors");
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

#[test]
fn test_brand_colors_debug() {
    let brand = BrandColors;
    assert_eq!(format!("{:?}", brand), "BrandColors");
}

#[test]
fn test_brand_colors_clone() {
    let brand = BrandColors;
    let cloned = brand.clone();
    assert_eq!(format!("{:?}", brand), format!("{:?}", cloned));
}

#[test]
fn test_brand_colors_copy() {
    let brand = BrandColors;
    let copied: BrandColors = brand;
    assert_eq!(format!("{:?}", brand), format!("{:?}", copied));
}

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

#[test]
fn test_theme_debug() {
    let theme = Theme;
    assert_eq!(format!("{:?}", theme), "Theme");
}

#[test]
fn test_theme_clone() {
    let theme = Theme;
    let cloned = theme.clone();
    assert_eq!(format!("{:?}", theme), format!("{:?}", cloned));
}

#[test]
fn test_theme_copy() {
    let theme = Theme;
    let copied: Theme = theme;
    assert_eq!(format!("{:?}", theme), format!("{:?}", copied));
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
