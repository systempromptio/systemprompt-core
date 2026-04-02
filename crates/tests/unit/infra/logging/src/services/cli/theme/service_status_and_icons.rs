//! Unit tests for ServiceStatus and Icons

use systemprompt_logging::services::cli::theme::{
    Icons, ItemStatus, MessageLevel, ModuleType, ServiceStatus,
};

// ============================================================================
// ServiceStatus Tests
// ============================================================================

#[test]
fn test_service_status_running_symbol() {
    assert_eq!(ServiceStatus::Running.symbol(), "\u{25cf}");
}

#[test]
fn test_service_status_stopped_symbol() {
    assert_eq!(ServiceStatus::Stopped.symbol(), "\u{25cb}");
}

#[test]
fn test_service_status_starting_symbol() {
    assert_eq!(ServiceStatus::Starting.symbol(), "\u{25d0}");
}

#[test]
fn test_service_status_failed_symbol() {
    assert_eq!(ServiceStatus::Failed.symbol(), "\u{2717}");
}

#[test]
fn test_service_status_unknown_symbol() {
    assert_eq!(ServiceStatus::Unknown.symbol(), "?");
}

#[test]
fn test_service_status_running_text() {
    assert_eq!(ServiceStatus::Running.text(), "Running");
}

#[test]
fn test_service_status_stopped_text() {
    assert_eq!(ServiceStatus::Stopped.text(), "Stopped");
}

#[test]
fn test_service_status_starting_text() {
    assert_eq!(ServiceStatus::Starting.text(), "Starting");
}

#[test]
fn test_service_status_failed_text() {
    assert_eq!(ServiceStatus::Failed.text(), "Failed");
}

#[test]
fn test_service_status_unknown_text() {
    assert_eq!(ServiceStatus::Unknown.text(), "Unknown");
}

#[test]
fn test_service_status_all_variants_have_unique_symbols() {
    let variants = [
        ServiceStatus::Running,
        ServiceStatus::Stopped,
        ServiceStatus::Starting,
        ServiceStatus::Failed,
        ServiceStatus::Unknown,
    ];

    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(
                variants[i].symbol(),
                variants[j].symbol(),
                "{:?} and {:?} should have different symbols",
                variants[i],
                variants[j]
            );
        }
    }
}

#[test]
fn test_service_status_all_variants_have_unique_text() {
    let variants = [
        ServiceStatus::Running,
        ServiceStatus::Stopped,
        ServiceStatus::Starting,
        ServiceStatus::Failed,
        ServiceStatus::Unknown,
    ];

    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(
                variants[i].text(),
                variants[j].text(),
                "{:?} and {:?} should have different text",
                variants[i],
                variants[j]
            );
        }
    }
}

#[test]
fn test_service_status_equality() {
    assert_eq!(ServiceStatus::Running, ServiceStatus::Running);
    assert_ne!(ServiceStatus::Running, ServiceStatus::Stopped);
}

#[test]
fn test_service_status_debug() {
    assert_eq!(format!("{:?}", ServiceStatus::Running), "Running");
    assert_eq!(format!("{:?}", ServiceStatus::Failed), "Failed");
}

// ============================================================================
// Icons Constants Tests
// ============================================================================

#[test]
fn test_icons_checkmark_value() {
    let s = Icons::CHECKMARK.to_string();
    assert!(s.contains('\u{2713}') || s.contains('\u{2713}'));
}

#[test]
fn test_icons_warning_value() {
    let s = Icons::WARNING.to_string();
    assert!(s.contains('\u{26a0}') || s.contains('!'));
}

#[test]
fn test_icons_error_value() {
    let s = Icons::ERROR.to_string();
    assert!(s.contains('\u{2717}') || s.contains('X'));
}

#[test]
fn test_icons_info_value() {
    let s = Icons::INFO.to_string();
    assert!(s.contains('\u{2139}') || s.contains('i'));
}

#[test]
fn test_icons_arrow_value() {
    let s = Icons::ARROW.to_string();
    assert!(s.contains('\u{2192}') || s.contains("->"));
}

// ============================================================================
// Icons for_module_type Tests
// ============================================================================

#[test]
fn test_icons_for_module_type_schema_matches_constant() {
    let from_method = Icons::for_module_type(ModuleType::Schema).to_string();
    let from_constant = Icons::SCHEMA.to_string();
    assert_eq!(from_method, from_constant);
}

#[test]
fn test_icons_for_module_type_seed_matches_constant() {
    let from_method = Icons::for_module_type(ModuleType::Seed).to_string();
    let from_constant = Icons::SEED.to_string();
    assert_eq!(from_method, from_constant);
}

#[test]
fn test_icons_for_module_type_module_matches_constant() {
    let from_method = Icons::for_module_type(ModuleType::Module).to_string();
    let from_constant = Icons::PACKAGE.to_string();
    assert_eq!(from_method, from_constant);
}

#[test]
fn test_icons_for_module_type_configuration_matches_constant() {
    let from_method = Icons::for_module_type(ModuleType::Configuration).to_string();
    let from_constant = Icons::CONFIG.to_string();
    assert_eq!(from_method, from_constant);
}

// ============================================================================
// Icons for_status Tests
// ============================================================================

#[test]
fn test_icons_for_status_valid_is_checkmark() {
    let icon = Icons::for_status(ItemStatus::Valid).to_string();
    assert_eq!(icon, Icons::CHECKMARK.to_string());
}

#[test]
fn test_icons_for_status_applied_is_checkmark() {
    let icon = Icons::for_status(ItemStatus::Applied).to_string();
    assert_eq!(icon, Icons::CHECKMARK.to_string());
}

#[test]
fn test_icons_for_status_missing_is_warning() {
    let icon = Icons::for_status(ItemStatus::Missing).to_string();
    assert_eq!(icon, Icons::WARNING.to_string());
}

#[test]
fn test_icons_for_status_pending_is_warning() {
    let icon = Icons::for_status(ItemStatus::Pending).to_string();
    assert_eq!(icon, Icons::WARNING.to_string());
}

#[test]
fn test_icons_for_status_failed_is_error() {
    let icon = Icons::for_status(ItemStatus::Failed).to_string();
    assert_eq!(icon, Icons::ERROR.to_string());
}

#[test]
fn test_icons_for_status_disabled_is_pause() {
    let icon = Icons::for_status(ItemStatus::Disabled).to_string();
    assert_eq!(icon, Icons::PAUSE.to_string());
}

// ============================================================================
// Icons for_message_level Tests
// ============================================================================

#[test]
fn test_icons_for_message_level_success_is_checkmark() {
    let icon = Icons::for_message_level(MessageLevel::Success).to_string();
    assert_eq!(icon, Icons::CHECKMARK.to_string());
}

#[test]
fn test_icons_for_message_level_warning_is_warning() {
    let icon = Icons::for_message_level(MessageLevel::Warning).to_string();
    assert_eq!(icon, Icons::WARNING.to_string());
}

#[test]
fn test_icons_for_message_level_error_is_error() {
    let icon = Icons::for_message_level(MessageLevel::Error).to_string();
    assert_eq!(icon, Icons::ERROR.to_string());
}

#[test]
fn test_icons_for_message_level_info_is_info() {
    let icon = Icons::for_message_level(MessageLevel::Info).to_string();
    assert_eq!(icon, Icons::INFO.to_string());
}
