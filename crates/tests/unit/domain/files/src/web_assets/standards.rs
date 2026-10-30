use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::{assets_root, own_scripts, shipped_files};

#[test]
fn the_client_defines_no_elements_and_ships_no_styles() {
    for (name, text) in own_scripts() {
        assert!(!text.contains("customElements.define"), "{name} defines an element");
        assert!(!text.contains("extends HTMLElement"), "{name} is a component");
    }
    let root = assets_root();
    assert!(!root.join("css").exists(), "the client ships no CSS");
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
    assert!(checked > 0, "only {checked} imports were checked");
}

fn asset(name: &str) -> String {
    std::fs::read_to_string(assets_root().join("js").join(name)).expect("asset")
}

#[test]
fn post_edits_takes_a_caller_stable_op_id() {
    let api = asset("documents-api.js");
    assert!(api.contains("op_id: editId"));
    assert!(!api.contains("op_id: opId()"));
}

#[test]
fn the_api_reaches_subjects_and_access_requests() {
    let api = asset("documents-api.js");
    assert!(api.contains("`${DOCUMENTS}/subjects${query({ kind, q, limit })}`"));
    assert!(api.contains("/access-requests/${encodeURIComponent(requestId)}/${decision}"));
}

#[test]
fn no_script_uses_native_dialogs_or_exceeds_the_line_limit() {
    for (name, text) in own_scripts() {
        for banned in ["alert(", "confirm(", "prompt("] {
            let native = text.match_indices(banned).any(|(at, _)| {
                at == 0
                    || !text[..at].ends_with(['.', '_'])
                        && !text[..at].ends_with(char::is_alphanumeric)
            });
            assert!(!native, "{name} calls {banned}");
        }
        assert!(text.lines().count() <= 150, "{name} is over 150 lines");
    }
}
