use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: DateTime<Utc>,
    pub guid: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RssChannel {
    pub title: String,
    pub link: String,
    pub description: String,
    pub items: Vec<RssItem>,
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn format_rfc2822(dt: &DateTime<Utc>) -> String {
    dt.format("%a, %d %b %Y %H:%M:%S +0000").to_string()
}

pub fn build_rss_xml(channel: &RssChannel) -> String {
    let mut xml = String::with_capacity(8192);

    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">"#);
    xml.push('\n');
    xml.push_str("<channel>\n");

    xml.push_str(&format!(
        "  <title>{}</title>\n",
        escape_xml(&channel.title)
    ));
    xml.push_str(&format!("  <link>{}</link>\n", escape_xml(&channel.link)));
    xml.push_str(&format!(
        "  <description>{}</description>\n",
        escape_xml(&channel.description)
    ));
    xml.push_str(&format!(
        r#"  <atom:link href="{}/feed.xml" rel="self" type="application/rss+xml"/>"#,
        escape_xml(&channel.link)
    ));
    xml.push('\n');

    for item in &channel.items {
        xml.push_str("  <item>\n");
        xml.push_str(&format!("    <title>{}</title>\n", escape_xml(&item.title)));
        xml.push_str(&format!("    <link>{}</link>\n", escape_xml(&item.link)));
        xml.push_str(&format!(
            "    <description>{}</description>\n",
            escape_xml(&item.description)
        ));
        xml.push_str(&format!(
            "    <pubDate>{}</pubDate>\n",
            format_rfc2822(&item.pub_date)
        ));
        xml.push_str(&format!(
            "    <guid isPermaLink=\"true\">{}</guid>\n",
            escape_xml(&item.guid)
        ));
        if let Some(ref author) = item.author {
            xml.push_str(&format!("    <author>{}</author>\n", escape_xml(author)));
        }
        xml.push_str("  </item>\n");
    }

    xml.push_str("</channel>\n");
    xml.push_str("</rss>\n");

    xml
}
