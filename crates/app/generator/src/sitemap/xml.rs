//! Pure-function sitemap XML serialisation: turns slices of [`SitemapUrl`]
//! into the `urlset` and `sitemapindex` documents required by the sitemaps.org
//! 0.9 protocol. The `xhtml:link` namespace is declared on every `<urlset>`
//! so per-URL `<xhtml:link rel="alternate" hreflang="…">` entries validate
//! against Google's hreflang spec.

use chrono::Utc;

#[derive(Debug, Clone)]
pub struct SitemapUrlAlternate {
    pub hreflang: String,
    pub href: String,
}

#[derive(Debug, Clone)]
pub struct SitemapUrl {
    pub loc: String,
    pub lastmod: String,
    pub changefreq: String,
    pub priority: f32,
    pub alternates: Vec<SitemapUrlAlternate>,
}

pub fn build_sitemap_xml(urls: &[SitemapUrl]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
"#,
    );

    for url in urls {
        xml.push_str(&format!(
            "  <url>\n    <loc>{}</loc>\n    <lastmod>{}</lastmod>\n    \
             <changefreq>{}</changefreq>\n    <priority>{:.1}</priority>\n",
            escape_xml(&url.loc),
            escape_xml(&url.lastmod),
            escape_xml(&url.changefreq),
            url.priority
        ));
        for alt in &url.alternates {
            xml.push_str(&format!(
                "    <xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>\n",
                escape_xml(&alt.hreflang),
                escape_xml(&alt.href),
            ));
        }
        xml.push_str("  </url>\n");
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
