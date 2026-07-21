//! HTML builder for artifact UI rendering.
//!
//! Every document built here opens with the generated `MCP_UI` constants and
//! closes with the frame-sizing script, so app code addresses protocol
//! methods through [`UiMethod`](systemprompt_models::mcp::UiMethod) rather
//! than string literals, and no renderer has to opt into host size
//! negotiation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::mcp::ui_method_js_constants;

#[derive(Debug)]
pub struct HtmlBuilder {
    title: String,
    styles: Vec<String>,
    scripts: Vec<String>,
    body: String,
}

impl HtmlBuilder {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            styles: Vec::new(),
            scripts: Vec::new(),
            body: String::new(),
        }
    }

    pub fn add_style(mut self, css: &str) -> Self {
        self.styles.push(css.to_owned());
        self
    }

    pub fn add_script(mut self, js: &str) -> Self {
        self.scripts.push(js.to_owned());
        self
    }

    pub fn body(mut self, html: &str) -> Self {
        html.clone_into(&mut self.body);
        self
    }

    pub fn build(self) -> String {
        let styles = if self.styles.is_empty() {
            String::new()
        } else {
            format!("<style>\n{}\n</style>", self.styles.join("\n"))
        };

        let scripts = {
            let mut all = vec![ui_method_js_constants()];
            all.extend(self.scripts);
            all.push(frame_script().to_owned());
            format!("<script>\n{}\n</script>", all.join("\n"))
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    {styles}
</head>
<body>
{body}
{scripts}
</body>
</html>"#,
            title = html_escape(&self.title),
            styles = styles,
            body = self.body,
            scripts = scripts,
        )
    }
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn json_to_js_literal(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

pub const fn base_styles() -> &'static str {
    include_str!("assets/css/base.css")
}

pub const fn mcp_app_bridge_script() -> &'static str {
    include_str!("assets/js/bridge.js")
}

pub const fn frame_script() -> &'static str {
    include_str!("assets/js/frame.js")
}

pub fn artifact_shell_template() -> String {
    include_str!("assets/html/artifact-shell.html")
        .replace(MCP_UI_CONSTANTS_PLACEHOLDER, &ui_method_js_constants())
}

const MCP_UI_CONSTANTS_PLACEHOLDER: &str = "/*MCP_UI_CONSTANTS*/";
