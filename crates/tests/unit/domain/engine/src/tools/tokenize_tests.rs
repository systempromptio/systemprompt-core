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

#[test]
fn quoted_digits_before_a_redirect_are_a_word_not_a_descriptor() {
    let segments = split_commands("'2'>out run", Dialect::Posix);
    assert_eq!(segments[0].words, ["2", "run"]);
    assert_eq!(segments[0].redirects, ["out"]);
    let bare = split_commands("run 2>out", Dialect::Posix);
    assert_eq!(bare[0].words, ["run"]);
    assert_eq!(heads("\"A\"=1 run", Dialect::Posix), ["A=1"]);
}

#[test]
fn ansi_c_quoting_decodes_escapes_and_keeps_unknown_ones() {
    let segments = split_commands(r"printf $'a\tb\x41\101\q' $'/tmp/x'", Dialect::Posix);
    assert_eq!(segments[0].words, ["printf", "a\tbAA\\q", "/tmp/x"]);
    assert!(
        segments[0]
            .path_words(Dialect::Posix)
            .contains(&"/tmp/x".to_owned())
    );
}

#[test]
fn here_document_bodies_are_skipped_and_only_their_expansions_scanned() {
    assert_eq!(
        heads("cat <<-'X' >out\n\trm a\n\tX\nls", Dialect::Posix),
        ["cat", "ls"]
    );
    assert_eq!(
        heads("cat <<X\nrm $(whoami)\nX\nls", Dialect::Posix),
        ["cat", "ls", "whoami"]
    );
    assert_eq!(heads("cat <<<word; ls", Dialect::Posix), ["cat", "ls"]);
}

#[test]
fn arithmetic_is_one_word_whose_substitutions_are_still_scanned() {
    let segments = split_commands("echo $((1 + $(id -u))) \"$((2))\"", Dialect::Posix);
    assert_eq!(segments[0].words, ["echo", "$((1 + $(id -u)))", "$((2))"]);
    assert_eq!(heads("echo $((1 + $(id -u)))", Dialect::Posix), ["echo", "id"]);
}

#[test]
fn the_command_after_coproc_is_the_head() {
    assert_eq!(heads("coproc git status", Dialect::Posix), ["git"]);
}
