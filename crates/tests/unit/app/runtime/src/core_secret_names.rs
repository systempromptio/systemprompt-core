//! Every fixed custom-secret name a core crate reads is in the one list the
//! install refusal reads (`CORE_SECRET_NAMES` and `PINNED_SECRET_KEYS`), or
//! is a typed built-in field of the secrets document.
//!
//! The walk reads the production source of every core crate and finds each
//! secret read (`SecretStore::secret` / `secret_exact`, `SecretLookup::lookup`,
//! `Secrets::custom_secret`, and `Secrets::get` by a `*SECRET*` constant)
//! whose name is a string literal or a constant. A name read through a config
//! field is covered by the structural walk of the profile and the services
//! config instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_manifest::Secrets;
use systemprompt_traits::is_core_secret_name;

const LAYERS: &[&str] = &["shared", "infra", "domain", "app", "entry"];
const READS: &[&str] = &[".secret(", ".secret_exact(", ".lookup(", "custom_secret("];

fn crates_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != "tests" {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn production_sources() -> Vec<(PathBuf, String)> {
    let root = crates_root();
    let mut files = Vec::new();
    for layer in LAYERS {
        sources(&root.join(layer), &mut files);
    }
    sources(&root.join("../systemprompt/src"), &mut files);
    files
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(&path).ok().map(|text| (path, text)))
        .collect()
}

fn string_consts(files: &[(PathBuf, String)]) -> BTreeMap<String, String> {
    let mut consts = BTreeMap::new();
    for (_, text) in files {
        for line in text.lines() {
            let Some(rest) = line.trim().split("const ").nth(1) else {
                continue;
            };
            let Some((name, value)) = rest.split_once(": &str = \"") else {
                continue;
            };
            if let Some(value) = value.strip_suffix("\";") {
                consts.insert(name.trim().to_owned(), value.to_owned());
            }
        }
    }
    consts
}

fn argument(text: &str) -> &str {
    let mut depth = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return text[..index].trim(),
            ')' => depth -= 1,
            _ => {},
        }
    }
    text.trim()
}

fn screaming(ident: &str) -> bool {
    !ident.is_empty()
        && ident
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Debug)]
struct Read {
    file: PathBuf,
    name: String,
}

fn fixed_name_reads(files: &[(PathBuf, String)]) -> Vec<Read> {
    let consts = string_consts(files);
    let mut reads = Vec::new();
    for (file, text) in files {
        let calls = READS
            .iter()
            .map(|call| (*call, false))
            .chain(std::iter::once((".get(", true)));
        for (call, constant_only) in calls {
            for (offset, _) in text.match_indices(call) {
                let arg = argument(&text[offset + call.len()..]);
                let arg = arg.trim_start_matches('&');
                let name = if let Some(literal) = arg.strip_prefix('"') {
                    if constant_only {
                        continue;
                    }
                    literal.trim_end_matches('"').to_owned()
                } else {
                    let ident = arg.rsplit("::").next().unwrap_or(arg);
                    if !screaming(ident) || (constant_only && !ident.contains("SECRET")) {
                        continue;
                    }
                    consts.get(ident).cloned().unwrap_or_else(|| {
                        panic!("{}: {ident} is not a string constant", file.display())
                    })
                };
                reads.push(Read {
                    file: file.clone(),
                    name,
                });
            }
        }
    }
    reads
}

#[test]
fn every_fixed_secret_name_a_core_crate_reads_is_a_core_secret_name() {
    let files = production_sources();
    assert!(
        files.len() > 500,
        "the core sources were found ({})",
        files.len()
    );
    let reads = fixed_name_reads(&files);
    for expected in ["mcp_credential_broker_secret", "licence_signing_secret"] {
        assert!(
            reads.iter().any(|read| read.name == expected),
            "the walk sees the {expected} read: {reads:?}"
        );
    }
    let unlisted: Vec<String> = reads
        .iter()
        .filter(|read| !is_core_secret_name(&read.name) && !Secrets::is_built_in_name(&read.name))
        .map(|read| format!("{}: {}", read.file.display(), read.name))
        .collect();
    assert!(
        unlisted.is_empty(),
        "a core crate reads a fixed secret name absent from CORE_SECRET_NAMES: {unlisted:?}"
    );
}
