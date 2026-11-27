//! The built-in table of well-known language servers, offered only when their
//! binary is found.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::{ServerId, ServerSpec};

pub(super) struct Builtin {
    pub(super) id: &'static str,
    pub(super) command: &'static str,
    args: &'static [&'static str],
    extensions: &'static [&'static str],
    root_markers: &'static [&'static str],
    pub(super) group: Option<&'static str>,
}

impl Builtin {
    pub(super) fn spec(&self) -> ServerSpec {
        ServerSpec {
            id: ServerId::new(self.id),
            command: self.command.to_owned(),
            args: self.args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: BTreeMap::new(),
            extensions: self
                .extensions
                .iter()
                .map(|ext| (*ext).to_owned())
                .collect(),
            root_markers: self
                .root_markers
                .iter()
                .map(|marker| (*marker).to_owned())
                .collect(),
            initialization_options: None,
        }
    }
}

const JS_ROOTS: &[&str] = &[
    "pnpm-lock.yaml",
    "yarn.lock",
    "package-lock.json",
    "bun.lock",
    "bun.lockb",
    "tsconfig.json",
    "jsconfig.json",
    "package.json",
];
const PYTHON_EXTS: &[&str] = &["py", "pyi"];
const PYTHON_ROOTS: &[&str] = &[
    "pyrightconfig.json",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "Pipfile",
];
const CSHARP_EXTS: &[&str] = &["cs", "csx"];
const DOTNET_ROOTS: &[&str] = &["*.sln", "*.slnx", "*.csproj", "global.json"];
const JVM_ROOTS: &[&str] = &[
    "settings.gradle",
    "settings.gradle.kts",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
];

pub(super) const BUILTINS: &[Builtin] = &[
    Builtin {
        id: "rust-analyzer",
        command: "rust-analyzer",
        args: &[],
        extensions: &["rs"],
        root_markers: &["Cargo.lock", "rust-project.json", "Cargo.toml"],
        group: None,
    },
    Builtin {
        id: "typescript-language-server",
        command: "typescript-language-server",
        args: &["--stdio"],
        extensions: &["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"],
        root_markers: JS_ROOTS,
        group: None,
    },
    Builtin {
        id: "gopls",
        command: "gopls",
        args: &[],
        extensions: &["go"],
        root_markers: &["go.work", "go.mod"],
        group: None,
    },
    Builtin {
        id: "basedpyright",
        command: "basedpyright-langserver",
        args: &["--stdio"],
        extensions: PYTHON_EXTS,
        root_markers: PYTHON_ROOTS,
        group: Some("python"),
    },
    Builtin {
        id: "pyright",
        command: "pyright-langserver",
        args: &["--stdio"],
        extensions: PYTHON_EXTS,
        root_markers: PYTHON_ROOTS,
        group: Some("python"),
    },
    Builtin {
        id: "clangd",
        command: "clangd",
        args: &[],
        extensions: &[
            "c", "h", "cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx", "h++", "m", "mm",
        ],
        root_markers: &[
            "compile_commands.json",
            "compile_flags.txt",
            ".clangd",
            "CMakeLists.txt",
        ],
        group: None,
    },
    Builtin {
        id: "csharp-ls",
        command: "csharp-ls",
        args: &[],
        extensions: CSHARP_EXTS,
        root_markers: DOTNET_ROOTS,
        group: Some("csharp"),
    },
    Builtin {
        id: "omnisharp",
        command: "OmniSharp",
        args: &["-lsp"],
        extensions: CSHARP_EXTS,
        root_markers: DOTNET_ROOTS,
        group: Some("csharp"),
    },
    Builtin {
        id: "lua-language-server",
        command: "lua-language-server",
        args: &[],
        extensions: &["lua"],
        root_markers: &[
            ".luarc.json",
            ".luarc.jsonc",
            ".stylua.toml",
            "stylua.toml",
            "selene.toml",
        ],
        group: None,
    },
    Builtin {
        id: "zls",
        command: "zls",
        args: &[],
        extensions: &["zig", "zon"],
        root_markers: &["build.zig"],
        group: None,
    },
    Builtin {
        id: "ruby-lsp",
        command: "ruby-lsp",
        args: &[],
        extensions: &["rb", "rake", "gemspec", "ru"],
        root_markers: &["Gemfile"],
        group: None,
    },
    Builtin {
        id: "bash-language-server",
        command: "bash-language-server",
        args: &["start"],
        extensions: &["sh", "bash"],
        root_markers: &[],
        group: None,
    },
    Builtin {
        id: "yaml-language-server",
        command: "yaml-language-server",
        args: &["--stdio"],
        extensions: &["yaml", "yml"],
        root_markers: &[],
        group: None,
    },
    Builtin {
        id: "intelephense",
        command: "intelephense",
        args: &["--stdio"],
        extensions: &["php"],
        root_markers: &["composer.json"],
        group: None,
    },
    Builtin {
        id: "svelte-language-server",
        command: "svelteserver",
        args: &["--stdio"],
        extensions: &["svelte"],
        root_markers: JS_ROOTS,
        group: None,
    },
    Builtin {
        id: "dart",
        command: "dart",
        args: &["language-server", "--protocol=lsp"],
        extensions: &["dart"],
        root_markers: &["pubspec.yaml"],
        group: None,
    },
    Builtin {
        id: "sourcekit-lsp",
        command: "sourcekit-lsp",
        args: &[],
        extensions: &["swift"],
        root_markers: &["Package.swift"],
        group: None,
    },
    Builtin {
        id: "terraform-ls",
        command: "terraform-ls",
        args: &["serve"],
        extensions: &["tf", "tfvars"],
        root_markers: &[".terraform"],
        group: None,
    },
    Builtin {
        id: "elixir-ls",
        command: "elixir-ls",
        args: &[],
        extensions: &["ex", "exs"],
        root_markers: &["mix.exs"],
        group: None,
    },
    Builtin {
        id: "kotlin-language-server",
        command: "kotlin-language-server",
        args: &[],
        extensions: &["kt", "kts"],
        root_markers: JVM_ROOTS,
        group: None,
    },
    Builtin {
        id: "jdtls",
        command: "jdtls",
        args: &[],
        extensions: &["java"],
        root_markers: JVM_ROOTS,
        group: None,
    },
];

pub fn builtin_servers() -> Vec<ServerSpec> {
    BUILTINS.iter().map(Builtin::spec).collect()
}
