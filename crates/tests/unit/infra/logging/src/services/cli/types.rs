//! Unit tests for CLI type enums

use systemprompt_logging::services::cli::theme::{
    ActionType, ColorType, EmphasisType, IconType, ItemStatus, MessageLevel, ModuleType,
};

// ============================================================================
// ItemStatus Tests
// ============================================================================

#[test]
fn test_item_status_clone() {
    let status = ItemStatus::Applied;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_item_status_copy() {
    let status = ItemStatus::Applied;
    let copied: ItemStatus = status;
    assert_eq!(status, copied);
}

#[test]
fn test_item_status_equality() {
    assert_eq!(ItemStatus::Valid, ItemStatus::Valid);
    assert_ne!(ItemStatus::Valid, ItemStatus::Failed);
}

// ============================================================================
// ModuleType Tests
// ============================================================================

#[test]
fn test_module_type_clone() {
    let module = ModuleType::Schema;
    let cloned = module.clone();
    assert_eq!(module, cloned);
}

#[test]
fn test_module_type_copy() {
    let module = ModuleType::Seed;
    let copied: ModuleType = module;
    assert_eq!(module, copied);
}

#[test]
fn test_module_type_equality() {
    assert_eq!(ModuleType::Schema, ModuleType::Schema);
    assert_ne!(ModuleType::Schema, ModuleType::Seed);
}

// ============================================================================
// MessageLevel Tests
// ============================================================================

#[test]
fn test_message_level_clone() {
    let level = MessageLevel::Success;
    let cloned = level.clone();
    assert_eq!(level, cloned);
}

#[test]
fn test_message_level_copy() {
    let level = MessageLevel::Warning;
    let copied: MessageLevel = level;
    assert_eq!(level, copied);
}

#[test]
fn test_message_level_equality() {
    assert_eq!(MessageLevel::Error, MessageLevel::Error);
    assert_ne!(MessageLevel::Error, MessageLevel::Info);
}

// ============================================================================
// ActionType Tests
// ============================================================================

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
