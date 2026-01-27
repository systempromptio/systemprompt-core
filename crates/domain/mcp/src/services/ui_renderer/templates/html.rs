pub struct HtmlBuilder {
    title: String,
    styles: Vec<String>,
    scripts: Vec<String>,
    body: String,
}

impl HtmlBuilder {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            styles: Vec::new(),
            scripts: Vec::new(),
            body: String::new(),
        }
    }

    pub fn add_style(mut self, css: &str) -> Self {
        self.styles.push(css.to_string());
        self
    }

    pub fn add_script(mut self, js: &str) -> Self {
        self.scripts.push(js.to_string());
        self
    }

    pub fn body(mut self, html: &str) -> Self {
        self.body = html.to_string();
        self
    }

    pub fn build(self) -> String {
        let styles = if self.styles.is_empty() {
            String::new()
        } else {
            format!("<style>\n{}\n</style>", self.styles.join("\n"))
        };

        let scripts = if self.scripts.is_empty() {
            String::new()
        } else {
            format!("<script>\n{}\n</script>", self.scripts.join("\n"))
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
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

pub const fn base_styles() -> &'static str {
    include_str!("assets/css/base.css")
}

pub const fn mcp_app_bridge_script() -> &'static str {
    include_str!("assets/js/bridge.js")
}
