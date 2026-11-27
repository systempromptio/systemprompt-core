use systemprompt_engine::tools::{Dialect, split_commands};

fn heads(command: &str, dialect: Dialect) -> Vec<String> {
    split_commands(command, dialect)
        .iter()
        .filter_map(|segment| segment.head(dialect))
        .collect()
}

fn prefixes(command: &str, dialect: Dialect) -> Vec<String> {
    split_commands(command, dialect)
        .iter()
        .filter(|segment| segment.head(dialect).is_some())
        .map(|segment| segment.prefix(dialect))
        .collect()
}

#[test]
fn posix_splits_on_operators() {
    assert_eq!(
        heads(
            "git status && npm test | grep ok; echo done",
            Dialect::Posix
        ),
        ["git", "npm", "grep", "echo"]
    );
    assert_eq!(
        prefixes(
            "git status && npm run build -- --watch || cargo test -p x",
            Dialect::Posix
        ),
        ["git status", "npm run build", "cargo test"]
    );
}

#[test]
fn posix_quotes_keep_operators_inside_words() {
    let segments = split_commands(r#"echo "a && b" 'c | d' e\ f"#, Dialect::Posix);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].words, ["echo", "a && b", "c | d", "e f"]);
}

#[test]
fn posix_skips_assignments_and_finds_substitutions() {
    assert_eq!(
        heads("FOO=1 BAR=2 cargo build --release", Dialect::Posix),
        ["cargo"]
    );
    assert_eq!(
        heads(
            "ls $(git rev-parse --show-toplevel) `whoami`",
            Dialect::Posix
        ),
        ["ls", "git", "whoami"]
    );
    assert_eq!(
        heads("if true; then rm -rf build; fi", Dialect::Posix),
        ["true", "rm"]
    );
    assert_eq!(
        heads("# just a comment", Dialect::Posix),
        Vec::<String>::new()
    );
}

#[test]
fn redirects_are_targets_not_words() {
    let segments = split_commands("cat a.txt > ../out.txt 2>&1", Dialect::Posix);
    assert_eq!(segments[0].words, ["cat", "a.txt"]);
    assert_eq!(segments[0].redirects, ["../out.txt"]);
    assert_eq!(segments[0].path_words(Dialect::Posix), ["../out.txt"]);
    assert_eq!(
        split_commands("make 2>/dev/null", Dialect::Posix)[0].path_words(Dialect::Posix),
        Vec::<String>::new()
    );
}

#[test]
fn powershell_commands_blocks_and_assignments() {
    let line =
        "Get-ChildItem -Path C:\\src | Where-Object { $_.Length -gt 1 } ; Remove-Item 'a b.txt'";
    assert_eq!(
        heads(line, Dialect::PowerShell),
        ["get-childitem", "where-object", "remove-item"]
    );
    assert_eq!(
        heads(
            "$files = Get-Item *.rs; & 'C:\\Tools\\Git.EXE' status",
            Dialect::PowerShell
        ),
        ["get-item", "c:\\tools\\git"]
    );
    assert_eq!(
        heads(
            "Write-Output \"$(git branch --show-current)\"",
            Dialect::PowerShell
        ),
        ["write-output", "git"]
    );
    assert_eq!(
        prefixes("git.exe Commit -m \"x\"", Dialect::PowerShell),
        ["git Commit"]
    );
}

#[test]
fn powershell_paths_include_drive_letters() {
    let segment = &split_commands(
        "Copy-Item -Destination:D:\\out .\\a.txt",
        Dialect::PowerShell,
    )[0];
    assert_eq!(
        segment.path_words(Dialect::PowerShell),
        ["D:\\out", ".\\a.txt"]
    );
}

#[test]
fn cmd_separators_and_switches() {
    assert_eq!(
        heads(
            "@echo off & dir /s && del \"a b.txt\" || type x",
            Dialect::Cmd
        ),
        ["echo", "dir", "del", "type"]
    );
    let segment = &split_commands("copy /y src\\a.txt %TEMP%\\b.txt", Dialect::Cmd)[0];
    assert_eq!(segment.path_words(Dialect::Cmd), ["src\\a.txt"]);
}

#[test]
fn path_words_cut_globs_and_flags() {
    let segment = &split_commands(
        "rm -rf ../build/* --out=/tmp/x https://a.b/c ~/notes",
        Dialect::Posix,
    )[0];
    assert_eq!(
        segment.path_words(Dialect::Posix),
        ["../build/", "/tmp/x", "~/notes"]
    );
}
