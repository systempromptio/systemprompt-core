//! The formatters tried when nothing is configured, in priority order per
//! extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::FormatterSpec;

const WEB: &[&str] = &[
    ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".json", ".jsonc", ".css",
    ".scss", ".less", ".html", ".htm", ".vue", ".svelte", ".md", ".mdx", ".yaml", ".yml",
    ".graphql", ".gql",
];
const BIOME: &[&str] = &[
    ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".json", ".jsonc", ".css",
];
const C_FAMILY: &[&str] = &[
    ".c", ".h", ".cc", ".cpp", ".cxx", ".c++", ".hh", ".hpp", ".hxx", ".h++", ".ino", ".m", ".mm",
];
const PRETTIER_CONFIGS: &[&str] = &[
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.json5",
    ".prettierrc.yaml",
    ".prettierrc.yml",
    ".prettierrc.toml",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.mjs",
    ".prettierrc.ts",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.mjs",
    "prettier.config.ts",
];

struct Builtin {
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    extensions: &'static [&'static str],
    root_markers: &'static [&'static str],
    node_modules: bool,
}

const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "biome",
        command: "biome",
        args: &["format", "--write", "$FILE"],
        extensions: BIOME,
        root_markers: &["biome.json", "biome.jsonc"],
        node_modules: true,
    },
    Builtin {
        name: "prettier",
        command: "prettier",
        args: &["--write", "$FILE"],
        extensions: WEB,
        root_markers: PRETTIER_CONFIGS,
        node_modules: true,
    },
    Builtin {
        name: "rustfmt",
        command: "rustfmt",
        args: &["--edition", "2021", "$FILE"],
        extensions: &[".rs"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "gofmt",
        command: "gofmt",
        args: &["-w", "$FILE"],
        extensions: &[".go"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "ruff",
        command: "ruff",
        args: &["format", "$FILE"],
        extensions: &[".py", ".pyi"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "black",
        command: "black",
        args: &["--quiet", "$FILE"],
        extensions: &[".py", ".pyi"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "clang-format",
        command: "clang-format",
        args: &["-i", "$FILE"],
        extensions: C_FAMILY,
        root_markers: &[".clang-format", "_clang-format"],
        node_modules: false,
    },
    Builtin {
        name: "dotnet-format",
        command: "dotnet",
        args: &["format", "whitespace", "--folder", "--include", "$FILE"],
        extensions: &[".cs", ".vb"],
        root_markers: &["*.sln", "*.slnx", "*.csproj", "*.vbproj"],
        node_modules: false,
    },
    Builtin {
        name: "shfmt",
        command: "shfmt",
        args: &["-w", "$FILE"],
        extensions: &[".sh", ".bash"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "terraform",
        command: "terraform",
        args: &["fmt", "$FILE"],
        extensions: &[".tf", ".tfvars"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "zig",
        command: "zig",
        args: &["fmt", "$FILE"],
        extensions: &[".zig", ".zon"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "dart",
        command: "dart",
        args: &["format", "$FILE"],
        extensions: &[".dart"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "ktlint",
        command: "ktlint",
        args: &["-F", "$FILE"],
        extensions: &[".kt", ".kts"],
        root_markers: &[],
        node_modules: false,
    },
    Builtin {
        name: "mix",
        command: "mix",
        args: &["format", "$FILE"],
        extensions: &[".ex", ".exs", ".heex"],
        root_markers: &["mix.exs"],
        node_modules: false,
    },
    Builtin {
        name: "stylua",
        command: "stylua",
        args: &["$FILE"],
        extensions: &[".lua"],
        root_markers: &["stylua.toml", ".stylua.toml"],
        node_modules: false,
    },
];

pub fn builtin_formatters() -> Vec<FormatterSpec> {
    BUILTINS
        .iter()
        .map(|builtin| FormatterSpec {
            name: builtin.name.to_owned(),
            command: builtin.command.to_owned(),
            args: to_strings(builtin.args),
            extensions: to_strings(builtin.extensions),
            root_markers: to_strings(builtin.root_markers),
            node_modules: builtin.node_modules,
        })
        .collect()
}

fn to_strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
