//! One rendered-HTML tier's `Content-Security-Policy`.
//!
//! The policy keeps the page an opaque-origin document (`sandbox` without
//! `allow-same-origin`), admits inline styles and embedded images, fonts and
//! media, and, as the tier says, the page's inline scripts and loads from
//! its listed origins, each written into the directives verbatim (a
//! `RemoteOrigin` is one source expression and nothing more). Every policy
//! carries `form-action 'none'` and `base-uri 'none'`, and the sandbox
//! never gains `allow-same-origin`, `allow-popups`, `allow-top-navigation`
//! or `allow-forms`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::HeaderValue;
use http::header::InvalidHeaderValue;

use super::rendered_html::RenderedHtmlTier;

pub(super) const LOCKED_CSP: &str = "sandbox; default-src 'none'; style-src 'unsafe-inline'; \
                                     img-src data: blob:; font-src data:; media-src data: blob:; \
                                     form-action 'none'; base-uri 'none'";

const EMBEDDED_SOURCES: [(&str, &str); 4] = [
    ("style-src", " 'unsafe-inline'"),
    ("img-src", " data: blob:"),
    ("font-src", " data:"),
    ("media-src", " data: blob:"),
];

impl RenderedHtmlTier {
    pub(super) fn csp(&self) -> Result<HeaderValue, InvalidHeaderValue> {
        let origins: String = self
            .remote_origins
            .iter()
            .flat_map(|origin| [" ", origin.as_str()])
            .collect();
        let mut csp = String::from(if self.scripts {
            "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval'"
        } else {
            "sandbox; default-src 'none'"
        });
        if self.scripts {
            csp.push_str(&origins);
        }
        for (directive, own) in EMBEDDED_SOURCES {
            csp.push_str("; ");
            csp.push_str(directive);
            csp.push_str(own);
            csp.push_str(&origins);
        }
        csp.push_str("; form-action 'none'; base-uri 'none'");
        HeaderValue::from_str(&csp)
    }
}
