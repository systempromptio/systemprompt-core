mod types;

pub use types::TemplateDataParams;

use anyhow::Result;
use serde_json::Value;

pub async fn prepare_template_data(params: TemplateDataParams<'_>) -> Result<Value> {
    Ok(serde_json::json!({
        "CONTENT": params.content_html,
        "TOC_HTML": params.toc_html,
        "SLUG": params.slug,
    }))
}
