//! Load-time checks over a parsed `rules.yaml`. Every problem in the file is
//! collected and reported together as [`RulesProblems`], so one run names all
//! of them.
//!
//! An entity carries a non-blank `why`, a well-formed `<kind>/<id>` declared
//! once, a glob only on a kind that takes one, an `owner` only as
//! `bundle:<name>` on a literal id, at least one band, band keys that are
//! `role` or a well-formed extension slug (never `user`), no blank subject,
//! and no subject both allowed and denied in the same band.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;

use super::types::{BandSpec, EntityDecl, EntityTarget, GLOB_KINDS, RulesDoc};
use crate::authz::error::{AuthzError, AuthzResult};
use crate::authz::types::RuleType;

#[derive(Debug, Error)]
pub enum RulesProblem {
    #[error("{0}: declared more than once")]
    Duplicate(String),
    #[error("{entity}: {source}")]
    Target {
        entity: String,
        #[source]
        source: Box<AuthzError>,
    },
    #[error("{0}: `why` is required")]
    MissingWhy(EntityTarget),
    #[error("{0}: only gateway_route, hook and model take a glob; write the id")]
    GlobKind(EntityTarget),
    #[error("{target}: owner '{owner}' must be written as bundle:<name>")]
    Owner { target: EntityTarget, owner: String },
    #[error("{0}: a glob cannot name an owner")]
    GlobOwner(EntityTarget),
    #[error("{0}: declares no allow and no deny")]
    NoBands(EntityTarget),
    #[error(
        "{target}: '{band}' is not a band (role, or a lowercase extension slug; per-user rules \
         are runtime state)"
    )]
    Band { target: EntityTarget, band: String },
    #[error("{target}: {band} names no subjects")]
    NoSubjects { target: EntityTarget, band: String },
    #[error("{target}: {band} contains a blank or padded subject")]
    BlankSubject { target: EntityTarget, band: String },
    #[error("{target}: {band} has a blank `why`")]
    BlankBandWhy { target: EntityTarget, band: String },
    #[error("{target}: {band} '{subject}' is both allowed and denied")]
    Conflict {
        target: EntityTarget,
        band: String,
        subject: String,
    },
}

#[derive(Debug, Error)]
pub struct RulesProblems(pub Vec<RulesProblem>);

impl fmt::Display for RulesProblems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, problem) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{problem}")?;
        }
        Ok(())
    }
}

impl RulesDoc {
    pub fn validate(&self) -> AuthzResult<()> {
        let mut problems = Vec::new();
        let mut seen = BTreeSet::new();
        for decl in &self.entities {
            if !seen.insert(decl.entity.as_str()) {
                problems.push(RulesProblem::Duplicate(decl.entity.clone()));
            }
            if let Err(problem) = decl.check() {
                problems.push(problem);
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(AuthzError::Rules(RulesProblems(problems)))
        }
    }
}

impl EntityDecl {
    fn check(&self) -> Result<(), RulesProblem> {
        let target: EntityTarget = self.entity.parse().map_err(|source| RulesProblem::Target {
            entity: self.entity.clone(),
            source: Box::new(source),
        })?;
        if self.why.trim().is_empty() {
            return Err(RulesProblem::MissingWhy(target));
        }
        if target.is_glob() && !GLOB_KINDS.contains(&target.kind) {
            return Err(RulesProblem::GlobKind(target));
        }
        self.check_owner(&target)?;
        if self.allow.is_empty() && self.deny.is_empty() {
            return Err(RulesProblem::NoBands(target));
        }
        check_bands(&target, &self.allow)?;
        check_bands(&target, &self.deny)?;
        for (band, allowed) in &self.allow {
            let Some(denied) = self.deny.get(band) else {
                continue;
            };
            if let Some(subject) = allowed
                .values()
                .iter()
                .find(|v| denied.values().contains(v))
            {
                return Err(RulesProblem::Conflict {
                    target,
                    band: band.clone(),
                    subject: subject.clone(),
                });
            }
        }
        Ok(())
    }

    fn check_owner(&self, target: &EntityTarget) -> Result<(), RulesProblem> {
        let Some(owner) = self.owner.as_deref() else {
            return Ok(());
        };
        let named = self
            .owner_bundle()
            .map(str::trim)
            .filter(|name| !name.is_empty());
        if named.is_none() {
            return Err(RulesProblem::Owner {
                target: target.clone(),
                owner: owner.to_owned(),
            });
        }
        if target.is_glob() {
            return Err(RulesProblem::GlobOwner(target.clone()));
        }
        Ok(())
    }
}

fn check_bands(
    target: &EntityTarget,
    bands: &BTreeMap<String, BandSpec>,
) -> Result<(), RulesProblem> {
    for (band, spec) in bands {
        let problem =
            |make: fn(EntityTarget, String) -> RulesProblem| make(target.clone(), band.clone());
        if band != RuleType::ROLE.as_str() && RuleType::extension(band.clone()).is_err() {
            return Err(problem(|target, band| RulesProblem::Band { target, band }));
        }
        if spec.values().is_empty() {
            return Err(problem(|target, band| RulesProblem::NoSubjects {
                target,
                band,
            }));
        }
        if spec
            .values()
            .iter()
            .any(|v| v.trim().is_empty() || v.trim() != v)
        {
            return Err(problem(|target, band| RulesProblem::BlankSubject {
                target,
                band,
            }));
        }
        if spec.why().is_some_and(|why| why.trim().is_empty()) {
            return Err(problem(|target, band| RulesProblem::BlankBandWhy {
                target,
                band,
            }));
        }
    }
    Ok(())
}
