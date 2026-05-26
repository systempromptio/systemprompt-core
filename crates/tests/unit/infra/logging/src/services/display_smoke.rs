//! Smoke tests for `DisplayUtils`, `StatusDisplay`, `ModuleItemDisplay`,
//! and `CollectionDisplay`. Pure stderr output; we exercise the rendering
//! paths for coverage and check that builder/Display implementations match.

use systemprompt_logging::services::cli::{
    CollectionDisplay, Display, DisplayUtils, ItemStatus, MessageLevel, ModuleItemDisplay,
    ModuleType, StatusDisplay,
};

#[test]
fn display_utils_messages_at_every_level() {
    DisplayUtils::message(MessageLevel::Info, "info");
    DisplayUtils::message(MessageLevel::Success, "ok");
    DisplayUtils::message(MessageLevel::Warning, "warn");
    DisplayUtils::message(MessageLevel::Error, "err");
}

#[test]
fn display_utils_section_headers() {
    DisplayUtils::section_header("Section");
    DisplayUtils::subsection_header("Sub");
}

#[test]
fn display_utils_item_with_and_without_detail() {
    DisplayUtils::item(ItemStatus::Valid, "name", None);
    DisplayUtils::item(ItemStatus::Failed, "broken", Some("reason"));
}

#[test]
fn display_utils_relationship_renders() {
    DisplayUtils::relationship(ModuleType::Module, "from", "to", ItemStatus::Valid);
}

#[test]
fn display_utils_module_status() {
    DisplayUtils::module_status("authz", "ready");
}

#[test]
fn display_utils_count_message_singular_and_plural() {
    DisplayUtils::count_message(MessageLevel::Info, 1, "user");
    DisplayUtils::count_message(MessageLevel::Info, 5, "user");
    DisplayUtils::count_message(MessageLevel::Info, 0, "user");
}

#[test]
fn status_display_builder_and_display() {
    let d = StatusDisplay::new(ItemStatus::Valid, "name");
    d.display();
    let d = StatusDisplay::new(ItemStatus::Failed, "name").with_detail("oops");
    d.display();
    assert_eq!(d.name, "name");
    assert_eq!(d.detail.as_deref(), Some("oops"));
}

#[test]
fn module_item_display_renders() {
    let m = ModuleItemDisplay::new(ModuleType::Module, "src.yaml", "dst.yaml", ItemStatus::Valid);
    m.display();
    assert_eq!(m.file, "src.yaml");
    assert_eq!(m.target, "dst.yaml");
}

#[test]
fn collection_display_with_count_and_without() {
    let coll = CollectionDisplay::new(
        "Things",
        vec![StatusDisplay::new(ItemStatus::Valid, "one")],
    );
    coll.display();

    let coll = CollectionDisplay::new(
        "Things",
        vec![StatusDisplay::new(ItemStatus::Valid, "one")],
    )
    .without_count();
    coll.display();
}

#[test]
fn collection_display_empty_is_a_noop() {
    let coll: CollectionDisplay<StatusDisplay> = CollectionDisplay::new("empty", vec![]);
    coll.display();
}
