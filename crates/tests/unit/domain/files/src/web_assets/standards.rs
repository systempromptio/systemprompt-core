use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::{assets_root, own_scripts, shipped_files};

fn defined_tags(text: &str) -> Vec<String> {
    text.match_indices("customElements.define(\"")
        .filter_map(|(at, needle)| {
            let rest = &text[at + needle.len()..];
            rest.split('"').next().map(str::to_owned)
        })
        .collect()
}

#[test]
fn every_component_file_defines_exactly_its_own_sp_element() {
    for (name, text) in own_scripts() {
        let tags = defined_tags(&text);
        match name.strip_suffix(".js").filter(|n| n.starts_with("sp-")) {
            Some(tag) => assert_eq!(tags, vec![tag.to_owned()], "{name}"),
            None => assert!(tags.is_empty(), "{name} is not a component file"),
        }
    }
}

#[test]
fn no_inner_html_assignment_with_interpolation() {
    for (name, text) in own_scripts() {
        for line in text.lines().filter(|l| l.contains("innerHTML")) {
            let interpolated = line.contains("+=")
                || line.contains("${")
                || line.contains('`')
                || line.contains(" + ");
            assert!(!interpolated, "{name}: {line}");
        }
    }
}

#[test]
fn no_console_calls() {
    for (name, text) in own_scripts() {
        assert!(!text.contains("console."), "{name}");
    }
}

#[test]
fn only_the_api_module_calls_fetch() {
    for (name, text) in own_scripts() {
        if name != "documents-api.js" {
            assert!(!text.contains("fetch("), "{name}");
        }
    }
}

fn specifiers(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_start)
        .filter(|l| {
            l.starts_with("import ") || l.starts_with("export ") || l.starts_with("} from ")
        })
        .filter_map(|l| {
            let quote = if l.contains('"') { '"' } else { '\'' };
            let tail = l
                .rsplit_once(" from ")
                .map_or_else(|| l.strip_prefix("import ")?.into(), |(_, t)| Some(t))?;
            let spec = tail.trim().trim_end_matches(';').trim_matches(quote);
            spec.starts_with('.').then(|| spec.to_owned())
        })
        .collect()
}

fn resolve(base: &Path, spec: &str) -> PathBuf {
    let mut out = base.parent().expect("parent").to_path_buf();
    for part in Path::new(spec).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            },
            Component::Normal(p) => out.push(p),
            _ => {},
        }
    }
    out
}

#[test]
fn every_relative_import_resolves_to_a_shipped_file() {
    let shipped: BTreeSet<PathBuf> = shipped_files();
    let scripts = shipped
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "js"));
    let mut checked = 0;
    for script in scripts {
        let text = std::fs::read_to_string(script).expect("read script");
        for spec in specifiers(&text) {
            let target = resolve(script, &spec);
            assert!(
                shipped.contains(&target),
                "{} imports {spec}",
                script.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} imports were checked");
}

#[test]
fn wasm_glue_loads_the_shipped_module() {
    let collab =
        std::fs::read_to_string(assets_root().join("js/documents-collab.js")).expect("collab");
    assert!(collab.contains("\"../wasm/loro.wasm\""));
    assert!(collab.contains("\"../wasm/loro.js\""));
}
