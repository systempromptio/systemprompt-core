//! The small HTML page each loopback connection is answered with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy)]
pub(super) enum Page {
    SignedIn,
    Denied,
    Invalid,
}

pub(super) async fn respond(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Referrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await?;
    stream.shutdown().await
}

pub(super) fn render(page: Page, product: &str) -> String {
    let product = escape_html(product);
    let (title, heading, text) = match page {
        Page::SignedIn => (
            "Signed in",
            "You're signed in".to_owned(),
            format!("Head back to {product} to carry on. This tab can be closed."),
        ),
        Page::Denied => (
            "Sign-in declined",
            "Sign-in declined".to_owned(),
            format!("Nothing was shared with {product}. This tab can be closed."),
        ),
        Page::Invalid => (
            "Link not valid",
            "This sign-in link isn't valid".to_owned(),
            format!("It may have been used already. Start signing in again from {product}."),
        ),
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width,initial-scale=1\"><title>{title} - {product}</title><style>\
         :root{{color-scheme:light dark}}body{{margin:0;min-height:100vh;display:grid;place-items:center;\
         font:15px/1.5 system-ui,-apple-system,\"Segoe UI\",sans-serif;background:Canvas;color:CanvasText}}\
         main{{max-width:28rem;padding:2rem;text-align:center}}h1{{font-size:1.4rem;margin:0 0 .5rem}}\
         p{{margin:0;opacity:.75}}</style></head><body><main><h1>{heading}</h1><p>{text}</p></main></body></html>"
    )
}

fn escape_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
