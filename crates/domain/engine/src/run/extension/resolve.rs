//! Turning every linked extension into one ordered set: shadowing by id,
//! dependency checks, ordering, and the subset an instance's enablement admits.
//! Generic over what is kept beside the metadata, so the engine, instance and
//! shell halves resolve the same way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::engine::ExtensionId;

use super::enabled::EnabledExtensions;
use super::error::ResolveError;
use super::metadata::ExtensionMetadata;

/// One extension that survived resolution, with what it was resolved from.
#[derive(Debug, Clone)]
pub struct Resolved<T> {
    pub metadata: ExtensionMetadata,
    pub value: T,
}

struct Winner<T> {
    entry: Resolved<T>,
    tied: bool,
}

pub fn resolve<T>(
    candidates: impl IntoIterator<Item = Resolved<T>>,
) -> Result<Vec<Resolved<T>>, ResolveError> {
    let mut winners: BTreeMap<ExtensionId, Winner<T>> = BTreeMap::new();
    for candidate in candidates {
        let id = candidate.metadata.id.clone();
        let Some(current) = winners.get_mut(&id) else {
            winners.insert(
                id,
                Winner {
                    entry: candidate,
                    tied: false,
                },
            );
            continue;
        };
        match rank(&candidate.metadata).cmp(&rank(&current.entry.metadata)) {
            std::cmp::Ordering::Greater => {
                tracing::debug!(extension = %id, priority = current.entry.metadata.priority, "extension shadowed");
                *current = Winner {
                    entry: candidate,
                    tied: false,
                };
            },
            std::cmp::Ordering::Equal => current.tied = true,
            std::cmp::Ordering::Less => {
                tracing::debug!(extension = %id, priority = candidate.metadata.priority, "extension shadowed");
            },
        }
    }
    let mut kept = BTreeMap::new();
    for (id, winner) in winners {
        if winner.tied {
            return Err(ResolveError::Ambiguous {
                id,
                priority: winner.entry.metadata.priority,
            });
        }
        kept.insert(id, winner.entry);
    }
    check_dependencies(&kept)?;
    let mut ordered: Vec<Resolved<T>> = kept.into_values().collect();
    ordered.sort_by(|left, right| {
        right
            .metadata
            .priority
            .cmp(&left.metadata.priority)
            .then_with(|| left.metadata.id.cmp(&right.metadata.id))
    });
    Ok(ordered)
}

const fn rank(metadata: &ExtensionMetadata) -> (bool, u32) {
    (metadata.required, metadata.priority)
}

fn check_dependencies<T>(kept: &BTreeMap<ExtensionId, Resolved<T>>) -> Result<(), ResolveError> {
    for (id, entry) in kept {
        for dependency in &entry.metadata.dependencies {
            let Some(target) = kept.get(dependency) else {
                return Err(ResolveError::MissingDependency {
                    extension: id.clone(),
                    dependency: dependency.clone(),
                });
            };
            if entry.metadata.required && !target.metadata.required {
                return Err(ResolveError::RequiredOnOptional {
                    extension: id.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    let mut done = BTreeSet::new();
    for id in kept.keys() {
        let mut path = Vec::new();
        visit(id, kept, &mut path, &mut done)?;
    }
    Ok(())
}

fn visit<T>(
    id: &ExtensionId,
    kept: &BTreeMap<ExtensionId, Resolved<T>>,
    path: &mut Vec<ExtensionId>,
    done: &mut BTreeSet<ExtensionId>,
) -> Result<(), ResolveError> {
    if done.contains(id) {
        return Ok(());
    }
    if let Some(start) = path.iter().position(|step| step == id) {
        let mut chain = path.split_off(start);
        chain.push(id.clone());
        return Err(ResolveError::DependencyCycle { chain });
    }
    path.push(id.clone());
    if let Some(entry) = kept.get(id) {
        for dependency in &entry.metadata.dependencies {
            visit(dependency, kept, path, done)?;
        }
    }
    path.pop();
    done.insert(id.clone());
    Ok(())
}

pub fn admitted<'a, T>(
    resolved: &'a [Resolved<T>],
    enabled: Option<&EnabledExtensions>,
) -> Vec<&'a Resolved<T>> {
    let by_id: BTreeMap<&ExtensionId, &Resolved<T>> = resolved
        .iter()
        .map(|entry| (&entry.metadata.id, entry))
        .collect();
    let mut verdicts: BTreeMap<&ExtensionId, bool> = BTreeMap::new();
    resolved
        .iter()
        .filter(|entry| admits(&entry.metadata.id, &by_id, enabled, &mut verdicts))
        .collect()
}

fn admits<'a, T>(
    id: &'a ExtensionId,
    by_id: &BTreeMap<&'a ExtensionId, &'a Resolved<T>>,
    enabled: Option<&EnabledExtensions>,
    verdicts: &mut BTreeMap<&'a ExtensionId, bool>,
) -> bool {
    if let Some(verdict) = verdicts.get(id) {
        return *verdict;
    }
    verdicts.insert(id, false);
    let Some(&entry) = by_id.get(id) else {
        return false;
    };
    let switched_on =
        entry.metadata.required || enabled.is_some_and(|enabled| enabled.contains(id));
    let verdict = switched_on
        && entry
            .metadata
            .dependencies
            .iter()
            .all(|dependency| admits(dependency, by_id, enabled, verdicts));
    verdicts.insert(id, verdict);
    verdict
}
