//! One plane's merge, write, audit and base update, on one transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_database::{record_config_plane_applied, with_change_context};
use systemprompt_provider_contracts::{
    ApplyOutcome, ChangeSource, ConfigPlane, ConfigPlaneError, DeclaredEntities, DeclaredState,
    PlaneSnapshot, PlaneTx, ResolvedEntities, ResolvedEntity, entity_hash,
};

use super::merge::{EntityMerge, Verdict, merge_plane};
use super::{APPLY_MODE, ApplyOptions, ConfigApplyService, PlaneMerge};
use crate::error::SchedulerResult;
use crate::repository::{
    BaseRecord, ConfigChangeRecord, ConflictRecord, LastApply, PlaneTransaction,
};

const ABSENT_SIDE_HASH: &str = "absent";

struct Sides<'a> {
    plane: &'a dyn ConfigPlane,
    options: &'a ApplyOptions<'a>,
    declared: &'a DeclaredEntities,
    declared_hash: &'a str,
    current: &'a PlaneSnapshot,
    after: &'a PlaneSnapshot,
}

impl ConfigApplyService {
    pub(super) async fn merge_one(
        &self,
        plane: &dyn ConfigPlane,
        options: &ApplyOptions<'_>,
    ) -> SchedulerResult<PlaneMerge> {
        let set = self.reconcile.planes();
        let mut result = PlaneMerge {
            plane: plane.name().to_owned(),
            ..PlaneMerge::default()
        };
        let Some(declared) = plane.sources(set.services_root())? else {
            result.undeclared = true;
            self.stamp_undeclared(plane, options).await?;
            return Ok(result);
        };
        let declared_hash = plane_hash(&declared);
        result.declared_hash = Some(declared_hash.clone());
        let repo = self.reconcile.repository();
        let ctx = self.reconcile.plane_context();
        let mut tx = repo.begin().await?;
        let current = plane.read(&ctx, &mut PlaneTx::new(&mut tx)).await?;
        let adopt = !repo.find_plane_applied(&mut tx, plane.name()).await?;
        let base = repo.list_base(&mut tx, plane.name()).await?;
        let merges = merge_plane(&declared, &current, &base, adopt);
        result.adopted = adopt;

        let groups = write_groups(plane, &declared, &merges);
        for (source, entities) in &groups {
            with_change_context(&mut tx, &options.actor, Some(options.reason), source).await?;
            let written = plane
                .write(&ctx, &mut PlaneTx::new(&mut tx), entities)
                .await?;
            result.outcome = add(result.outcome, written);
        }
        let after = if groups.is_empty() {
            current.clone()
        } else {
            plane.read(&ctx, &mut PlaneTx::new(&mut tx)).await?
        };
        let sides = Sides {
            plane,
            options,
            declared: &declared,
            declared_hash: &declared_hash,
            current: &current,
            after: &after,
        };
        self.settle(&mut tx, &merges, &sides, &mut result).await?;
        let drift_rows =
            i32::try_from(result.drift.len() + result.conflicts.len()).unwrap_or(i32::MAX);
        repo.set_last_apply(
            &mut tx,
            &LastApply {
                plane: plane.name(),
                authority: set.authority_of(plane).as_str(),
                declared_hash: &declared_hash,
                actor: options.actor.label(),
                mode: APPLY_MODE,
                drift_rows,
            },
        )
        .await?;
        record_config_plane_applied(&mut tx, plane.name()).await?;
        if options.dry_run {
            tx.rollback().await?;
        } else {
            tx.commit().await?;
        }
        Ok(result)
    }

    async fn stamp_undeclared(
        &self,
        plane: &dyn ConfigPlane,
        options: &ApplyOptions<'_>,
    ) -> SchedulerResult<()> {
        let mut tx = self.reconcile.repository().begin().await?;
        record_config_plane_applied(&mut tx, plane.name()).await?;
        if options.dry_run {
            tx.rollback().await?;
        } else {
            tx.commit().await?;
        }
        Ok(())
    }

    async fn settle(
        &self,
        tx: &mut PlaneTransaction,
        merges: &[EntityMerge],
        sides: &Sides<'_>,
        result: &mut PlaneMerge,
    ) -> SchedulerResult<()> {
        let repo = self.reconcile.repository();
        let plane = sides.plane;
        let name = plane.name();
        let actor = sides.options.actor.label();
        let fallback = ChangeSource::Code(plane.declared_path().to_owned()).to_string();
        for merge in merges {
            let entity = merge.entity.as_str();
            let file = sides.declared.get(entity);
            let source = file.map_or_else(|| fallback.clone(), |d| d.source.to_string());
            match merge.verdict {
                Verdict::Apply => {
                    let landed = sides.after.get(entity).map(|v| entity_hash(v));
                    if landed != merge.file_hash {
                        return Err(ConfigPlaneError::NotConverged {
                            plane: name.to_owned(),
                            remaining: 1,
                        }
                        .into());
                    }
                    let record = change_record(name, entity, sides);
                    repo.record_changes(tx, &[record]).await?;
                    result.applied.push(merge.entity.clone());
                },
                Verdict::Converged => {
                    repo.update_conflict_converged(tx, name, entity, actor)
                        .await?;
                    result.converged.push(merge.entity.clone());
                },
                Verdict::Drift => result.drift.push(merge.entity.clone()),
                Verdict::Conflict => {
                    repo.upsert_conflict(tx, &conflict_record(name, merge, sides))
                        .await?;
                    result.conflicts.push(merge.entity.clone());
                },
                Verdict::Unchanged => result.unchanged += 1,
            }
            let settles = matches!(merge.verdict, Verdict::Apply | Verdict::Converged);
            if !settles && !merge.adopted {
                continue;
            }
            match &merge.file_hash {
                Some(hash) => {
                    repo.upsert_base(
                        tx,
                        &BaseRecord {
                            plane: name,
                            entity,
                            declared_hash: hash,
                            source: &source,
                            applied_by: actor,
                        },
                    )
                    .await?;
                },
                None => repo.delete_base(tx, name, entity).await?,
            }
        }
        Ok(())
    }
}

fn write_groups(
    plane: &dyn ConfigPlane,
    declared: &DeclaredEntities,
    merges: &[EntityMerge],
) -> Vec<(ChangeSource, ResolvedEntities)> {
    let mut groups: BTreeMap<String, (ChangeSource, ResolvedEntities)> = BTreeMap::new();
    for merge in merges.iter().filter(|m| m.verdict == Verdict::Apply) {
        let file = declared.get(&merge.entity);
        let source = file.map_or_else(
            || ChangeSource::Code(plane.declared_path().to_owned()),
            |d| d.source.clone(),
        );
        groups
            .entry(source.to_string())
            .or_insert_with(|| (source.clone(), ResolvedEntities::new()))
            .1
            .insert(
                merge.entity.clone(),
                ResolvedEntity {
                    value: file.map(|d| d.value.clone()),
                    source,
                },
            );
    }
    groups.into_values().collect()
}

fn change_record<'a>(plane: &'a str, entity: &'a str, sides: &Sides<'a>) -> ConfigChangeRecord<'a> {
    let options = sides.options;
    let before_value = sides.current.get(entity).map(String::as_str);
    let after_value = sides.after.get(entity).map(String::as_str);
    let change_kind = match (before_value, after_value) {
        (None, _) => "missing",
        (Some(_), None) => "extra",
        (Some(_), Some(_)) => "changed",
    };
    ConfigChangeRecord {
        plane,
        entity,
        change_kind,
        before_value,
        after_value,
        mode: APPLY_MODE,
        actor: options.actor.label(),
        reason: options.reason,
        declared_hash: sides.declared_hash,
    }
}

fn conflict_record<'a>(
    plane: &'a str,
    merge: &'a EntityMerge,
    sides: &Sides<'a>,
) -> ConflictRecord<'a> {
    ConflictRecord {
        plane,
        entity: &merge.entity,
        base_hash: merge.base_hash.as_deref(),
        file_hash: merge.file_hash.as_deref().unwrap_or(ABSENT_SIDE_HASH),
        file_value: sides.declared.get(&merge.entity).map(|d| d.value.as_str()),
        db_hash: merge.db_hash.as_deref().unwrap_or(ABSENT_SIDE_HASH),
        db_value: sides.current.get(&merge.entity).map(String::as_str),
    }
}

fn plane_hash(declared: &DeclaredEntities) -> String {
    DeclaredState::from_entries(
        declared
            .iter()
            .map(|(k, d)| (k.clone(), d.value.clone()))
            .collect(),
    )
    .hash
}

const fn add(total: ApplyOutcome, more: ApplyOutcome) -> ApplyOutcome {
    ApplyOutcome {
        inserted: total.inserted + more.inserted,
        updated: total.updated + more.updated,
        deleted: total.deleted + more.deleted,
    }
}
