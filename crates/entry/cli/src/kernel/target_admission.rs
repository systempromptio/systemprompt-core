//! Admission of the composition against a target profile the operator's host
//! cannot run.
//!
//! A cloud target's profile may hold variables that exist only on the target,
//! so it is never loaded whole here. [`read_disabled`] reads the layered
//! profile at a path as written and keeps only its `extensions` section, the
//! one input admission reads; only that section is interpolated, and a
//! variable in it that this host does not set is a typed error. The deferred
//! admission then runs against it and the open admitted views are returned.
//! Nothing else in the profile is interpolated, validated or returned.
//!
//! An error on this path names the path, the field and a variable's name,
//! never a resolved value: when a parse or admission refusal would print the
//! value of a variable the section interpolated, the refusal is replaced by
//! one that names the variable ([`TargetAdmissionError::ParseWithheld`],
//! [`TargetAdmissionError::AdmitWithheld`]). A literal id is reported as
//! written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde_yaml::{Mapping, Value};
use systemprompt_extension::cli::{AdmittedViews, KernelGrant};
use systemprompt_manifest::profile::layered::{base_path_for, merge_yaml, parse_raw_layer};
use systemprompt_manifest::profile::{ProfileError, profile_field_kind};
use systemprompt_manifest::{
    ExtensionsConfig, InterpolationError, interpolate_yaml, read_env_optional,
};
use systemprompt_traits::BoxedSource;

use crate::context::CommandContext;

const EXTENSIONS_KEY: &str = "extensions";

#[derive(Debug, thiserror::Error)]
pub enum TargetAdmissionError {
    #[error(
        "admitting a target profile requires the `{}` kernel grant",
        KernelGrant::TargetAdmission
    )]
    Refused,
    #[error(
        "the composition admission is not available to this command: it was admitted against the runner's profile or already used"
    )]
    Spent,
    #[error("reading the disabled members of {}", path.display())]
    Profile {
        path: PathBuf,
        #[source]
        source: ProfileError,
    },
    #[error("the disabled members of {} reference a variable this host does not set", path.display())]
    Interpolation {
        path: PathBuf,
        #[source]
        source: InterpolationError,
    },
    #[error("the disabled members of {} are not a list of extension ids", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error(
        "the disabled members of {} are not a list of extension ids: the field holds the value of {}",
        path.display(),
        Variables(variables)
    )]
    ParseWithheld {
        path: PathBuf,
        variables: Vec<String>,
    },
    #[error("the composition does not admit against the disabled members of {}", path.display())]
    Admit {
        path: PathBuf,
        #[source]
        source: BoxedSource,
    },
    #[error(
        "the composition does not admit against the disabled members of {}: it refuses the value of {}",
        path.display(),
        Variables(variables)
    )]
    AdmitWithheld {
        path: PathBuf,
        variables: Vec<String>,
    },
}

struct Variables<'a>(&'a [String]);

impl std::fmt::Display for Variables<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, name) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "${{{name}}}")?;
        }
        Ok(())
    }
}

struct Resolved {
    name: String,
    value: String,
}

fn chain_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push('\n');
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

fn printed(error: &dyn std::error::Error, resolved: &[Resolved]) -> Vec<String> {
    let text = chain_text(error);
    let mut names: Vec<String> = resolved
        .iter()
        .filter(|variable| text.contains(&variable.value))
        .map(|variable| variable.name.clone())
        .collect();
    names.dedup();
    names
}

pub fn read_disabled(profile: &Path) -> Result<ExtensionsConfig, TargetAdmissionError> {
    read_target(profile).map(|(extensions, _)| extensions)
}

fn read_target(
    profile: &Path,
) -> Result<(ExtensionsConfig, Vec<Resolved>), TargetAdmissionError> {
    let layer = |path: &Path| {
        std::fs::read_to_string(path)
            .map_err(|source| ProfileError::ReadFile {
                path: path.to_path_buf(),
                source,
            })
            .and_then(|content| parse_raw_layer(&content, path))
            .map_err(|source| TargetAdmissionError::Profile {
                path: profile.to_path_buf(),
                source,
            })
    };
    let overlay = layer(profile)?;
    let tree = match base_path_for(profile).filter(|base| base.is_file()) {
        Some(base) => merge_yaml(layer(&base)?, overlay),
        None => overlay,
    };
    let Some(section) = tree
        .get(EXTENSIONS_KEY)
        .filter(|value| !value.is_null())
        .cloned()
    else {
        return Ok((ExtensionsConfig::default(), Vec::new()));
    };
    let mut scoped = Value::Mapping(Mapping::from_iter([(
        Value::String(EXTENSIONS_KEY.to_owned()),
        section,
    )]));
    let resolved: RefCell<Vec<Resolved>> = RefCell::new(Vec::new());
    interpolate_yaml(
        &mut scoped,
        &|name| {
            let value = read_env_optional(name);
            if let Some(value) = &value {
                resolved.borrow_mut().push(Resolved {
                    name: name.to_owned(),
                    value: value.clone(),
                });
            }
            value
        },
        &profile_field_kind,
    )
    .map_err(|source| TargetAdmissionError::Interpolation {
        path: profile.to_path_buf(),
        source,
    })?;
    let resolved = resolved.into_inner();
    let section = scoped.get(EXTENSIONS_KEY).cloned().unwrap_or(Value::Null);
    match serde_yaml::from_value(section) {
        Ok(extensions) => Ok((extensions, resolved)),
        Err(source) => {
            let variables = printed(&source, &resolved);
            Err(if variables.is_empty() {
                TargetAdmissionError::Parse {
                    path: profile.to_path_buf(),
                    source,
                }
            } else {
                TargetAdmissionError::ParseWithheld {
                    path: profile.to_path_buf(),
                    variables,
                }
            })
        },
    }
}

pub(crate) fn admit_target(
    ctx: &CommandContext,
    profile: &Path,
) -> Result<AdmittedViews, TargetAdmissionError> {
    let (extensions, resolved) = read_target(profile)?;
    let admit = ctx.take_admit().ok_or(TargetAdmissionError::Spent)?;
    let admitted = admit(Some(&extensions)).map_err(|source| {
        let variables = printed(source.as_ref(), &resolved);
        if variables.is_empty() {
            TargetAdmissionError::Admit {
                path: profile.to_path_buf(),
                source,
            }
        } else {
            TargetAdmissionError::AdmitWithheld {
                path: profile.to_path_buf(),
                variables,
            }
        }
    })?;
    Ok(AdmittedViews::of(&admitted))
}
