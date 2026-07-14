//! Smoke tests for `DisplayUtils`. Pure stderr output; we exercise the
//! rendering paths for coverage.

use systemprompt_logging::services::cli::{DisplayUtils, MessageLevel};

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
