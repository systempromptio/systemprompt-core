use super::html::{base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script, HtmlBuilder};
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct FormRenderer;

impl FormRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_fields(artifact: &Artifact) -> Vec<FormField> {
        let mut fields = Vec::new();

        if let Some(hints) = &artifact.metadata.rendering_hints {
            if let Some(field_defs) = hints.get("fields").and_then(JsonValue::as_array) {
                for def in field_defs {
                    if let Some(field) = FormField::from_json(def) {
                        fields.push(field);
                    }
                }
            }
        }

        for part in &artifact.parts {
            if let Some(data) = part.as_data() {
                if let Some(form_fields) = data.get("fields").and_then(JsonValue::as_array) {
                    for def in form_fields {
                        if let Some(field) = FormField::from_json(def) {
                            if !fields.iter().any(|f| f.name == field.name) {
                                fields.push(field);
                            }
                        }
                    }
                }
            }
        }

        fields
    }

    fn extract_submit_tool(artifact: &Artifact) -> Option<String> {
        artifact
            .metadata
            .rendering_hints
            .as_ref()
            .and_then(|h| h.get("submit_tool"))
            .and_then(JsonValue::as_str)
            .map(String::from)
    }
}

#[derive(Debug)]
struct FormField {
    name: String,
    label: String,
    field_type: String,
    required: bool,
    placeholder: Option<String>,
    default_value: Option<JsonValue>,
    options: Vec<FormOption>,
}

#[derive(Debug)]
struct FormOption {
    value: String,
    label: String,
}

impl FormField {
    fn from_json(value: &JsonValue) -> Option<Self> {
        let name = value.get("name").and_then(JsonValue::as_str)?.to_string();

        Some(Self {
            name: name.clone(),
            label: value
                .get("label")
                .and_then(JsonValue::as_str)
                .unwrap_or(&name)
                .to_string(),
            field_type: value
                .get("type")
                .and_then(JsonValue::as_str)
                .unwrap_or("text")
                .to_string(),
            required: value.get("required").and_then(JsonValue::as_bool).unwrap_or(false),
            placeholder: value.get("placeholder").and_then(JsonValue::as_str).map(String::from),
            default_value: value.get("default").cloned(),
            options: value
                .get("options")
                .and_then(JsonValue::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| {
                            let value = o.get("value").and_then(JsonValue::as_str)?.to_string();
                            let label = o.get("label").and_then(JsonValue::as_str).unwrap_or(&value).to_string();
                            Some(FormOption { value, label })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    fn render_html(&self) -> String {
        let required_attr = if self.required { " required" } else { "" };
        let placeholder_attr = self
            .placeholder
            .as_ref()
            .map(|p| format!(r#" placeholder="{}""#, html_escape(p)))
            .unwrap_or_default();

        let input_html = match self.field_type.as_str() {
            "textarea" => format!(
                r#"<textarea name="{name}" id="{name}" class="form-input"{required}{placeholder}>{value}</textarea>"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_value.as_ref().and_then(JsonValue::as_str).unwrap_or(""),
            ),
            "select" => {
                use std::fmt::Write;
                let options_html = self.options.iter().fold(String::new(), |mut acc, o| {
                    let selected = self
                        .default_value
                        .as_ref()
                        .and_then(JsonValue::as_str)
                        .is_some_and(|dv| dv == o.value);
                    let _ = write!(
                        acc,
                        r#"<option value="{value}"{selected}>{label}</option>"#,
                        value = html_escape(&o.value),
                        selected = if selected { " selected" } else { "" },
                        label = html_escape(&o.label),
                    );
                    acc
                });

                format!(
                    r#"<select name="{name}" id="{name}" class="form-input"{required}>{options}</select>"#,
                    name = html_escape(&self.name),
                    required = required_attr,
                    options = options_html,
                )
            }
            "checkbox" => {
                let checked = self.default_value.as_ref().and_then(JsonValue::as_bool).unwrap_or(false);
                format!(
                    r#"<input type="checkbox" name="{name}" id="{name}" class="form-checkbox"{checked}>"#,
                    name = html_escape(&self.name),
                    checked = if checked { " checked" } else { "" },
                )
            }
            "number" => format!(
                r#"<input type="number" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_value.as_ref().map(ToString::to_string).unwrap_or_default(),
            ),
            "email" => format!(
                r#"<input type="email" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_value.as_ref().and_then(JsonValue::as_str).unwrap_or(""),
            ),
            "date" => format!(
                r#"<input type="date" name="{name}" id="{name}" class="form-input"{required} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                value = self.default_value.as_ref().and_then(JsonValue::as_str).unwrap_or(""),
            ),
            _ => format!(
                r#"<input type="text" name="{name}" id="{name}" class="form-input"{required}{placeholder} value="{value}">"#,
                name = html_escape(&self.name),
                required = required_attr,
                placeholder = placeholder_attr,
                value = self.default_value.as_ref().and_then(JsonValue::as_str).unwrap_or(""),
            ),
        };

        let required_mark = if self.required { r#"<span class="required-mark">*</span>"# } else { "" };

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

#[async_trait]
impl UiRenderer for FormRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Form
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let fields = Self::extract_fields(artifact);
        let submit_tool = Self::extract_submit_tool(artifact);
        let title = artifact.name.as_deref().unwrap_or("Form");

        let fields_html: String = fields.iter().map(FormField::render_html).collect();

        let fields_json: Vec<JsonValue> = fields
            .iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "type": f.field_type,
                    "required": f.required
                })
            })
            .collect();

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <form id="mcp-form" class="mcp-form">
        {fields}
        <div class="form-actions">
            <button type="submit" class="submit-btn">Submit</button>
            <button type="reset" class="reset-btn">Reset</button>
        </div>
    </form>
    <div id="form-message" class="form-message" style="display: none;"></div>
</div>"#,
            title_html = if title.is_empty() {
                String::new()
            } else {
                format!(r#"<h1 class="mcp-app-title">{}</h1>"#, html_escape(title))
            },
            description_html = artifact
                .description
                .as_ref()
                .map(|d| format!(r#"<p class="mcp-app-description">{}</p>"#, html_escape(d)))
                .unwrap_or_default(),
            fields = fields_html,
        );

        let script = format!(
            "{bridge}\nwindow.FORM_FIELDS = {fields_json};\nwindow.FORM_SUBMIT_TOOL = {submit_tool};\n{app}",
            bridge = mcp_app_bridge_script(),
            fields_json = json_to_js_literal(&serde_json::json!(fields_json)),
            submit_tool = submit_tool.map_or_else(|| "null".to_string(), |t| format!("\"{t}\"")),
            app = include_str!("assets/js/form.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(form_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

const fn form_styles() -> &'static str {
    include_str!("assets/css/form.css")
}
