//! Form field types extracted from artifact rendering hints/data.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::html_escape;
use super::typed::{lenient, lenient_vec};
use serde::{Deserialize, Deserializer};
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
pub(super) struct FormFieldSpec {
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    label: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    field_type: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    required: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    placeholder: Option<String>,
    // JSON: form artifact field default — any JSON scalar the producer supplies.
    #[serde(default, deserialize_with = "present")]
    default: Option<JsonValue>,
    #[serde(default, deserialize_with = "lenient_vec")]
    options: Vec<FormOptionSpec>,
}

#[derive(Debug, Deserialize)]
struct FormOptionSpec {
    #[serde(default, deserialize_with = "lenient")]
    value: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    label: Option<String>,
}

// JSON: form artifact field default — an explicit `null` is kept, not dropped.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<JsonValue>, D::Error> {
    JsonValue::deserialize(deserializer).map(Some)
}

#[derive(Debug)]
pub(super) struct FormField {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub placeholder: Option<String>,
    // JSON: form artifact field default — any JSON scalar the producer supplies.
    pub default_value: Option<JsonValue>,
    pub options: Vec<FormOption>,
}

#[derive(Debug)]
pub(super) struct FormOption {
    pub value: String,
    pub label: String,
}

impl FormField {
    pub(super) fn from_spec(spec: FormFieldSpec) -> Option<Self> {
        let name = spec.name?;

        Some(Self {
            label: spec.label.unwrap_or_else(|| name.clone()),
            name,
            field_type: spec.field_type.unwrap_or_else(|| "text".to_owned()),
            required: spec.required.unwrap_or(false),
            placeholder: spec.placeholder,
            default_value: spec.default,
            options: spec
                .options
                .into_iter()
                .filter_map(|o| {
                    let value = o.value?;
                    Some(FormOption {
                        label: o.label.unwrap_or_else(|| value.clone()),
                        value,
                    })
                })
                .collect(),
        })
    }

    fn render_select(&self, required_attr: &str) -> String {
        let options_html = self.options.iter().fold(String::new(), |mut acc, o| {
            let selected = self
                .default_value
                .as_ref()
                .and_then(JsonValue::as_str)
                .is_some_and(|dv| dv == o.value);
            acc.push_str(&format!(
                r#"<option value="{value}"{selected}>{label}</option>"#,
                value = html_escape(&o.value),
                selected = if selected { " selected" } else { "" },
                label = html_escape(&o.label),
            ));
            acc
        });

        format!(
            r#"<select name="{name}" id="{name}" class="form-input"{required}>{options}</select>"#,
            name = html_escape(&self.name),
            required = required_attr,
            options = options_html,
        )
    }

    fn default_str(&self) -> String {
        self.default_value
            .as_ref()
            .and_then(JsonValue::as_str)
            .map_or_else(String::new, html_escape)
    }

    fn render_input(&self, required_attr: &str, placeholder_attr: &str) -> String {
        match self.field_type.as_str() {
            "textarea" => format!(
                r#"<textarea name="{name}" id="{name}" class="form-input"{required}{placeholder}>{value}</textarea>"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_str(),
            ),
            "select" => self.render_select(required_attr),
            "checkbox" => {
                let checked = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);
                format!(
                    r#"<input type="checkbox" name="{name}" id="{name}" class="form-checkbox"{required}{checked}>"#,
                    name = html_escape(&self.name),
                    required = required_attr,
                    checked = if checked { " checked" } else { "" },
                )
            },
            "number" => format!(
                r#"<input type="number" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_value.as_ref().map_or_else(String::new, |v| {
                    v.as_str().map_or_else(|| v.to_string(), html_escape)
                }),
            ),
            "email" => format!(
                r#"<input type="email" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_str(),
            ),
            "date" => format!(
                r#"<input type="date" name="{name}" id="{name}" class="form-input"{required} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                value = self.default_str(),
            ),
            _ => format!(
                r#"<input type="text" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_str(),
            ),
        }
    }

    pub(super) fn render_html(&self) -> String {
        let required_attr = if self.required { " required" } else { "" };
        let placeholder_attr = self.placeholder.as_ref().map_or_else(String::new, |p| {
            format!(r#" placeholder="{}""#, html_escape(p))
        });

        let input_html = self.render_input(required_attr, &placeholder_attr);

        let required_mark = if self.required {
            r#"<span class="required-mark" aria-hidden="true">*</span><span class="visually-hidden"> (required)</span>"#
        } else {
            ""
        };

        if self.field_type == "checkbox" {
            return format!(
                r#"<div class="form-field form-field-inline">
    {input}
    <label for="{name}" class="form-label form-label-inline">{label}{required_mark}</label>
</div>"#,
                name = html_escape(&self.name),
                label = html_escape(&self.label),
                required_mark = required_mark,
                input = input_html,
            );
        }

        format!(
            r#"<div class="form-field">
    <label for="{name}" class="form-label">{label}{required_mark}</label>
    {input}
</div>"#,
            name = html_escape(&self.name),
            label = html_escape(&self.label),
            required_mark = required_mark,
            input = input_html,
        )
    }
}
