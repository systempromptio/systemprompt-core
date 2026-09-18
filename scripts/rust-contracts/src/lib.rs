//! Rust source contracts for explicit result handling and closed guards.
//!
//! `discarded` reports a fallible value thrown away: `let _ =`, `_ =`, a
//! trailing `.ok();`, `drop(<call>)` on a call that is not an ownership
//! transfer (`into_inner`, `take`, `replace`, `new`, …), and
//! `unwrap_or_default()` on a receiver that is a `Result` — an awaited
//! expression, a call into `serde_json`/`std::fs`/`std::env`, or one of the
//! named fallible operations (`text`, `parse`, `try_from`, `lock`, …). A
//! `// Why: discard-ok: <reason>` line above the statement carves it out.
//!
//! `fail-open` also carries a best-effort heuristic for partial policy
//! projection: a guard fn that walks an inventory parameter (`known_*`,
//! `*_catalog`, `inventory`) and returns a plain value is emitting a map a
//! client can resolve to a permissive default when the inventory is empty or
//! stale — 0.51.0's tool catalog did exactly that. The fn must return
//! `Option`/`Result` so an empty or stale inventory withholds the subject.
//! The heuristic is a tripwire, not the gate; the review remains the gate.
//!
//! `tracing-messages` reports a `trace!`/`debug!`/`info!`/`warn!`/`error!`
//! whose message literal interpolates (`"failed to run {cmd}"`): values are
//! structured fields, the message is a constant. A bare `"{}"` or `"{name}"`
//! message — a prepared freeform string — is the one exempt form.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use proc_macro2::{Delimiter, TokenTree};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Pat, Stmt};

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub rule: &'static str,
}

pub fn inspect(source: &str, mode: &str) -> Result<Vec<Finding>, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut scanner = Scanner {
        annotations: annotation_lines(source),
        mode,
        guard: false,
        inventory_params: Vec::new(),
        findings: Vec::new(),
    };
    scanner.visit_file(&syntax);
    Ok(scanner.findings)
}

struct Scanner<'a> {
    annotations: Vec<usize>,
    mode: &'a str,
    guard: bool,
    inventory_params: Vec<String>,
    findings: Vec<Finding>,
}

impl Scanner<'_> {
    fn report(&mut self, span: proc_macro2::Span, rule: &'static str) {
        let start = span.start().line;
        let end = span.end().line;
        let allowed = self
            .annotations
            .iter()
            .any(|line| *line >= start.saturating_sub(1) && *line <= end);
        if !allowed
            && !self
                .findings
                .iter()
                .any(|f| f.line == start && f.rule == rule)
        {
            self.findings.push(Finding { line: start, rule });
        }
    }
}

fn guard_name(name: &str) -> bool {
    ["is_", "has_", "check_", "verify_", "supported", "allowed"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || name.contains("is_supported")
        || name.contains("is_allowed")
}

fn inventory_param(name: &str) -> bool {
    name.starts_with("known_") || name.ends_with("_catalog") || name == "inventory"
}

fn returns_fallible(output: &syn::ReturnType) -> bool {
    match output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            syn::Type::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Option" || segment.ident == "Result"),
            _ => false,
        },
    }
}

fn inventory_params(sig: &syn::Signature) -> Vec<String> {
    if returns_fallible(&sig.output) {
        return Vec::new();
    }
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(typed) => match typed.pat.as_ref() {
                Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .filter(|name| inventory_param(name))
        .collect()
}

fn path_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        Expr::Reference(reference) => path_ident(&reference.expr),
        Expr::Field(field) => path_ident(&field.base),
        _ => None,
    }
}

const OWNERSHIP_TRANSFERS: &[&str] = &[
    "into_inner",
    "take",
    "replace",
    "swap",
    "clone",
    "new",
    "from",
    "into",
    "default",
];

fn transfers_ownership(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(call) => OWNERSHIP_TRANSFERS.contains(&call.method.to_string().as_str()),
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Path(path) => path.path.segments.last().is_some_and(|segment| {
                OWNERSHIP_TRANSFERS.contains(&segment.ident.to_string().as_str())
            }),
            _ => false,
        },
        _ => false,
    }
}

fn dropped_call(call: &syn::ExprCall) -> bool {
    let Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    if !path.path.is_ident("drop") || call.args.len() != 1 {
        return false;
    }
    match call.args.first() {
        Some(argument @ (Expr::Call(_) | Expr::MethodCall(_))) => !transfers_ownership(argument),
        Some(Expr::Await(_) | Expr::Try(_)) => true,
        _ => false,
    }
}

const FALLIBLE_OPERATIONS: &[&str] = &[
    "write",
    "write_all",
    "status",
    "output",
    "send",
    "persist",
    "save",
    "remove_file",
    "remove_dir_all",
    "create_dir_all",
    "set_permissions",
    "text",
    "bytes",
    "json",
    "read_to_string",
    "read_dir",
    "to_value",
    "from_value",
    "from_str",
    "from_slice",
    "to_string_pretty",
    "to_vec",
    "parse",
    "try_from",
    "try_into",
    "current_exe",
    "current_dir",
    "canonicalize",
    "metadata",
    "var",
    "try_with",
    "lock",
    "join",
    "recv",
];

const FALLIBLE_PATH_PREFIXES: &[&[&str]] = &[
    &["serde_json"],
    &["std", "fs"],
    &["std", "env"],
    &["fs"],
    &["env"],
];

fn fallible_path(path: &syn::Path) -> bool {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let named = segments
        .last()
        .is_some_and(|name| FALLIBLE_OPERATIONS.contains(&name.as_str()));
    let prefixed = FALLIBLE_PATH_PREFIXES.iter().any(|prefix| {
        segments.len() > prefix.len() && prefix.iter().zip(&segments).all(|(want, got)| want == got)
    });
    named || prefixed
}

fn fallible_receiver(receiver: &Expr) -> bool {
    match receiver {
        Expr::Await(_) => true,
        Expr::MethodCall(call) => FALLIBLE_OPERATIONS.contains(&call.method.to_string().as_str()),
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Path(path) => fallible_path(&path.path),
            _ => false,
        },
        Expr::Paren(paren) => fallible_receiver(&paren.expr),
        _ => false,
    }
}

const TRACING_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

// Why: a network or database result whose error is discarded on the way to a
// constant message is exactly how a provider's `403 unauthorized_client`
// spent a day looking like an outage; the closure must say what it saw.
const BOUNDARY_METHODS: [&str; 11] = [
    "send",
    "json",
    "text",
    "bytes",
    "chunk",
    "error_for_status",
    "execute",
    "fetch_one",
    "fetch_all",
    "fetch_optional",
    "fetch",
];

fn boundary_receiver(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(call) => {
            BOUNDARY_METHODS.contains(&call.method.to_string().as_str())
                || boundary_receiver(&call.receiver)
        },
        Expr::Await(inner) => boundary_receiver(&inner.base),
        Expr::Try(inner) => boundary_receiver(&inner.expr),
        Expr::Paren(inner) => boundary_receiver(&inner.expr),
        _ => false,
    }
}

fn silent_closure(expr: &Expr) -> bool {
    let Expr::Closure(closure) = expr else {
        return false;
    };
    let unnamed = closure.inputs.iter().any(|input| match input {
        Pat::Wild(_) => true,
        Pat::Ident(ident) => ident.ident.to_string().starts_with('_'),
        Pat::Type(typed) => matches!(typed.pat.as_ref(), Pat::Wild(_))
            || matches!(typed.pat.as_ref(), Pat::Ident(ident) if ident.ident.to_string().starts_with('_')),
        _ => false,
    });
    unnamed && !mentions_tracing(&closure.body)
}

fn mentions_tracing(expr: &Expr) -> bool {
    struct Finder(bool);
    impl<'ast> Visit<'ast> for Finder {
        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            if tracing_macro(&node.path) {
                self.0 = true;
            }
            visit::visit_macro(self, node);
        }
    }
    let mut finder = Finder(false);
    finder.visit_expr(expr);
    finder.0
}

fn tracing_macro(path: &syn::Path) -> bool {
    let level = path
        .segments
        .last()
        .is_some_and(|segment| TRACING_LEVELS.contains(&segment.ident.to_string().as_str()));
    let qualifier = path.segments.len() == 1
        || path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == "tracing" || segment.ident == "log");
    level && qualifier
}

fn message_literal(tokens: proc_macro2::TokenStream) -> Option<proc_macro2::Literal> {
    let mut previous: Option<char> = None;
    let mut inside_call = false;
    for tree in tokens {
        match tree {
            TokenTree::Literal(literal) => {
                let text = literal.to_string();
                if !text.starts_with('"') {
                    previous = None;
                    continue;
                }
                let field_value = matches!(previous, Some('=' | '%' | '?' | ':'));
                if field_value || inside_call {
                    previous = None;
                    continue;
                }
                return Some(literal);
            },
            TokenTree::Punct(punct) => {
                previous = Some(punct.as_char());
                if punct.as_char() == ',' {
                    inside_call = false;
                }
            },
            TokenTree::Group(group) => {
                inside_call = group.delimiter() == Delimiter::Parenthesis;
                previous = None;
            },
            TokenTree::Ident(_) => previous = None,
        }
    }
    None
}

fn interpolates(literal: &str) -> bool {
    let Some(inner) = literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return false;
    };
    if inner == "{}" {
        return false;
    }
    let bare_name = inner
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        });
    if bare_name {
        return false;
    }
    inner.replace("{{", "").replace("}}", "").contains('{')
}

fn true_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Bool(value) if value.value))
}

impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous = self.guard;
        let previous_params = std::mem::take(&mut self.inventory_params);
        self.guard = guard_name(&node.sig.ident.to_string());
        if self.guard {
            self.inventory_params = inventory_params(&node.sig);
        }
        visit::visit_item_fn(self, node);
        self.guard = previous;
        self.inventory_params = previous_params;
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        let previous = self.guard;
        let previous_params = std::mem::take(&mut self.inventory_params);
        self.guard = guard_name(&node.sig.ident.to_string());
        if self.guard {
            self.inventory_params = inventory_params(&node.sig);
        }
        visit::visit_trait_item_fn(self, node);
        self.guard = previous;
        self.inventory_params = previous_params;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let previous = self.guard;
        let previous_params = std::mem::take(&mut self.inventory_params);
        self.guard = guard_name(&node.sig.ident.to_string());
        if self.guard {
            self.inventory_params = inventory_params(&node.sig);
        }
        visit::visit_impl_item_fn(self, node);
        self.guard = previous;
        self.inventory_params = previous_params;
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        if self.mode == "discarded" {
            let discarded = match node {
                Stmt::Local(local) => local.init.as_ref().is_some_and(|init| matches!(local.pat, Pat::Wild(_)) || matches!(init.expr.as_ref(), Expr::MethodCall(call) if call.method == "ok" && call.args.is_empty())),
                Stmt::Expr(Expr::Assign(assign), _) => {
                    matches!(assign.left.as_ref(), Expr::Infer(_))
                },
                Stmt::Expr(Expr::MethodCall(call), Some(_)) => {
                    call.method == "ok" && call.args.is_empty()
                },
                Stmt::Expr(Expr::Call(call), Some(_)) => dropped_call(call),
                _ => false,
            };
            if discarded {
                self.report(node.span(), "discarded-result");
            }
        }
        visit::visit_stmt(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.mode == "fail-open"
            && self.guard
            && (node.method == "is_none_or"
                || (node.method == "map_or" || node.method == "unwrap_or")
                    && node.args.first().is_some_and(true_literal))
        {
            self.report(node.span(), "fail-open-guard");
        }
        if self.mode == "fail-open"
            && self.guard
            && (node.method == "iter" || node.method == "into_iter" || node.method == "iter_mut")
            && path_ident(&node.receiver).is_some_and(|name| self.inventory_params.contains(&name))
        {
            self.report(node.span(), "partial-projection");
        }
        if self.mode == "discarded"
            && node.method == "unwrap_or_default"
            && fallible_receiver(&node.receiver)
        {
            self.report(node.span(), "fallible-default");
        }
        if self.mode == "swallowed-errors"
            && node.method == "map_err"
            && boundary_receiver(&node.receiver)
            && node.args.first().is_some_and(silent_closure)
        {
            self.report(node.span(), "swallowed-boundary-error");
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        if self.mode == "fail-open"
            && self.guard
            && path_ident(&node.expr).is_some_and(|name| self.inventory_params.contains(&name))
        {
            self.report(node.span(), "partial-projection");
        }
        visit::visit_expr_for_loop(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.mode == "tracing-messages" && tracing_macro(&node.path) {
            if let Some(literal) = message_literal(node.tokens.clone()) {
                if interpolates(&literal.to_string()) {
                    self.report(literal.span(), "tracing-message-interpolation");
                }
            }
        }
        visit::visit_macro(self, node);
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        if self.mode == "fail-open"
            && self.guard
            && matches!(node.pat, Pat::Wild(_))
            && true_literal(&node.body)
        {
            self.report(node.span(), "fail-open-guard");
        }
        visit::visit_arm(self, node);
    }
}

fn annotation_lines(source: &str) -> Vec<usize> {
    let mut offset = 0;
    let mut line = 1;
    let mut annotations = Vec::new();
    for token in rustc_lexer::tokenize(source) {
        let text = &source[offset..offset + token.len];
        if token.kind == rustc_lexer::TokenKind::LineComment
            && text
                .strip_prefix("// Why: discard-ok:")
                .is_some_and(|reason| !reason.trim().is_empty())
        {
            annotations.push(line);
        }
        line += text.bytes().filter(|byte| *byte == b'\n').count();
        offset += token.len;
    }
    annotations
}
