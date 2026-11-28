//! PowerShell and cmd command lines.

use std::time::{Duration, Instant};

use systemprompt_engine::tools::{Dialect, split_commands};

use super::{contains_all, heads, paths, prefixes, ps};

#[test]
fn ordinary_syntax_reveals_its_commands() {
    let cases: [(&str, &[&str]); 12] = [
        (
            "Get-ChildItem | Where-Object { $_.Length -gt 1 } | Sort-Object",
            &["get-childitem", "where-object", "sort-object"],
        ),
        ("$files = Get-Item *.rs", &["get-item"]),
        (
            "if ($ok) { Remove-Item a } else { Write-Host b }",
            &["remove-item", "write-host"],
        ),
        ("foreach ($f in $list) { Copy-Item $f d }", &["copy-item"]),
        ("while ($true) { Start-Sleep 1 }", &["start-sleep"]),
        ("function Go { Get-Process }; Go", &["get-process", "go"]),
        ("$h = @{ a = (Get-Date) }", &["get-date"]),
        (
            "Write-Output \"$(Get-Location) $(Join-Path a b)\"",
            &["write-output", "get-location", "join-path"],
        ),
        ("& 'C:\\Tools\\git.exe' status", &["c:\\tools\\git"]),
        ("git --% log --oneline", &["git"]),
        ("Get-A; Get-B & Get-C", &["get-a", "get-b", "get-c"]),
        (
            "Microsoft.PowerShell.Management\\Get-Item x",
            &["microsoft.powershell.management\\get-item"],
        ),
    ];
    for (line, wanted) in cases {
        let found = ps(line);
        assert!(contains_all(&found, wanted), "{line}: {found:?}");
    }
}

#[test]
fn here_strings_stay_attached_to_their_command() {
    let found = ps("$t = @'\nRemove-Item x\n'@; Write-Output $t");
    assert!(!found.contains(&"remove-item".to_owned()), "{found:?}");
    assert!(found.contains(&"write-output".to_owned()), "{found:?}");
}

#[test]
fn quoting_rules_cannot_hide_a_later_command() {
    let found = ps("Write-Output 'a`'; Remove-Item x");
    assert!(found.contains(&"remove-item".to_owned()), "{found:?}");
    let doubled = ps("Write-Output \"a\"\"b\"; Remove-Item y");
    assert!(doubled.contains(&"remove-item".to_owned()), "{doubled:?}");
}

#[test]
fn separators_inside_strings_or_escaped_stay_in_the_argument() {
    for line in [
        "Write-Output 'a; Remove-Item x'",
        "Write-Output \"a | b\"",
        "Write-Output a`;b",
    ] {
        assert_eq!(ps(line).len(), 1, "{line}: {:?}", ps(line));
    }
}

#[test]
fn backticks_are_literal_in_verbatim_strings_and_escapes_are_not_evaluated() {
    assert_eq!(ps("Write-Output 'C:\\a`b'"), ["write-output"]);
    let found = ps("Write-Output \"cost `$(Remove-Item x)\"");
    assert!(!found.contains(&"remove-item".to_owned()), "{found:?}");
}

#[test]
fn nested_script_blocks_are_scanned() {
    let found = ps("Invoke-Command { Get-A { Get-B { Remove-Item deep } } }");
    assert!(
        contains_all(&found, &["invoke-command", "get-a", "get-b", "remove-item"]),
        "{found:?}"
    );
}

#[test]
fn comments_are_ignored_but_hashes_inside_words_are_not_comments() {
    assert_eq!(ps("Get-A # Remove-Item x"), ["get-a"]);
    assert_eq!(ps("<# Remove-Item x #> Get-B"), ["get-b"]);
    let segment = &split_commands("Write-Output a#b; Get-C", Dialect::PowerShell);
    assert!(
        segment.iter().any(|s| s.words.contains(&"a#b".to_owned())),
        "{segment:?}"
    );
    assert!(ps("Write-Output a#b; Get-C").contains(&"get-c".to_owned()));
}

#[test]
fn redirects_do_not_swallow_arguments() {
    let segments = split_commands("Get-X 2>&1 arg > out.txt", Dialect::PowerShell);
    assert!(
        segments[0].words.contains(&"arg".to_owned()),
        "{segments:?}"
    );
    assert!(
        !segments[0].words.contains(&"out.txt".to_owned()),
        "{segments:?}"
    );
}

#[test]
fn carriage_returns_separate_statements() {
    assert_eq!(ps("Get-A\rGet-B\r\nGet-C"), ["get-a", "get-b", "get-c"]);
}

#[test]
fn directory_parameters_are_found_whatever_their_case_or_quoting() {
    let found = paths(
        "Get-ChildItem -PATH:'C:\\a b' ; Set-Location -LiteralPath \"..\\up\"",
        Dialect::PowerShell,
    );
    assert!(contains_all(&found, &["C:\\a b", "..\\up"]), "{found:?}");
}

#[test]
fn a_statement_head_cmdlet_with_ordinary_parameters_is_kept() {
    let found = ps("ForEach-Object -InputObject 1 -Process { Remove-Item x }");
    assert!(
        contains_all(&found, &["foreach-object", "remove-item"]),
        "{found:?}"
    );
}

#[test]
#[ignore = "parity ledger: the call operator is dropped from the saved approval prefix"]
fn approvals_for_the_call_operator_keep_the_operator() {
    let saved = prefixes("& $tool --run", Dialect::PowerShell);
    assert!(
        saved.iter().any(|prefix| prefix.starts_with('&')),
        "{saved:?}"
    );
}

#[test]
fn variable_expressions_differ_from_delegated_execution() {
    assert_eq!(ps("$x.Method()"), Vec::<String>::new());
}

#[test]
#[ignore = "parity ledger: a call operator on a variable yields no command"]
fn delegated_execution_through_a_variable_is_a_command() {
    assert_ne!(ps("& $x arg"), Vec::<String>::new());
}

#[test]
fn commands_nested_in_directory_expressions_are_found_unevaluated() {
    let found = ps("Set-Location (Join-Path $HOME (Get-Random))");
    assert!(
        contains_all(&found, &["set-location", "join-path", "get-random"]),
        "{found:?}"
    );
}

#[test]
#[ignore = "parity ledger: typographic quotes are not refused in PowerShell lines"]
fn typographic_quotes_fail_closed() {
    assert!(
        split_commands(
            "Write-Output \u{201c}a; Remove-Item x\u{201d}",
            Dialect::PowerShell
        )
        .is_empty()
    );
}

#[test]
fn deeply_nested_script_blocks_are_bounded() {
    let started = Instant::now();
    let nested = format!("{}Get-X{}", "& { ".repeat(3_000), " }".repeat(3_000));
    split_commands(&nested, Dialect::PowerShell);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn deeply_mixed_strings_and_subexpressions_are_bounded() {
    let started = Instant::now();
    let mixed = format!("{}x{}", "\"$(@(".repeat(2_000), "))\"".repeat(2_000));
    split_commands(&mixed, Dialect::PowerShell);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn very_long_powershell_input_is_bounded() {
    let started = Instant::now();
    split_commands(&"Get-A ".repeat(100_000), Dialect::PowerShell);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn cmd_lines_split_on_their_separators() {
    assert_eq!(
        heads("dir & del x && type y || echo z", Dialect::Cmd),
        ["dir", "del", "type", "echo"]
    );
}

#[test]
#[ignore = "parity ledger: incomplete PowerShell input is split best-effort instead of refused"]
fn incomplete_powershell_input_is_refused() {
    for line in [
        "Get-A (",
        "Get-A { Remove-Item x",
        "Write-Output 'open",
        "Write-Output \"open",
        "Get-A |",
    ] {
        assert!(
            split_commands(line, Dialect::PowerShell).is_empty(),
            "{line}"
        );
    }
}
