//! Reading `services/brain/` into entities with their declaring file, and
//! rendering a [`DeclaredBrain`] back into that file set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use systemprompt_extension::prelude::ConfigPlaneResult;
use systemprompt_manifest::services::brain::{
    BRAIN_DIR, FILTERS_FILE, IMPORTANCE_FILE, INTERESTS_DIR, KINDS_DIR, PROFILES_DIR,
    RETRIEVAL_FILE, TOPICS_FILE,
};
use systemprompt_models::brain::{TopicsDoc, validate};

use super::entities::DeclaredBrain;
use super::{invalid, unreadable};

pub(super) type Sourced = BTreeMap<String, (String, String)>;

const YAML_EXTENSION: &str = "yaml";

#[derive(Debug, Clone, Copy)]
enum Slot {
    Topics,
    Filters,
    Importance,
    Retrieval,
    Kind,
    Interest,
    Workflow,
}

const SINGLE_FILES: [(&str, Slot); 4] = [
    (TOPICS_FILE, Slot::Topics),
    (FILTERS_FILE, Slot::Filters),
    (IMPORTANCE_FILE, Slot::Importance),
    (RETRIEVAL_FILE, Slot::Retrieval),
];

const DIRECTORIES: [(&str, Slot); 3] = [
    (KINDS_DIR, Slot::Kind),
    (INTERESTS_DIR, Slot::Interest),
    (PROFILES_DIR, Slot::Workflow),
];

pub(super) fn declared(services_root: &Path) -> ConfigPlaneResult<Option<Sourced>> {
    if !services_root.join(BRAIN_DIR).is_dir() {
        return Ok(None);
    }
    let pieces = read_pieces(services_root)?;
    let mut merged = DeclaredBrain::default();
    for (_, piece) in &pieces {
        merged.merge(piece.clone());
    }
    validate(&merged.docs()).map_err(|e| invalid(BRAIN_DIR, &e))?;
    let mut entities = Sourced::new();
    for (file, piece) in pieces {
        for (key, value) in piece.entity_list().map_err(|e| invalid(&file, &e))? {
            if entities
                .insert(key.clone(), (value, file.clone()))
                .is_some()
            {
                return Err(invalid(&file, &format!("{key} is declared more than once")));
            }
        }
    }
    Ok(Some(entities))
}

fn read_pieces(services_root: &Path) -> ConfigPlaneResult<Vec<(String, DeclaredBrain)>> {
    let mut pieces = Vec::new();
    for (file, slot) in SINGLE_FILES {
        let relative = format!("{BRAIN_DIR}/{file}");
        if services_root.join(&relative).is_file() {
            pieces.push((
                relative.clone(),
                read_piece(services_root, &relative, slot)?,
            ));
        }
    }
    for (dir, slot) in DIRECTORIES {
        for relative in listed(services_root, dir)? {
            pieces.push((
                relative.clone(),
                read_piece(services_root, &relative, slot)?,
            ));
        }
    }
    Ok(pieces)
}

fn listed(services_root: &Path, dir: &str) -> ConfigPlaneResult<Vec<String>> {
    let relative_dir = format!("{BRAIN_DIR}/{dir}");
    let path = services_root.join(&relative_dir);
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| unreadable(&relative_dir, e))? {
        let entry_path = entry.map_err(|e| unreadable(&relative_dir, e))?.path();
        let is_yaml = entry_path
            .extension()
            .is_some_and(|ext| ext == YAML_EXTENSION);
        if let (true, true, Some(name)) = (
            is_yaml,
            entry_path.is_file(),
            entry_path.file_name().and_then(|n| n.to_str()),
        ) {
            names.push(format!("{relative_dir}/{name}"));
        }
    }
    names.sort();
    Ok(names)
}

fn read_piece(
    services_root: &Path,
    relative: &str,
    slot: Slot,
) -> ConfigPlaneResult<DeclaredBrain> {
    let text = std::fs::read_to_string(services_root.join(relative))
        .map_err(|e| unreadable(relative, e))?;
    let mut piece = DeclaredBrain::default();
    match slot {
        Slot::Topics => piece.topics = parse::<TopicsDoc>(relative, &text)?.topics,
        Slot::Filters => piece.filters = Some(parse(relative, &text)?),
        Slot::Importance => piece.importance = Some(parse(relative, &text)?),
        Slot::Retrieval => piece.retrieval = Some(parse(relative, &text)?),
        Slot::Kind => piece.kinds.push(parse(relative, &text)?),
        Slot::Interest => piece.interests.push(parse(relative, &text)?),
        Slot::Workflow => piece.workflows.push(parse(relative, &text)?),
    }
    Ok(piece)
}

fn parse<T: DeserializeOwned>(relative: &str, text: &str) -> ConfigPlaneResult<T> {
    serde_yaml::from_str(text).map_err(|e| unreadable(relative, e))
}

pub fn render_files(brain: &DeclaredBrain) -> ConfigPlaneResult<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    if !brain.topics.is_empty() {
        let doc = TopicsDoc {
            topics: brain.topics.clone(),
        };
        files.insert(TOPICS_FILE.to_owned(), yaml(TOPICS_FILE, &doc)?);
    }
    if let Some(doc) = &brain.filters {
        files.insert(FILTERS_FILE.to_owned(), yaml(FILTERS_FILE, doc)?);
    }
    if let Some(doc) = &brain.importance {
        files.insert(IMPORTANCE_FILE.to_owned(), yaml(IMPORTANCE_FILE, doc)?);
    }
    if let Some(doc) = &brain.retrieval {
        files.insert(RETRIEVAL_FILE.to_owned(), yaml(RETRIEVAL_FILE, doc)?);
    }
    for kind in &brain.kinds {
        insert_member(&mut files, KINDS_DIR, kind.kind.as_str(), kind)?;
    }
    for interest in &brain.interests {
        insert_member(&mut files, INTERESTS_DIR, interest.id.as_str(), interest)?;
    }
    for workflow in &brain.workflows {
        insert_member(&mut files, PROFILES_DIR, workflow.id.as_str(), workflow)?;
    }
    Ok(files)
}

fn insert_member<T: Serialize>(
    files: &mut BTreeMap<String, String>,
    dir: &str,
    id: &str,
    value: &T,
) -> ConfigPlaneResult<()> {
    let path = format!("{dir}/{id}.{YAML_EXTENSION}");
    let text = yaml(&path, value)?;
    files.insert(path, text);
    Ok(())
}

fn yaml<T: Serialize>(path: &str, value: &T) -> ConfigPlaneResult<String> {
    serde_yaml::to_string(value).map_err(|e| invalid(path, &e))
}
