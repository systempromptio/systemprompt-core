//! Console formatter that filters system fields and renders structured values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::{self, Write};
use tracing::field::{Field, Visit};
use tracing_subscriber::field::{MakeVisitor, VisitFmt, VisitOutput};
use tracing_subscriber::fmt::format::Writer;

use crate::sanitize::{REDACTION_PLACEHOLDER, escape_control, is_redacted, is_system_sentinel};

#[derive(Debug, Clone, Copy, Default)]
pub struct FilterSystemFields;

impl FilterSystemFields {
    pub const fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub struct FilteringVisitor<'a> {
    writer: Writer<'a>,
    is_first: bool,
    result: fmt::Result,
}

impl<'a> FilteringVisitor<'a> {
    const fn new(writer: Writer<'a>) -> Self {
        Self {
            writer,
            is_first: true,
            result: Ok(()),
        }
    }

    fn record_filtered(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.result.is_err() {
            return;
        }

        let debug_str = format!("{:?}", value);
        if is_system_sentinel(&debug_str) {
            return;
        }

        self.write_value(field.name(), &debug_str);
    }

    fn write_value(&mut self, name: &str, rendered: &str) {
        let safe = if is_redacted(name) {
            REDACTION_PLACEHOLDER.to_owned()
        } else {
            escape_control(rendered)
        };
        self.result = self.write_field(name, &safe);
    }

    fn write_field(&mut self, name: &str, value: &str) -> fmt::Result {
        if self.is_first {
            self.is_first = false;
        } else {
            self.writer.write_char(' ')?;
        }
        write!(self.writer, "{}={}", name, value)
    }
}

impl Visit for FilteringVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_filtered(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if self.result.is_err() {
            return;
        }
        if is_system_sentinel(value) {
            return;
        }
        self.write_value(field.name(), &format!("{:?}", value));
    }
}

impl VisitOutput<fmt::Result> for FilteringVisitor<'_> {
    fn finish(self) -> fmt::Result {
        self.result
    }
}

impl VisitFmt for FilteringVisitor<'_> {
    fn writer(&mut self) -> &mut dyn Write {
        &mut self.writer
    }
}

impl<'a> MakeVisitor<Writer<'a>> for FilterSystemFields {
    type Visitor = FilteringVisitor<'a>;

    fn make_visitor(&self, target: Writer<'a>) -> Self::Visitor {
        FilteringVisitor::new(target)
    }
}
