use anyhow::Result;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Debug, Clone)]
struct SitemapUrl {
    loc: String,
    lastmod: String,
    changefreq: String,
    priority: f32,
}

fn build_sitemap_xml(urls: &[SitemapUrl]) -> Result<String> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    for url in urls {
        xml.push_str(&format!(
            r#"  <url>
    <loc>{}</loc>
    <lastmod>{}</lastmod>
    <changefreq>{}</changefreq>
    <priority>{:.1}</priority>
  </url>
"#,
            escape_xml(&url.loc),
            escape_xml(&url.lastmod),
            escape_xml(&url.changefreq),
            url.priority
        ));
    }

    xml.push_str("</urlset>");
    Ok(xml)
}

fn build_sitemap_index(chunks: &[Vec<SitemapUrl>], base_url: &str) -> Result<String> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    for (idx, _chunk) in chunks.iter().enumerate() {
        let filename = format!("sitemap-{}.xml", idx + 1);

        xml.push_str(&format!(
            r#"  <sitemap>
    <loc>{}/sitemaps/{}</loc>
    <lastmod>{}</lastmod>
  </sitemap>
"#,
            base_url,
            filename,
            chrono::Utc::now().format("%Y-%m-%d")
        ));
    }

    xml.push_str("</sitemapindex>");
    Ok(xml)
}

mod sitemap_tests {
    use super::*;

    #[test]
    fn test_escape_xml_ampersand() {
        assert_eq!(escape_xml("a&b"), "a&amp;b");
    }

    #[test]
    fn test_escape_xml_tags() {
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_escape_xml_quotes() {
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_escape_xml_apostrophe() {
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn test_build_sitemap_xml() {
        let urls = vec![SitemapUrl {
            loc: "https://example.com/blog/test".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.8,
        }];

        let xml = build_sitemap_xml(&urls).unwrap();
        assert!(xml.contains("<url>"));
        assert!(xml.contains("https://example.com/blog/test"));
        assert!(xml.contains("<changefreq>weekly</changefreq>"));
        assert!(xml.contains("<priority>0.8</priority>"));
        assert!(xml.contains("</urlset>"));
    }

    #[test]
    fn test_build_sitemap_xml_escaping() {
        let urls = vec![SitemapUrl {
            loc: "https://example.com/blog/test?a=1&b=2".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.8,
        }];

        let xml = build_sitemap_xml(&urls).unwrap();
        assert!(xml.contains("&amp;"));
        assert!(!xml.contains("?a=1&b=2"));
    }

    #[test]
    fn test_build_sitemap_index() {
        let chunk1 = vec![SitemapUrl {
            loc: "url1".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.8,
        }];

        let chunk2 = vec![SitemapUrl {
            loc: "url2".to_string(),
            lastmod: "2024-01-02".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.8,
        }];

        let base_url = "https://example.com";
        let index = build_sitemap_index(&[chunk1, chunk2], base_url).unwrap();
        assert!(index.contains("<sitemapindex"));
        assert!(index.contains("https://example.com/sitemaps/sitemap-1.xml"));
        assert!(index.contains("https://example.com/sitemaps/sitemap-2.xml"));
        assert!(index.contains("</sitemapindex>"));
    }
}

mod markdown_tests {
    use comrak::{markdown_to_html, ComrakOptions};

    fn strip_first_h1(content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut found_h1 = false;

        for line in lines {
            let trimmed = line.trim();
            if !found_h1 && trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
                found_h1 = true;
                continue;
            }
            result.push(line);
        }

        result.join("\n")
    }

    fn render_markdown(content: &str) -> anyhow::Result<String> {
        let mut options = ComrakOptions::default();

        options.extension.strikethrough = true;
        options.extension.table = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.superscript = true;
        options.render.unsafe_ = false;

        let content_without_h1 = strip_first_h1(content);
        let html = markdown_to_html(&content_without_h1, &options);
        Ok(html)
    }

    fn extract_frontmatter(content: &str) -> Option<(serde_yaml::Value, String)> {
        if !content.starts_with("---") {
            return None;
        }

        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return None;
        }

        let frontmatter_str = parts[1].trim();
        let body = parts[2].to_string();

        serde_yaml::from_str(frontmatter_str)
            .ok()
            .map(|yaml| (yaml, body))
    }

    #[test]
    fn test_render_markdown_basic() {
        let md = "# Hello\n\nThis is **bold**.";
        let html = render_markdown(md).unwrap();
        assert!(!html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_render_markdown_strips_first_h1() {
        let md = "# Title\n\nContent here\n\n## Subtitle";
        let html = render_markdown(md).unwrap();
        assert!(!html.contains("<h1>"));
        assert!(html.contains("<h2>Subtitle</h2>"));
        assert!(html.contains("Content here"));
    }

    #[test]
    fn test_render_markdown_preserves_h2() {
        let md = "## Subtitle\n\nContent";
        let html = render_markdown(md).unwrap();
        assert!(html.contains("<h2>Subtitle</h2>"));
    }

    #[test]
    fn test_render_markdown_list() {
        let md = "- Item 1\n- Item 2";
        let html = render_markdown(md).unwrap();
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>Item 1</li>"));
        assert!(html.contains("<li>Item 2</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_render_markdown_table() {
        let md = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
        let html = render_markdown(md).unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("Header 1"));
        assert!(html.contains("Header 2"));
    }

    #[test]
    fn test_render_markdown_strikethrough() {
        let md = "~~strikethrough~~";
        let html = render_markdown(md).unwrap();
        assert!(html.contains("<del>strikethrough</del>"));
    }

    #[test]
    fn test_frontmatter_extraction() {
        let content = "---\ntitle: Test\nauthor: Edward\n---\n# Content";
        let (fm, body) = extract_frontmatter(content).unwrap();
        assert_eq!(fm["title"].as_str().unwrap(), "Test");
        assert_eq!(fm["author"].as_str().unwrap(), "Edward");
        assert!(body.contains("# Content"));
    }

    #[test]
    fn test_frontmatter_no_frontmatter() {
        let content = "# No frontmatter here";
        let result = extract_frontmatter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_frontmatter_invalid_yaml() {
        let content = "---\ninvalid: yaml: content: here\n---\nBody";
        let result = extract_frontmatter(content);
        assert!(result.is_none());
    }
}

mod api_tests {
    #[tokio::test]
    async fn test_fetch_content_invalid_url() {
        let result =
            fetch_content_from_api("http://invalid.local.test.nonexistent:9999", "blog").await;
        assert!(result.is_err());
    }

    async fn fetch_content_from_api(
        api_url: &str,
        source_id: &str,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        use anyhow::anyhow;

        let url = format!("{api_url}/api/v1/content/{source_id}");

        let response = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to connect to {}: {}", url, e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch {}: {} {}",
                source_id,
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown error".to_string())
            ));
        }

        let items: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse response from {}: {}", source_id, e))?;

        Ok(items)
    }
}
