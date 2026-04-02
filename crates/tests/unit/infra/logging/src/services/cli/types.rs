//! Unit tests for CLI type enums

use systemprompt_logging::services::cli::theme::{
    ActionType, ColorType, EmphasisType, IconType, ItemStatus, MessageLevel, ModuleType,
};

// ============================================================================
// ItemStatus Tests
// ============================================================================

#[test]
fn test_item_status_debug() {
    assert_eq!(format!("{:?}", ItemStatus::Missing), "Missing");
    assert_eq!(format!("{:?}", ItemStatus::Applied), "Applied");
    assert_eq!(format!("{:?}", ItemStatus::Failed), "Failed");
    assert_eq!(format!("{:?}", ItemStatus::Valid), "Valid");
    assert_eq!(format!("{:?}", ItemStatus::Disabled), "Disabled");
    assert_eq!(format!("{:?}", ItemStatus::Pending), "Pending");
}

#[test]
fn test_item_status_clone_preserves_value() {
    let original = ItemStatus::Valid;
    let cloned = original;
    assert_eq!(original, cloned);
}

#[test]
fn test_item_status_equality() {
    assert_eq!(ItemStatus::Valid, ItemStatus::Valid);
    assert_ne!(ItemStatus::Valid, ItemStatus::Failed);
    assert_ne!(ItemStatus::Missing, ItemStatus::Pending);
}

// ============================================================================
// ModuleType Tests
// ============================================================================

#[test]
fn test_module_type_debug() {
    assert_eq!(format!("{:?}", ModuleType::Schema), "Schema");
    assert_eq!(format!("{:?}", ModuleType::Seed), "Seed");
    assert_eq!(format!("{:?}", ModuleType::Module), "Module");
    assert_eq!(format!("{:?}", ModuleType::Configuration), "Configuration");
}

#[test]
fn test_module_type_equality() {
    assert_eq!(ModuleType::Schema, ModuleType::Schema);
    assert_ne!(ModuleType::Schema, ModuleType::Seed);
    assert_ne!(ModuleType::Module, ModuleType::Configuration);
}

// ============================================================================
// MessageLevel Tests
// ============================================================================

#[test]
fn test_message_level_debug() {
    assert_eq!(format!("{:?}", MessageLevel::Success), "Success");
    assert_eq!(format!("{:?}", MessageLevel::Warning), "Warning");
    assert_eq!(format!("{:?}", MessageLevel::Error), "Error");
    assert_eq!(format!("{:?}", MessageLevel::Info), "Info");
}

#[test]
fn test_message_level_equality() {
    assert_eq!(MessageLevel::Success, MessageLevel::Success);
    assert_ne!(MessageLevel::Success, MessageLevel::Error);
    assert_ne!(MessageLevel::Warning, MessageLevel::Info);
}

// ============================================================================
// ActionType Tests
// ============================================================================

#[test]
fn test_action_type_debug() {
    assert_eq!(format!("{:?}", ActionType::Install), "Install");
    assert_eq!(format!("{:?}", ActionType::Update), "Update");
    assert_eq!(format!("{:?}", ActionType::Arrow), "Arrow");
}

// ============================================================================
// EmphasisType Tests
// ============================================================================

#[test]
fn test_emphasis_type_debug() {
    assert_eq!(format!("{:?}", EmphasisType::Highlight), "Highlight");
    assert_eq!(format!("{:?}", EmphasisType::Dim), "Dim");
    assert_eq!(format!("{:?}", EmphasisType::Bold), "Bold");
    assert_eq!(format!("{:?}", EmphasisType::Underlined), "Underlined");
}

// ============================================================================
// IconType Conversion Tests
// ============================================================================

#[test]
fn test_icon_type_from_item_status() {
    let icon: IconType = ItemStatus::Valid.into();
    assert!(matches!(icon, IconType::Status(ItemStatus::Valid)));
}

#[test]
fn test_icon_type_from_item_status_failed() {
    let icon: IconType = ItemStatus::Failed.into();
    assert!(matches!(icon, IconType::Status(ItemStatus::Failed)));
}

#[test]
fn test_icon_type_from_module_type() {
    let icon: IconType = ModuleType::Schema.into();
    assert!(matches!(icon, IconType::Module(ModuleType::Schema)));
}

#[test]
fn test_icon_type_from_module_type_seed() {
    let icon: IconType = ModuleType::Seed.into();
    assert!(matches!(icon, IconType::Module(ModuleType::Seed)));
}

#[test]
fn test_icon_type_from_message_level() {
    let icon: IconType = MessageLevel::Success.into();
    assert!(matches!(icon, IconType::Message(MessageLevel::Success)));
}

#[test]
fn test_icon_type_from_message_level_error() {
    let icon: IconType = MessageLevel::Error.into();
    assert!(matches!(icon, IconType::Message(MessageLevel::Error)));
}

#[test]
fn test_icon_type_from_action_type() {
    let icon: IconType = ActionType::Install.into();
    assert!(matches!(icon, IconType::Action(ActionType::Install)));
}

#[test]
fn test_icon_type_from_action_type_update() {
    let icon: IconType = ActionType::Update.into();
    assert!(matches!(icon, IconType::Action(ActionType::Update)));
}

// ============================================================================
// ColorType Conversion Tests
// ============================================================================

#[test]
fn test_color_type_from_item_status() {
    let color: ColorType = ItemStatus::Valid.into();
    assert!(matches!(color, ColorType::Status(ItemStatus::Valid)));
}

#[test]
fn test_color_type_from_item_status_pending() {
    let color: ColorType = ItemStatus::Pending.into();
    assert!(matches!(color, ColorType::Status(ItemStatus::Pending)));
}

#[test]
fn test_color_type_from_message_level() {
    let color: ColorType = MessageLevel::Warning.into();
    assert!(matches!(color, ColorType::Message(MessageLevel::Warning)));
}

#[test]
fn test_color_type_from_message_level_info() {
    let color: ColorType = MessageLevel::Info.into();
    assert!(matches!(color, ColorType::Message(MessageLevel::Info)));
}

#[test]
fn test_color_type_from_emphasis_type() {
    let color: ColorType = EmphasisType::Highlight.into();
    assert!(matches!(color, ColorType::Emphasis(EmphasisType::Highlight)));
}

#[test]
fn test_color_type_from_emphasis_type_bold() {
    let color: ColorType = EmphasisType::Bold.into();
    assert!(matches!(color, ColorType::Emphasis(EmphasisType::Bold)));
}

// ============================================================================
// IconType All Variants Test
// ============================================================================

#[test]
fn test_icon_type_all_action_variants() {
    let variants = [ActionType::Install, ActionType::Update, ActionType::Arrow];

    for action in variants {
        let icon: IconType = action.into();
        assert!(matches!(icon, IconType::Action(_)));
    }
}

#[test]
fn test_color_type_all_emphasis_variants() {
    let variants = [
        EmphasisType::Highlight,
        EmphasisType::Dim,
        EmphasisType::Bold,
        EmphasisType::Underlined,
    ];

    for emphasis in variants {
        let color: ColorType = emphasis.into();
        assert!(matches!(color, ColorType::Emphasis(_)));
    }
}

#[test]
fn test_item_status_all_variants() {
    let variants = [
        ItemStatus::Missing,
        ItemStatus::Applied,
        ItemStatus::Failed,
        ItemStatus::Valid,
        ItemStatus::Disabled,
        ItemStatus::Pending,
    ];

    for status in variants {
        let icon: IconType = status.into();
        assert!(matches!(icon, IconType::Status(_)));

        let color: ColorType = status.into();
        assert!(matches!(color, ColorType::Status(_)));
    }
}

// ============================================================================
// IconType Debug Tests
// ============================================================================

#[test]
fn test_icon_type_debug_status() {
    let icon: IconType = ItemStatus::Valid.into();
    let debug = format!("{:?}", icon);
    assert!(debug.contains("Status"));
    assert!(debug.contains("Valid"));
}

#[test]
fn test_icon_type_debug_module() {
    let icon: IconType = ModuleType::Schema.into();
    let debug = format!("{:?}", icon);
    assert!(debug.contains("Module"));
    assert!(debug.contains("Schema"));
}

#[test]
fn test_icon_type_debug_message() {
    let icon: IconType = MessageLevel::Error.into();
    let debug = format!("{:?}", icon);
    assert!(debug.contains("Message"));
    assert!(debug.contains("Error"));
}

#[test]
fn test_icon_type_debug_action() {
    let icon: IconType = ActionType::Arrow.into();
    let debug = format!("{:?}", icon);
    assert!(debug.contains("Action"));
    assert!(debug.contains("Arrow"));
}

#[test]
fn test_color_type_debug_status() {
    let color: ColorType = ItemStatus::Failed.into();
    let debug = format!("{:?}", color);
    assert!(debug.contains("Status"));
    assert!(debug.contains("Failed"));
}

#[test]
fn test_color_type_debug_emphasis() {
    let color: ColorType = EmphasisType::Dim.into();
    let debug = format!("{:?}", color);
    assert!(debug.contains("Emphasis"));
    assert!(debug.contains("Dim"));
}
