use serde_json::Value;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct TemplateDataParams<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub popular_ids: &'a [String],
    pub config: &'a serde_yaml::Value,
    pub web_config: &'a serde_yaml::Value,
    pub content_html: &'a str,
    pub url_pattern: &'a str,
    pub db_pool: DbPool,
}

pub struct DateData {
    pub formatted: String,
    pub iso: String,
    pub modified_formatted: String,
    pub modified_iso: String,
}

pub struct ImageData {
    pub featured: String,
    pub absolute_url: String,
    pub hero: String,
    pub hero_alt: String,
}

pub struct ContentData {
    pub related_html: String,
    pub references_html: String,
    pub social_html: String,
    pub header_cta_url: String,
    pub banner_cta_url: String,
    pub toc_html: String,
    pub sections_html: String,
}

pub struct OrgConfig<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub logo: &'a str,
}

pub struct ArticleConfig<'a> {
    pub article_type: &'a str,
    pub section: &'a str,
    pub language: &'a str,
}

pub struct BrandingData<'a> {
    pub author: &'a str,
    pub twitter_handle: &'a str,
    pub display_sitename: bool,
    pub logo_path: &'a str,
    pub favicon_path: &'a str,
}

pub struct BuildTemplateJsonParams<'a> {
    pub item: &'a Value,
    pub content_html: &'a str,
    pub slug: &'a str,
    pub canonical_path: &'a str,
    pub date_data: &'a DateData,
    pub image_data: &'a ImageData,
    pub content_data: &'a ContentData,
    pub navigation: &'a (String, String, String),
    pub org: OrgConfig<'a>,
    pub article: ArticleConfig<'a>,
    pub branding: BrandingData<'a>,
}
