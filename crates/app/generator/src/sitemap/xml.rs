use chrono::Utc;

#[derive(Debug, Clone)]
pub struct SitemapUrl {
    pub loc: String,
    pub lastmod: String,
    pub changefreq: String,
    pub priority: f32,
}

pub fn build_sitemap_xml(urls: &[SitemapUrl]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    for url in urls {
        xml.push_str(&format!(
            r"  <url>
    <loc>{}</loc>
    <lastmod>{}</lastmod>
    <changefreq>{}</changefreq>
    <priority>{:.1}</priority>
  </url>
",
            escape_xml(&url.loc),
            escape_xml(&url.lastmod),
            escape_xml(&url.changefreq),
            url.priority
        ));
    }

    xml.push_str("</urlset>");
    xml
}

pub fn build_sitemap_index(chunks: &[Vec<SitemapUrl>], base_url: &str) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    for (idx, _chunk) in chunks.iter().enumerate() {
        let filename = format!("sitemap-{}.xml", idx + 1);

        xml.push_str(&format!(
            r"  <sitemap>
    <loc>{}/sitemaps/{}</loc>
    <lastmod>{}</lastmod>
  </sitemap>
",
            base_url,
            filename,
            Utc::now().format("%Y-%m-%d")
        ));
    }

    xml.push_str("</sitemapindex>");
    xml
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
