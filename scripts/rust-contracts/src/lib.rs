//! Rust source contracts for explicit result handling and closed guards.
//!
//! `fail-open` also carries a best-effort heuristic for partial policy
//! projection: a guard fn that walks an inventory parameter (`known_*`,
//! `*_catalog`, `inventory`) and returns a plain value is emitting a map a
//! client can resolve to a permissive default when the inventory is empty or
//! stale — 0.51.0's tool catalog did exactly that. The fn must return
//! `Option`/`Result` so an empty or stale inventory withholds the subject.
//! The heuristic is a tripwire, not the gate; the review remains the gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        if self.mode == "discarded" && node.method == "unwrap_or_default" {
            let operation = match node.receiver.as_ref() {
                Expr::MethodCall(call) => Some(call.method.to_string()),
                Expr::Call(call) => match call.func.as_ref() {
                    Expr::Path(path) => path
                        .path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string()),
                    _ => None,
                },
                _ => None,
            };
            if operation.as_deref().is_some_and(|name| {
                [
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
                ]
                .contains(&name)
            }) {
                self.report(node.span(), "fallible-default");
            }
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
