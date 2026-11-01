use std::collections::BTreeSet;

use systemprompt_identifiers::PluginId;
use systemprompt_manifest::services::{PluginAuthor, PluginConfig, ServicesConfig, SkillConfig};
use systemprompt_models::plugin::{ComponentSource, PluginComponentRef};

fn skill(id: &str) -> SkillConfig {
    serde_yaml::from_str(&format!("id: {id}\nname: {id}\ndescription: d\n")).expect("skill")
}

fn services(skills: &[&str]) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.skills.skills = skills
        .iter()
        .map(|id| ((*id).to_owned(), skill(id)))
        .collect();
    config
}

fn plugin(skills: PluginComponentRef) -> PluginConfig {
    PluginConfig {
        id: PluginId::new("selector"),
        name: "selector".to_owned(),
        description: "d".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: PluginAuthor {
            name: "Ed".to_owned(),
            email: "ed@example.com".to_owned(),
        },
        keywords: vec![],
        license: "MIT".to_owned(),
        category: "dev".to_owned(),
        skills,
        agents: PluginComponentRef::default(),
        rules: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: Default::default(),
        scripts: vec![],
        dependencies: vec![],
    }
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn an_instance_source_selects_every_skill_but_the_excluded() {
    let config = services(&["alpha", "beta", "gamma"]);
    let selected = config.plugin_selected_skill_ids(&plugin(PluginComponentRef {
        source: ComponentSource::Instance,
        exclude: vec!["beta".to_owned()],
        ..PluginComponentRef::default()
    }));
    assert_eq!(selected, set(&["alpha", "gamma"]));
}

#[test]
fn an_instance_source_without_exclusions_selects_all() {
    let config = services(&["alpha", "beta"]);
    let selected = config.plugin_selected_skill_ids(&plugin(PluginComponentRef {
        source: ComponentSource::Instance,
        ..PluginComponentRef::default()
    }));
    assert_eq!(selected, set(&["alpha", "beta"]));
}

#[test]
fn an_explicit_source_selects_only_the_included() {
    let config = services(&["alpha", "beta"]);
    let selected = config.plugin_selected_skill_ids(&plugin(PluginComponentRef {
        include: vec!["beta".to_owned(), "undeclared".to_owned()],
        ..PluginComponentRef::default()
    }));
    assert_eq!(selected, set(&["beta", "undeclared"]));
}
