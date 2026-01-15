use std::fmt::{self, Write};
use tracing::field::{Field, Visit};
use tracing_subscriber::field::{MakeVisitor, VisitFmt, VisitOutput};
use tracing_subscriber::fmt::format::Writer;

#[derive(Debug, Clone, Copy, Default)]
pub struct FilterSystemFields;

impl FilterSystemFields {
    pub fn new() -> Self {
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
    fn new(writer: Writer<'a>) -> Self {
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
        if debug_str == "\"system\"" || debug_str == "system" {
            return;
        }

        self.result = self.write_field(field.name(), &debug_str);
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
        if value == "system" {
            return;
        }
        self.result = self.write_field(field.name(), &format!("{:?}", value));
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
