//! What the bridge shows as its name and sign-in copy for the active
//! instance.
//!
//! Each field resolves on its own: the branding the active gateway delivered
//! in its last verified manifest, then the branding in its cached discovery
//! document, then the compiled [`Brand`](crate::brand::Brand). A remote theme
//! stylesheet is honoured only from the signed manifest — discovery is served
//! unauthenticated, and a stylesheet can read and exfiltrate page content where
//! an image cannot. Identity values (binary name, config paths, keyring
//! service, environment prefix) never change at run time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::bridge::instance::InstanceBranding;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisplayBrand {
    pub app_name: String,
    pub window_title: String,
    pub sign_in_label: String,
    pub sign_in_hint: String,
    pub docs_url: String,
    pub contact_email: String,
    pub pitch_head: String,
    pub pitch_body: String,
    pub logo_url: Option<String>,
    pub theme_css_url: Option<String>,
}

impl Default for DisplayBrand {
    fn default() -> Self {
        Self::compose(crate::brand::brand(), None, None)
    }
}

impl DisplayBrand {
    #[must_use]
    pub fn compose(
        brand: &crate::brand::Brand,
        discovery: Option<&InstanceBranding>,
        delivered: Option<&InstanceBranding>,
    ) -> Self {
        let pick = |field: fn(&InstanceBranding) -> Option<&String>, fallback: &str| -> String {
            delivered
                .and_then(field)
                .or_else(|| discovery.and_then(field))
                .filter(|v| !v.trim().is_empty())
                .map_or_else(|| fallback.to_owned(), Clone::clone)
        };
        let name = pick(|b| b.display_name.as_ref(), brand.app_name);
        let window_title = if name == brand.app_name {
            brand.window_title.to_owned()
        } else {
            name.clone()
        };
        Self {
            window_title,
            sign_in_label: brand.sign_in_label.to_owned(),
            sign_in_hint: pick(|b| b.sign_in_hint.as_ref(), brand.sign_in_hint),
            docs_url: https_only(
                pick(|b| b.docs_url.as_ref(), brand.docs_url),
                brand.docs_url,
            ),
            contact_email: pick(|b| b.contact_email.as_ref(), brand.contact_email),
            pitch_head: brand.pitch_head.to_owned(),
            pitch_body: brand.pitch_body.to_owned(),
            logo_url: delivered
                .and_then(|b| b.logo_url.clone())
                .or_else(|| discovery.and_then(|b| b.logo_url.clone()))
                .filter(|u| is_https(u)),
            theme_css_url: delivered
                .and_then(|b| b.theme_css_url.clone())
                .filter(|u| is_https(u)),
            app_name: name,
        }
    }

    #[must_use]
    pub fn resolve() -> Self {
        let delivered = crate::config::gateways::active()
            .ok()
            .and_then(|profile| {
                let cfg = crate::config::read_file(&profile.config_file).ok()?;
                let gateway = crate::config::gateway_url_or_default(&cfg);
                crate::config::gateways::read_delivered(&profile)
                    .ok()
                    .flatten()
                    .filter(|d| {
                        d.gateway
                            .as_ref()
                            .is_some_and(|g| crate::config::same_origin(g, &gateway))
                    })
            })
            .and_then(|d| d.branding);
        let discovery = crate::gateway::discovery::cached().map(|d| d.branding);
        Self::compose(
            crate::brand::brand(),
            discovery.as_ref(),
            delivered.as_ref(),
        )
    }
}

fn is_https(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.scheme() == "https")
}

fn https_only(candidate: String, fallback: &str) -> String {
    if is_https(&candidate) || candidate == fallback {
        candidate
    } else {
        fallback.to_owned()
    }
}
