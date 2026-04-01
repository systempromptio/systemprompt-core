//! Unit tests for ServiceStatus and Icons

use systemprompt_logging::services::cli::theme::{
    Icons, ItemStatus, MessageLevel, ModuleType, ServiceStatus,
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
