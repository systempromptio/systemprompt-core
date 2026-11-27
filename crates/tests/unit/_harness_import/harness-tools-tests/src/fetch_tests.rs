use std::path::Path;

use harness_tools::{FetchConfig, FetchTool, OutputLimits, Tool, ToolError, ToolOutput};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{invalid_message, recorded};

const PAGE: &str = r#"<!doctype html><html><head><title>T</title><style>p{}</style></head>
<body><h1>Hello &amp; welcome</h1><!-- note --><p>Some <b>bold</b> and <a href="https://x.dev/a">a link</a>.</p>
<script>alert("<p>no</p>")</script><ul><li>one</li><li>two</li></ul><pre>let  x = 1;
</pre><p>a &lt; b</p></body></html>"#;

async fn serve(body: Vec<u8>, content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
        stream.shutdown().await.unwrap();
    });
    format!("http://{address}/files/page.html")
}

async fn fetch(root: &Path, url: &str, format: &str) -> Result<ToolOutput, ToolError> {
    let (ctx, _) = recorded(root);
    let tool = FetchTool::new(FetchConfig::default(), OutputLimits::default());
    tool.run(json!({"url": url, "format": format}), &ctx).await
}

async fn fetch_html(html: &str, format: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let url = serve(html.as_bytes().to_vec(), "text/html; charset=utf-8").await;
    fetch(dir.path(), &url, format).await.unwrap().output
}

#[tokio::test]
async fn renders_markdown() {
    assert_eq!(
        fetch_html(PAGE, "markdown").await,
        "# Hello & welcome\n\nSome **bold** and [a link](https://x.dev/a).\n\n- one\n- two\n\n```\nlet  x = 1;\n```\n\na < b"
    );
}

#[tokio::test]
async fn renders_plain_text() {
    let text = fetch_html(PAGE, "text").await;
    assert!(text.starts_with("Hello & welcome\n\nSome bold and a link."));
    assert!(!text.contains("alert"));
    assert!(text.contains("- one\n- two"));
}

#[tokio::test]
async fn raw_html_is_returned_as_it_is() {
    assert_eq!(fetch_html(PAGE, "html").await, PAGE);
}

#[tokio::test]
async fn decodes_entities() {
    assert_eq!(
        fetch_html("<p>&#65;&#x42;&quot;&bogus;&</p>", "text").await,
        "AB\"&bogus;&"
    );
}

#[tokio::test]
async fn ordered_lists_count_from_start() {
    let html = "<ol start=\"3\"><li>a</li><li>b</li></ol>";
    assert_eq!(fetch_html(html, "markdown").await, "3. a\n4. b");
}

#[tokio::test]
async fn images_come_back_as_attachments() {
    let dir = tempfile::tempdir().unwrap();
    let url = serve(b"\x89PNG\r\n\x1a\n".to_vec(), "image/png").await;
    let out = fetch(dir.path(), &url, "markdown").await.unwrap();
    assert_eq!(out.attachments.len(), 1);
    assert_eq!(out.attachments[0].mime, "image/png");
    assert_eq!(out.attachments[0].filename.as_deref(), Some("page.html"));
}

#[tokio::test]
async fn only_http_urls_are_fetched() {
    let dir = tempfile::tempdir().unwrap();
    let err = fetch(dir.path(), "ftp://example.com/a", "markdown")
        .await
        .unwrap_err();
    assert!(invalid_message(&err).contains("only http:// and https://"));
    let malformed = fetch(dir.path(), "not a url", "markdown")
        .await
        .unwrap_err();
    assert!(matches!(malformed, ToolError::Malformed { .. }));
    assert!(
        malformed
            .to_string()
            .starts_with("invalid input: invalid URL: ")
    );
}
