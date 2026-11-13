//! `${VAR}` interpolation for services YAML.
//!
//! Every string scalar in a services file may carry a placeholder (provider
//! and deployment endpoints, MCP server URLs and `env` maps, agent URLs,
//! marketplace sources), with one exception: any value under a key in
//! [`VERBATIM_KEYS`] is authored text or a runtime-expanded command and is
//! never rewritten, so a literal `${…}` in a system prompt, skill
//! instructions, a description, a hook command or a skill's carried-through
//! frontmatter reaches its consumer unchanged. The services types carry no JSON
//! schema, so a resolved value always enters the tree as a string: a numeric or
//! boolean field does not accept a placeholder.
//!
//! [`ServicesInterpolation`] selects the unset-variable policy: `Resolve`
//! (configuration about to run) fails with an [`InterpolationError`] naming
//! the variable and key path; `Inspect` (validate, show, doctor) resolves
//! what is set and leaves an unset placeholder as its original text;
//! `Verbatim` (a read whose result is written back to disk) rewrites nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value;

use crate::env::{
    FieldKind, InterpolationError, PathSegment, interpolate_yaml,
    interpolate_yaml_keeping_unresolved,
};

pub const VERBATIM_KEYS: &[&str] = &[
    "system_prompt",
    "system_prompts",
    "system_prompt_overrides",
    "prompt",
    "instructions",
    "description",
    "body",
    "content",
    "example",
    "examples",
    "template",
    "status_message",
    "hooks",
    "command",
    "frontmatter",
];

/// The unset-variable policy applied when a services file is parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServicesInterpolation {
    #[default]
    Resolve,
    Inspect,
    Verbatim,
}

#[must_use]
pub fn services_field_kind(path: &[PathSegment]) -> FieldKind {
    let verbatim = path.iter().any(
        |segment| matches!(segment, PathSegment::Key(key) if VERBATIM_KEYS.contains(&key.as_str())),
    );
    if verbatim {
        FieldKind::VERBATIM
    } else {
        FieldKind::UNKNOWN
    }
}

pub fn interpolate_services_yaml(
    value: &mut Value,
    lookup: &impl Fn(&str) -> Option<String>,
    mode: ServicesInterpolation,
) -> Result<Vec<InterpolationError>, InterpolationError> {
    match mode {
        ServicesInterpolation::Resolve => {
            interpolate_yaml(value, lookup, &services_field_kind).map(|()| Vec::new())
        },
        ServicesInterpolation::Inspect => Ok(interpolate_yaml_keeping_unresolved(
            value,
            lookup,
            &services_field_kind,
        )),
        ServicesInterpolation::Verbatim => Ok(Vec::new()),
    }
}
