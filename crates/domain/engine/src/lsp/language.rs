//! Protocol language identifiers by file extension, `plaintext` for anything
//! unknown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

pub fn language_id(path: &Path) -> &'static str {
    let Some(ext) = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
    else {
        return "plaintext";
    };
    match ext.as_str() {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "go" => "go",
        "py" | "pyi" => "python",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => "cpp",
        "m" => "objective-c",
        "mm" => "objective-cpp",
        "cs" | "csx" => "csharp",
        "lua" => "lua",
        "zig" | "zon" => "zig",
        "rb" | "rake" | "gemspec" | "ru" => "ruby",
        "sh" | "bash" => "shellscript",
        "yaml" | "yml" => "yaml",
        "php" => "php",
        "dart" => "dart",
        "swift" => "swift",
        "tf" => "terraform",
        "tfvars" => "terraform-vars",
        "ex" | "exs" => "elixir",
        "svelte" => "svelte",
        "vue" => "vue",
        "kt" | "kts" => "kotlin",
        "java" => "java",
        "json" => "json",
        "jsonc" => "jsonc",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "md" => "markdown",
        "toml" => "toml",
        "ps1" | "psm1" => "powershell",
        _ => "plaintext",
    }
}
