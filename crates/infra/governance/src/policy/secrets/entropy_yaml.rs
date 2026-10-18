//! Reads the optional `entropy` block of a `secret_scan` policy entry,
//! reporting mistyped keys and skipping allowlist expressions that do not
//! compile so a typo never disables the detector silently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use regex::Regex;
use serde_yaml::Value as YamlValue;

use super::EntropyConfig;

const ENTROPY_KEYS: [&str; 4] = ["enabled", "min_len", "threshold", "allowlist"];

pub(super) fn from_yaml(v: &YamlValue) -> EntropyConfig {
    let defaults = EntropyConfig::default();
    let Some(block) = v.get("entropy") else {
        return defaults;
    };
    report_entropy_block_typos(block);
    let allowlist = block
        .get("allowlist")
        .and_then(YamlValue::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(YamlValue::as_str)
                .filter_map(|expr| match Regex::new(expr) {
                    Ok(re) => Some(re),
                    Err(error) => {
                        tracing::error!(
                            %expr,
                            %error,
                            "secret_scan: entropy.allowlist entry skipped; regex failed to compile"
                        );
                        None
                    },
                })
                .collect()
        })
        .unwrap_or(defaults.allowlist);
    EntropyConfig {
        enabled: block
            .get("enabled")
            .and_then(YamlValue::as_bool)
            .unwrap_or(defaults.enabled),
        min_len: block
            .get("min_len")
            .and_then(YamlValue::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(defaults.min_len),
        threshold: block
            .get("threshold")
            .and_then(YamlValue::as_f64)
            .unwrap_or(defaults.threshold),
        allowlist,
    }
}

fn report_entropy_block_typos(block: &YamlValue) {
    let Some(map) = block.as_mapping() else {
        tracing::error!(
            "secret_scan: `entropy` is not a mapping; the block is ignored and \
             built-in defaults apply"
        );
        return;
    };
    for key in map.keys() {
        let name = key.as_str().unwrap_or("<non-string>");
        if !ENTROPY_KEYS.contains(&name) {
            tracing::error!(
                key = %name,
                "secret_scan: unknown `entropy` key ignored; valid keys are \
                 enabled, min_len, threshold, allowlist"
            );
        }
    }
    let wrong_shape = [
        (
            "enabled",
            map.get("enabled").is_some_and(|v| v.as_bool().is_none()),
        ),
        (
            "min_len",
            map.get("min_len").is_some_and(|v| v.as_u64().is_none()),
        ),
        (
            "threshold",
            map.get("threshold").is_some_and(|v| v.as_f64().is_none()),
        ),
        (
            "allowlist",
            map.get("allowlist")
                .is_some_and(|v| v.as_sequence().is_none()),
        ),
    ];
    for (name, mistyped) in wrong_shape {
        if mistyped {
            tracing::error!(
                key = %name,
                "secret_scan: entropy key has the wrong type; the built-in \
                 default is used instead"
            );
        }
    }
}
