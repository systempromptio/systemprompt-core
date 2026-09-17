//! Small builders for OTLP attribute lists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use opentelemetry_proto::tonic::common::v1::any_value::Value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue};
use opentelemetry_proto::tonic::resource::v1::Resource;

pub(super) const SERVICE_NAME: &str = "systemprompt";
pub(super) const SCOPE_NAME: &str = "systemprompt.gateway";

#[derive(Debug, Default)]
pub(super) struct Attrs(Vec<KeyValue>);

impl Attrs {
    #[must_use]
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn text(&mut self, key: &str, value: &str) -> &mut Self {
        self.push(key, Value::StringValue(value.to_owned()));
        self
    }

    pub(super) fn opt_str(&mut self, key: &str, value: Option<&str>) -> &mut Self {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            self.text(key, value);
        }
        self
    }

    pub(super) fn int(&mut self, key: &str, value: i64) -> &mut Self {
        self.push(key, Value::IntValue(value));
        self
    }

    pub(super) fn opt_int(&mut self, key: &str, value: Option<impl Into<i64>>) -> &mut Self {
        if let Some(value) = value {
            self.int(key, value.into());
        }
        self
    }

    pub(super) fn float(&mut self, key: &str, value: f64) -> &mut Self {
        self.push(key, Value::DoubleValue(value));
        self
    }

    pub(super) fn flag(&mut self, key: &str, value: bool) -> &mut Self {
        self.push(key, Value::BoolValue(value));
        self
    }

    fn push(&mut self, key: &str, value: Value) {
        self.0.push(KeyValue {
            key: key.to_owned(),
            value: Some(AnyValue { value: Some(value) }),
            key_strindex: 0,
        });
    }

    #[must_use]
    pub(super) fn finish(&mut self) -> Vec<KeyValue> {
        std::mem::take(&mut self.0)
    }
}

#[must_use]
pub(super) fn resource(instance_id: Option<&str>) -> Resource {
    let mut attrs = Attrs::new();
    attrs
        .text("service.name", SERVICE_NAME)
        .text("service.version", env!("CARGO_PKG_VERSION"))
        .opt_str("service.instance.id", instance_id);
    Resource {
        attributes: attrs.finish(),
        dropped_attributes_count: 0,
        entity_refs: Vec::new(),
    }
}

#[must_use]
pub(super) fn scope() -> InstrumentationScope {
    InstrumentationScope {
        name: SCOPE_NAME.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        attributes: Vec::new(),
        dropped_attributes_count: 0,
    }
}
