//! Development-only HTTP preview of the GUI web tree.
//!
//! The bridge's webview is Windows/macOS only (`mod gui` is cfg-gated off
//! elsewhere) and reads its assets over a wry custom protocol, never HTTP. That
//! left front-end work on the GUI unviewable on a Linux workstation: edit,
//! ship to Windows, screenshot by hand.
//!
//! This module serves the same tree over plain HTTP so a browser — or
//! Playwright — can render it anywhere. It is compiled only under the
//! `dev-preview` feature, which is not in `default`, so it cannot reach a
//! shipped binary.
//!
//! Two decisions worth keeping:
//!
//! * **Assets are read from disk first**, embedded table second. They are
//!   `include_str!`-embedded, so serving the embedded copy would cost a `cargo
//!   build` per CSS edit — which is exactly the friction this exists to remove.
//!   Disk-first means edit, refresh, done.
//! * **The shell comes from `web_assets::render_index()`**, not a copy. The
//!   preview gets the same placeholder substitution and brand-theme injection
//!   as the real app, so it cannot quietly drift from what ships.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod fixtures;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::stdio::{diag, print_line};

const MOCK_TAG: &str = "<script type=\"module\" src=\"/dev/mock-ipc.js\"></script>";

// Why: the mock has to be installed *before* the entry module, because
// components call `bridge.stateSnapshot()` from `connectedCallback` the moment
// they upgrade — ordered module scripts run in document order, and the mock's
// top-level await holds the entry module until the fixture has landed.
const ENTRY_TAG: &str = "<script type=\"module\" src=\"/assets/js/index.js\"";

#[derive(Debug)]
pub struct Options {
    pub port: u16,
    pub web_root: PathBuf,
}

pub fn serve(opts: &Options) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", opts.port))?;
    let port = listener.local_addr()?.port();
    print_line(&format!("bridge dev preview: http://127.0.0.1:{port}/"));
    print_line(&format!("  web root : {}", opts.web_root.display()));
    print_line(&format!(
        "  fixtures : {}",
        fixtures::names(&opts.web_root).join(", ")
    ));
    print_line(&format!(
        "  usage    : http://127.0.0.1:{port}/?fixture=stale"
    ));
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => handle(stream, opts),
            Err(e) => diag(&format!("dev-web: accept failed: {e}")),
        }
    }
    Ok(())
}

fn handle(mut stream: std::net::TcpStream, opts: &Options) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let target = line.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    let response = route(path, query, opts);
    let (status, content_type, body) = response;
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: \
         no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    _ = stream.write_all(head.as_bytes());
    _ = stream.write_all(&body);
    _ = stream.flush();
}

fn route(path: &str, query: &str, opts: &Options) -> (&'static str, &'static str, Vec<u8>) {
    match path {
        "/" | "/index.html" => {
            let disk = std::fs::read_to_string(opts.web_root.join("index.html")).ok();
            let shell = disk.map_or_else(crate::web_assets::render_index, |src| {
                crate::web_assets::render_index_from(&src)
            });
            let html = shell.replacen(ENTRY_TAG, &format!("{MOCK_TAG}{ENTRY_TAG}"), 1);
            ("200 OK", "text/html; charset=utf-8", html.into_bytes())
        },
        "/dev/state" => {
            let name = param(query, "fixture").unwrap_or_else(|| "healthy".to_owned());
            fixtures::load(&opts.web_root, &name).map_or_else(
                || {
                    (
                        "404 Not Found",
                        "application/json",
                        br#"{"error":"unknown fixture"}"#.to_vec(),
                    )
                },
                |json| ("200 OK", "application/json", json.into_bytes()),
            )
        },
        "/dev/fixtures" => (
            "200 OK",
            "application/json",
            serde_json::to_vec(&fixtures::names(&opts.web_root)).unwrap_or_default(),
        ),
        _ => serve_file(path, opts),
    }
}

fn serve_file(path: &str, opts: &Options) -> (&'static str, &'static str, Vec<u8>) {
    // Why: `/dev/*` resolves from disk or not at all — build.rs strips it from
    // the staged tree, so it is not merely unused in a shipped binary, it is
    // not in it.
    let rel = path
        .strip_prefix("/assets/")
        .or_else(|| path.strip_prefix('/'))
        .unwrap_or(path);
    if percent_decode(rel).contains("..") {
        return ("403 Forbidden", "text/plain", b"no".to_vec());
    }
    let on_disk = opts.web_root.join(rel);
    if on_disk.is_file()
        && let Ok(bytes) = std::fs::read(&on_disk)
    {
        return ("200 OK", content_type_for(&on_disk), bytes);
    }
    crate::web_assets::lookup_path(path).map_or_else(
        || ("404 Not Found", "text/plain", b"not found".to_vec()),
        |asset| ("200 OK", asset.content_type, asset.body.into_owned()),
    )
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(hi) = (bytes[i + 1] as char).to_digit(16)
            && let Some(lo) = (bytes[i + 2] as char).to_digit(16)
        {
            out.push(char::from(u8::try_from(hi * 16 + lo).unwrap_or(b'?')));
            i += 3;
        } else {
            out.push(char::from(bytes[i]));
            i += 1;
        }
    }
    out
}

fn content_type_for(path: &Path) -> &'static str {
    systemprompt_models::mime::http_content_type(path)
}

fn param(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}
