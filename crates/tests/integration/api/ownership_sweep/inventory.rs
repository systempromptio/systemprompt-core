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

fn entry_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../entry")
}

pub(super) type Source = (String, String);

pub(super) fn sources() -> Vec<Source> {
    let root = entry_root().join("api/src");
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    assert!(
        files.len() > 50,
        "api source tree found at {}: {} files",
        root.display(),
        files.len()
    );
    let mut sources = Vec::new();
    read_tree(&root, "", &files, &mut sources);
    for capability in capability_entry_crates() {
        let src = entry_root().join(&capability).join("src");
        let mut files = Vec::new();
        collect_rs(&src, &mut files);
        read_tree(&src, &format!("{capability}/"), &files, &mut sources);
    }
    sources
}

pub(super) fn router_inventory() -> BTreeSet<RouteKey> {
    let sources = sources();
    let mut keys = BTreeSet::new();
    for (rel, code) in &sources {
        scan_file(rel, code, &sources, &mut keys);
    }
    keys
}

pub(super) fn fn_body<'a>(sources: &'a [Source], name: &str) -> Option<(&'a str, &'a str)> {
    let needle = format!("fn {name}(");
    sources.iter().find_map(|(rel, code)| {
        let at = code.find(&needle)?;
        let open = at + code[at..].find('{')?;
        let close = matching_brace(code, open + 1);
        Some((rel.as_str(), &code[at..=close]))
    })
}

pub(super) fn each_call(code: &str, call: &str, mut found: impl FnMut(usize, &str)) {
    let mut from = 0;
    while let Some(at) = code[from..].find(call) {
        let open = from + at + call.len();
        let close = matching_close(code, open);
        found(open, &code[open..close]);
        from = close;
    }
}

fn matching_brace(code: &str, open: usize) -> usize {
    let mut depth = 1usize;
    for (offset, c) in code[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            },
            _ => {},
        }
    }
    panic!("unbalanced fn body");
}

fn capability_entry_crates() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(entry_root())
        .expect("read entry dir")
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            (path.join("src").is_dir() && name != "api" && name != "cli").then_some(name)
        })
        .collect();
    names.sort();
    names
}

fn read_tree(root: &Path, prefix: &str, files: &[PathBuf], sources: &mut Vec<Source>) {
    for file in files {
        let rel = file
            .strip_prefix(root)
            .expect("under root")
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(file).expect("read entry source");
        sources.push((format!("{prefix}{rel}"), strip_line_comments(&text)));
    }
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

fn scan_file(rel: &str, code: &str, sources: &[Source], keys: &mut BTreeSet<RouteKey>) {
    each_call(code, ".route(", |open, inner| {
        let site = format!("{rel}::{}", enclosing_fn(&code[..open]));
        let (first, rest) = split_first_arg(inner);
        let first: String = first.split_whitespace().collect();
        if let Some((_, method)) = first.split_once("Method::") {
            let (path_arg, _) = split_first_arg(rest);
            keys.insert(RouteKey {
                site,
                method: method.to_owned(),
                path: path_arg.split_whitespace().collect(),
            });
            return;
        }
        let (path, handler) = (first, rest);
        let handler = resolve_binding(code, handler.trim());
        let mut methods = top_level_methods(&handler);
        if methods.is_empty() {
            methods = returned_methods(sources, &handler);
        }
        assert!(
            !methods.is_empty(),
            "{site}: the method router `{}` of route `{path}` names no HTTP method the \
             inventory can read",
            handler.trim()
        );
        for method in methods {
            keys.insert(RouteKey {
                site: site.clone(),
                method,
                path: path.clone(),
            });
        }
    });
}

fn returned_methods(sources: &[Source], handler: &str) -> Vec<String> {
    let Some(call) = handler.trim().split('(').next() else {
        return Vec::new();
    };
    let callee = call.rsplit("::").next().unwrap_or(call);
    match fn_body(sources, callee) {
        Some((_, body)) if body[..body.find('{').unwrap_or(0)].contains("MethodRouter") => {
            let inner = &body[body.find('{').map_or(0, |b| b + 1)..body.len() - 1];
            top_level_methods(inner)
        },
        _ => Vec::new(),
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

pub(super) fn split_first_arg(inner: &str) -> (&str, &str) {
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

pub(super) fn enclosing_fn(before: &str) -> String {
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
