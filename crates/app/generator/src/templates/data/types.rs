use serde_json::Value;
use systemprompt_database::DbPool;
use systemprompt_models::FullWebConfig;

#[derive(Debug)]
pub struct TemplateDataParams<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub popular_ids: &'a [String],
    pub config: &'a serde_yaml::Value,
    pub web_config: &'a FullWebConfig,
    pub content_html: &'a str,
    pub toc_html: &'a str,
    pub url_pattern: &'a str,
    pub db_pool: DbPool,
    pub slug: &'a str,
}
