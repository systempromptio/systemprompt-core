//! What the webview loads and is allowed to load: the IPC bootstrap script
//! injected at document start, and the `sp://` custom-protocol handler that
//! serves the embedded web tree.
//!
//! Split from `native.rs`, which keeps window and webview construction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use wry::http::Response;
use wry::http::header::CONTENT_TYPE;

use super::native::SP_HOST;
use crate::web_assets::{self, Asset};

pub(super) const BRIDGE_BOOTSTRAP: &str = r#"
(function () {
  if (window.__bridge && window.__bridge.__installed) { return; }
  const pending = new Map();
  const subs = new Map();
  const bridge = {
    __installed: true,
    pending,
    subs,
    reply(id, payload) {
      const p = pending.get(id);
      if (!p) { return; }
      pending.delete(id);
      if (payload && payload.ok) { p.resolve(payload.value); }
      else { p.reject(payload && payload.error ? payload.error : { scope: "internal", code: "internal", message: "no payload" }); }
    },
    emit(channel, payload) {
      const set = subs.get(channel);
      if (!set) { return; }
      for (const cb of Array.from(set)) {
        try { cb(payload); } catch (e) { console.error("bridge subscriber threw", e); }
      }
    },
  };
  window.__bridge = bridge;
})();
"#;
pub(super) fn serve_custom_asset(request: &http::Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let uri = request.uri();
    let host_match = uri.host().is_none_or(|h| h == SP_HOST);
    if !host_match {
        return not_found();
    }
    let mut path = uri.path().to_owned();
    if path.is_empty() || path == "/" {
        "/index.html".clone_into(&mut path);
    }
    web_assets::lookup_path(&path).map_or_else(
        || {
            tracing::warn!(%path, "GUI asset not found; serving 404");
            not_found()
        },
        asset_response,
    )
}
fn asset_response(asset: Asset) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(asset.body);
    _ = response.headers_mut().insert(
        CONTENT_TYPE,
        http::HeaderValue::from_str(asset.content_type)
            .unwrap_or_else(|_| http::HeaderValue::from_static("application/octet-stream")),
    );
    _ = response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("no-store, must-revalidate"),
    );
    _ = response.headers_mut().insert(
        http::header::X_CONTENT_TYPE_OPTIONS,
        http::HeaderValue::from_static("nosniff"),
    );
    response
}
fn not_found() -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed::<'static, [u8]>(b"not found"));
    *response.status_mut() = http::StatusCode::NOT_FOUND;
    _ = response.headers_mut().insert(
        CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}
