//! `{{key}}` placeholder substitution for template instantiation in the text
//! formats; an unknown key is left in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::TemplateParams;

pub(crate) fn substitute(template: &str, params: &TemplateParams) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let after_open = &rest[open + 2..];
        let Some(close) = after_open.find("}}") else {
            break;
        };
        out.push_str(&rest[..open]);
        let key = after_open[..close].trim();
        match params.values.get(key) {
            Some(value) => out.push_str(value),
            None => out.push_str(&rest[open..open + 2 + close + 2]),
        }
        rest = &after_open[close + 2..];
    }
    out.push_str(rest);
    out
}
