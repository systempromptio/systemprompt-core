//! Pure-function sitemap XML serialisation: turns slices of [`SitemapUrl`]
//! into the `urlset` and `sitemapindex` documents required by the sitemaps.org
//! 0.9 protocol.

use chrono::Utc;

/// A single `<url>` entry inside a sitemap document.
#[derive(Debug, Clone)]
pub struct SitemapUrl {
    /// Absolute URL of the page (`<loc>`).
    pub loc: String,
    /// `YYYY-MM-DD` last-modified date (`<lastmod>`).
    pub lastmod: String,
    /// Change-frequency hint (`<changefreq>`): e.g. `daily`, `weekly`.
    pub changefreq: String,
    /// Priority (`<priority>`) in the range `0.0..=1.0`.
    pub priority: f32,
}

/// Serialise `urls` into a single `urlset` sitemap XML document.
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

/// Serialise a sitemap index pointing at `sitemap-1.xml`, `sitemap-2.xml`, …
/// for each chunk, hosted under `base_url`.
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

/// XML-escape the five reserved characters (`&`, `<`, `>`, `"`, `'`).
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
