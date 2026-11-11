use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const METHODS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "head", "options", "any",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RouteKey {
    pub(super) site: String,
    pub(super) method: String,
    pub(super) path: String,
}

impl std::fmt::Display for RouteKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}", self.site, self.method, self.path)
    }
}

pub(super) fn api_source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../entry/api/src")
}

pub(super) fn router_inventory() -> BTreeSet<RouteKey> {
    let root = api_source_root();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    assert!(
        files.len() > 50,
        "api source tree found at {}: {} files",
        root.display(),
        files.len()
    );
    let mut keys = BTreeSet::new();
    for file in files {
        let rel = file
            .strip_prefix(&root)
            .expect("under root")
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file).expect("read api source");
        let code = strip_line_comments(&text);
        scan_file(&rel, &code, &mut keys);
    }
    keys
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read api source dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn strip_line_comments(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn scan_file(rel: &str, code: &str, keys: &mut BTreeSet<RouteKey>) {
    let mut from = 0;
    while let Some(found) = code[from..].find(".route(") {
        let open = from + found + ".route(".len();
        let close = matching_close(code, open);
        let inner = &code[open..close];
        let (path_arg, handler) = split_first_arg(inner);
        let path: String = path_arg.split_whitespace().collect();
        let site = format!("{rel}::{}", enclosing_fn(&code[..open]));
        let handler = resolve_binding(code, handler.trim());
        for method in top_level_methods(&handler) {
            keys.insert(RouteKey {
                site: site.clone(),
                method,
                path: path.clone(),
            });
        }
        from = close;
    }
}

fn matching_close(code: &str, open: usize) -> usize {
    let mut depth = 1usize;
    for (offset, c) in code[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            },
            _ => {},
        }
    }
    panic!("unbalanced .route( call");
}

fn split_first_arg(inner: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (offset, c) in inner.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return (&inner[..offset], &inner[offset + 1..]),
            _ => {},
        }
    }
    panic!(".route( call with one argument: {inner}");
}

fn enclosing_fn(before: &str) -> String {
    let at = before.rfind("fn ").expect(".route( inside a fn");
    before[at + 3..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn resolve_binding(code: &str, handler: &str) -> String {
    let is_ident = !handler.is_empty() && handler.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !is_ident {
        return handler.to_owned();
    }
    let needle = format!("let {handler}");
    let at = code
        .find(&needle)
        .unwrap_or_else(|| panic!("method router binding `{handler}` not found"));
    let rest = &code[at..];
    let eq = rest.find('=').expect("binding has an initialiser");
    let end = rest.find(';').expect("binding ends");
    rest[eq + 1..end].to_owned()
}

fn top_level_methods(handler: &str) -> Vec<String> {
    let bytes = handler.as_bytes();
    let mut depth = 0i32;
    let mut methods = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth == 0 && c.is_ascii_alphabetic() && word_starts(handler, i) => {
                let end = handler[i..]
                    .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .map_or(bytes.len(), |n| i + n);
                let word = &handler[i..end];
                if METHODS.contains(&word) && bytes.get(end) == Some(&b'(') {
                    methods.push(word.to_ascii_uppercase());
                }
                i = end;
                continue;
            },
            _ => {},
        }
        i += 1;
    }
    methods
}

fn word_starts(text: &str, i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = text.as_bytes()[i - 1];
    if prev.is_ascii_alphanumeric() || prev == b'_' {
        return false;
    }
    prev != b':' || text[..i].ends_with("routing::")
}
