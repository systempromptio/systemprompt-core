//! Smoke tests for the CLI display helpers. Pure stderr output; we exercise the
//! rendering paths for coverage.

use systemprompt_logging::services::cli::MessageLevel;
use systemprompt_logging::services::cli::display::{message, section_header, subsection_header};

#[test]
fn display_messages_at_every_level() {
    message(MessageLevel::Info, "info");
    message(MessageLevel::Success, "ok");
    message(MessageLevel::Warning, "warn");
    message(MessageLevel::Error, "err");
}

#[test]
fn display_section_headers() {
    section_header("Section");
    subsection_header("Sub");
}
