//! Form field types extracted from artifact rendering hints/data.

use super::html::html_escape;
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub(super) struct FormField {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub placeholder: Option<String>,
    pub default_value: Option<JsonValue>,
    pub options: Vec<FormOption>,
}

#[derive(Debug)]
pub(super) struct FormOption {
    pub value: String,
    pub label: String,
}

impl FormField {
    pub(super) fn from_json(value: &JsonValue) -> Option<Self> {
        let name = value.get("name").and_then(JsonValue::as_str)?.to_owned();

        Some(Self {
            name: name.clone(),
            label: value
                .get("label")
                .and_then(JsonValue::as_str)
                .unwrap_or(&name)
                .to_owned(),
            field_type: value
                .get("type")
                .and_then(JsonValue::as_str)
                .unwrap_or("text")
                .to_owned(),
            required: value
                .get("required")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
            placeholder: value
                .get("placeholder")
                .and_then(JsonValue::as_str)
                .map(String::from),
            default_value: value.get("default").cloned(),
            options: value
                .get("options")
                .and_then(JsonValue::as_array)
                .map_or_else(Vec::new, |arr| {
                    arr.iter()
                        .filter_map(|o| {
                            let value = o.get("value").and_then(JsonValue::as_str)?.to_owned();
                            let label = o
                                .get("label")
                                .and_then(JsonValue::as_str)
                                .unwrap_or(&value)
                                .to_owned();
                            Some(FormOption { value, label })
                        })
                        .collect()
                }),
        })
    }

    pub(super) fn render_html(&self) -> String {
        let required_attr = if self.required { " required" } else { "" };
        let placeholder_attr = self.placeholder.as_ref().map_or_else(String::new, |p| {
            format!(r#" placeholder="{}""#, html_escape(p))
        });

        let input_html = match self.field_type.as_str() {
            "textarea" => format!(
                r#"<textarea name="{name}" id="{name}" class="form-input"{required}{placeholder}>{value}</textarea>"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
            ),
            "select" => {
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
            },
            "checkbox" => {
                let checked = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);
                format!(
                    r#"<input type="checkbox" name="{name}" id="{name}" class="form-checkbox"{checked}>"#,
                    name = html_escape(&self.name),
                    checked = if checked { " checked" } else { "" },
                )
            },
            "number" => format!(
                r#"<input type="number" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self
                    .default_value
                    .as_ref()
                    .map_or_else(String::new, ToString::to_string),
            ),
            "email" => format!(
                r#"<input type="email" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
            ),
            "date" => format!(
                r#"<input type="date" name="{name}" id="{name}" class="form-input"{required} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                value = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
            ),
            _ => format!(
                r#"<input type="text" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self
                    .default_value
                    .as_ref()
                    .and_then(JsonValue::as_str)
                    .unwrap_or(""),
            ),
        };

        let required_mark = if self.required {
            r#"<span class="required-mark">*</span>"#
        } else {
            ""
        };

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
