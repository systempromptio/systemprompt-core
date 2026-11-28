//! The fetch tool against a local server: conversion of pages, size and time
//! limits, redirects and what is asked before anything is fetched.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use systemprompt_engine::tools::{
    FetchConfig, FetchTool, OutputLimits, Tool, ToolError, ToolOutput,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{Gate, context};

#[derive(Debug, Clone)]
struct Reply {
    status: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    stall: bool,
    declare: bool,
}

fn page(content_type: &str, body: impl Into<Vec<u8>>) -> Reply {
    Reply {
        status: "200 OK",
        headers: vec![("Content-Type".to_owned(), content_type.to_owned())],
        body: body.into(),
        stall: false,
        declare: true,
    }
}

fn redirect(to: &str) -> Reply {
    Reply {
        status: "302 Found",
        headers: vec![("Location".to_owned(), to.to_owned())],
        body: Vec::new(),
        stall: false,
        declare: true,
    }
}

async fn serve<F>(route: F) -> String
where
    F: Fn(&str) -> Reply + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let route = Arc::new(route);
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let route = Arc::clone(&route);
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let head = String::from_utf8_lossy(&request).into_owned();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let reply = route(&path);
                if reply.stall {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    return;
                }
                let mut text = format!("HTTP/1.1 {}\r\nConnection: close\r\n", reply.status);
                for (name, value) in &reply.headers {
                    text.push_str(&format!("{name}: {value}\r\n"));
                }
                if reply.declare {
                    text.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
                }
                text.push_str("\r\n");
                let written = stream.write_all(text.as_bytes()).await;
                if written.is_ok() && stream.write_all(&reply.body).await.is_ok() {
                    drop(stream.shutdown().await);
                }
            });
        }
    });
    format!("http://{address}")
}

fn tool(config: FetchConfig) -> FetchTool {
    FetchTool::new(config, OutputLimits::default())
}

async fn fetch_with(
    root: &Path,
    config: FetchConfig,
    input: serde_json::Value,
    gate: Gate,
) -> (Result<ToolOutput, ToolError>, Arc<Gate>) {
    let gate = Arc::new(gate);
    let ctx = context(root, Arc::clone(&gate));
    (tool(config).run(input, &ctx).await, gate)
}

pub(crate) async fn markdown(html: &'static str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(move |_| page("text/html", html)).await;
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/p")}),
        Gate::default(),
    )
    .await;
    result.unwrap().output
}

async fn text(html: &'static str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(move |_| page("text/html", html)).await;
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/p"), "format": "text"}),
        Gate::default(),
    )
    .await;
    result.unwrap().output
}

#[tokio::test]
async fn headings_emphasis_links_and_breaks_become_markdown() {
    let out = markdown("<h2>Title</h2><p><em>soft</em> <strong>hard</strong> <a href=\"https://a.b/c\">link</a><br>next</p><hr><p>after</p>").await;
    assert!(out.starts_with("## Title\n\n"), "{out}");
    assert!(out.contains("*soft*") || out.contains("_soft_"), "{out}");
    assert!(out.contains("**hard**"), "{out}");
    assert!(out.contains("[link](https://a.b/c)"), "{out}");
    assert!(out.contains("after"), "{out}");
}

#[tokio::test]
async fn images_keep_their_alternative_text_and_address() {
    let out = markdown("<p><img src=\"https://a.b/i.png\" alt=\"chart\"></p>").await;
    assert!(out.contains("![chart](https://a.b/i.png)"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: a code fence can be closed early by fence characters inside the code"]
async fn preformatted_code_is_kept_verbatim_in_a_fence_its_content_cannot_close() {
    let out = markdown("<pre>a  b\n```\nc</pre>").await;
    assert!(out.contains("a  b\n```\nc"), "{out}");
    let fence = out.lines().next().unwrap_or_default().to_owned();
    assert!(fence.starts_with("````"), "{out}");
}

#[tokio::test]
async fn inline_code_is_kept_verbatim() {
    let out = markdown("<p>call <code>a*b_c</code> now</p>").await;
    assert!(out.contains("`a*b_c`"), "{out}");
}

#[tokio::test]
async fn nested_lists_stay_readable() {
    let out = markdown("<ul><li>one<ul><li>inner</li></ul></li><li>two</li></ul>").await;
    assert!(out.contains("- one"), "{out}");
    assert!(out.contains("  - inner"), "{out}");
    assert!(out.contains("- two"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: blockquote content is not prefixed with a quote marker"]
async fn blockquotes_become_quoted_markdown() {
    let quote = markdown("<blockquote><p>quoted</p></blockquote>").await;
    assert!(quote.contains("> quoted"), "{quote}");
}

#[tokio::test]
#[ignore = "parity ledger: table rows have no leading pipe and no header separator row"]
async fn tables_become_markdown() {
    let table =
        markdown("<table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>").await;
    assert!(table.contains("| a | b |"), "{table}");
    assert!(table.contains("| 1 | 2 |"), "{table}");
}

#[tokio::test]
async fn entities_are_decoded_and_whitespace_collapsed_without_joining_words() {
    let out = markdown("<p>fish &amp;\n\n   chips&nbsp;&copy;</p><p>one</p><p>two</p>").await;
    assert!(out.contains("fish & chips"), "{out}");
    assert!(out.contains('\u{a9}'), "{out}");
    assert!(out.contains("one\n\ntwo"), "{out}");
}

#[tokio::test]
async fn scripts_styles_head_and_hidden_content_are_dropped() {
    let out = markdown("<html><head><title>T</title><style>p{}</style></head><body><script>bad()</script><noscript>no</noscript><p>keep</p></body></html>").await;
    assert!(out.contains("keep"), "{out}");
    for absent in ["bad()", "p{}", "T\n"] {
        assert!(!out.contains(absent), "{absent} in {out}");
    }
}

#[tokio::test]
#[ignore = "parity ledger: hidden elements and templates are rendered"]
async fn hidden_and_template_content_is_dropped() {
    let out = markdown("<p>keep</p><div hidden>secret</div><template>tpl</template>").await;
    assert!(!out.contains("secret") && !out.contains("tpl"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: text that reads as markdown syntax is not escaped"]
async fn text_that_would_become_markup_is_escaped() {
    let out = markdown("<p># not a heading</p><p>1. not a list</p><p>*not emphasis*</p>").await;
    assert!(out.contains("\\# not a heading"), "{out}");
    assert!(out.contains("\\*not emphasis\\*"), "{out}");
}

#[tokio::test]
async fn conversion_is_deterministic_and_bounded_for_malformed_input() {
    let html: &'static str =
        "<div><p><b><i>unclosed <a href='x'>link <ul><li>item</p></div></table><<>>";
    let first = markdown(html).await;
    let second = markdown(html).await;
    assert_eq!(first, second);
    assert!(first.contains("unclosed"));
    assert!(first.len() < 200, "{first}");
}

#[tokio::test]
async fn plain_text_drops_markup_and_active_content() {
    let out = text("<h1>Head</h1><p>Some <b>bold</b></p><script>x()</script>").await;
    assert!(out.starts_with("Head\n\nSome bold"), "{out}");
    assert!(!out.contains("x()"));
}

#[tokio::test]
async fn details_are_shown_according_to_visibility() {
    let out = markdown("<details><summary>More</summary><p>body</p></details>").await;
    assert!(out.contains("More"), "{out}");
}

#[tokio::test]
async fn a_code_block_at_the_output_limit_is_closed_and_cut_on_a_character() {
    let dir = tempfile::tempdir().unwrap();
    let html = format!("<pre>{}</pre>", "é".repeat(400));
    let base = serve(move |_| page("text/html", html.clone())).await;
    let gate = Arc::new(Gate::default());
    let ctx = context(dir.path(), gate);
    let tool = FetchTool::new(
        FetchConfig::default(),
        OutputLimits {
            max_lines: 100,
            max_bytes: 101,
        },
    );
    let out = tool
        .run(json!({"url": format!("{base}/p")}), &ctx)
        .await
        .unwrap();
    let shown = out
        .output
        .split("\n\n[Output truncated")
        .next()
        .unwrap()
        .to_owned();
    assert!(shown.trim_end().ends_with("```"), "{shown}");
}

#[tokio::test]
async fn a_non_http_address_is_refused_before_anything_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    for url in ["file:///etc/hosts", "ftp://a.b/c", "data:text/plain,hi"] {
        let (result, gate) = fetch_with(
            dir.path(),
            FetchConfig::default(),
            json!({"url": url}),
            Gate::default(),
        )
        .await;
        assert!(result.is_err(), "{url}");
        assert!(gate.asked().is_empty(), "{url}");
    }
}

#[tokio::test]
#[ignore = "parity ledger: permission is asked by host, and embedded credentials are not refused"]
async fn permission_is_asked_for_the_address_as_written_and_credentials_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(|_| page("text/plain", "ok")).await;
    let (result, gate) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/doc?q=1#part")}),
        Gate::default(),
    )
    .await;
    result.unwrap();
    assert_eq!(gate.asked()[0].patterns, [format!("{base}/doc?q=1")]);
    let with_user = base.replace("http://", "http://user:secret@");
    let (refused, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{with_user}/doc")}),
        Gate::default(),
    )
    .await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn a_refused_fetch_never_reaches_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let base = serve(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        page("text/plain", "ok")
    })
    .await;
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/x")}),
        Gate::refusing("fetch"),
    )
    .await;
    assert!(matches!(result, Err(ToolError::Rejected)));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
#[ignore = "parity ledger: redirects are followed without asking about their target"]
async fn a_redirect_is_followed_only_to_a_permitted_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = serve(|_| page("text/plain", "far")).await;
    let to = format!("{target}/far");
    let base = serve(move |_| redirect(&to)).await;
    let (_, gate) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/start")}),
        Gate::default(),
    )
    .await;
    assert_eq!(gate.asked().len(), 2);
}

#[tokio::test]
async fn a_redirect_to_another_scheme_or_an_endless_chain_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(|path: &str| {
        if path == "/ftp" {
            redirect("ftp://a.b/c")
        } else {
            redirect("/loop")
        }
    })
    .await;
    for path in ["/ftp", "/loop"] {
        let (result, _) = fetch_with(
            dir.path(),
            FetchConfig::default(),
            json!({"url": format!("{base}{path}")}),
            Gate::default(),
        )
        .await;
        assert!(result.is_err(), "{path}");
    }
}

#[tokio::test]
async fn a_body_over_the_limit_is_refused_whether_declared_or_streamed() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(|path: &str| {
        let mut reply = page("text/plain", vec![b'x'; 200]);
        reply.declare = path == "/declared";
        reply
    })
    .await;
    let config = FetchConfig {
        max_bytes: 100,
        ..FetchConfig::default()
    };
    for path in ["/declared", "/streamed"] {
        let (result, _) = fetch_with(
            dir.path(),
            config.clone(),
            json!({"url": format!("{base}{path}")}),
            Gate::default(),
        )
        .await;
        let message = result.unwrap_err().to_string();
        assert!(message.contains("100 byte limit"), "{path}: {message}");
    }
}

#[tokio::test]
async fn a_stalled_request_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(|_| Reply {
        stall: true,
        ..page("text/plain", "")
    })
    .await;
    let started = std::time::Instant::now();
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/slow"), "timeout": 1}),
        Gate::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn an_ordinary_forbidden_answer_is_not_retried() {
    let dir = tempfile::tempdir().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let base = serve(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Reply {
            status: "403 Forbidden",
            ..page("text/plain", "no")
        }
    })
    .await;
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/x")}),
        Gate::default(),
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("403"));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
#[ignore = "parity ledger: a bot-challenge answer is not retried"]
async fn a_bot_challenge_is_retried_once_with_a_plain_user_agent() {
    let dir = tempfile::tempdir().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let base = serve(move |_| {
        let seen = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if seen == 0 {
            Reply {
                status: "403 Forbidden",
                headers: vec![("x-bot-challenge".to_owned(), "1".to_owned())],
                ..page("text/html", "challenge")
            }
        } else {
            page("text/plain", "through")
        }
    })
    .await;
    let (result, _) = fetch_with(
        dir.path(),
        FetchConfig::default(),
        json!({"url": format!("{base}/x")}),
        Gate::default(),
    )
    .await;
    assert!(result.unwrap().output.contains("through"));
}
