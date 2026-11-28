//! POSIX-shell command lines.

use std::time::{Duration, Instant};

use systemprompt_engine::tools::{Dialect, split_commands};

use super::{contains_all, paths, posix, prefixes, redirects};

fn quick<T>(work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = work();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
    out
}

#[test]
fn every_command_in_lists_pipelines_and_substitutions_is_found() {
    let cases: [(&str, &[&str]); 9] = [
        ("ls", &["ls"]),
        ("a; b && c || d | e & f", &["a", "b", "c", "d", "e", "f"]),
        (
            "echo $(git rev-parse HEAD) `whoami`",
            &["echo", "git", "whoami"],
        ),
        ("X=$(date) make", &["date", "make"]),
        ("cat > $(mktemp)", &["cat", "mktemp"]),
        ("echo ${X:-$(hostname)}", &["echo", "hostname"]),
        ("echo $(echo $(id))", &["echo", "echo", "id"]),
        ("A=1 B=2 cargo build", &["cargo"]),
        ("ls # rm -rf /", &["ls"]),
    ];
    for (line, wanted) in cases {
        let found = posix(line);
        assert!(contains_all(&found, wanted), "{line}: {found:?}");
    }
}

#[test]
fn operators_inside_quotes_or_escapes_stay_in_their_argument() {
    for line in [
        "echo 'a; b && c'",
        "echo \"a | b\"",
        "echo a\\;b",
        "grep 'x || y' file",
        "echo \"\\\"; rm x\"",
    ] {
        assert_eq!(posix(line).len(), 1, "{line}: {:?}", posix(line));
    }
}

#[test]
fn single_quoted_text_is_inert() {
    for line in [
        "echo '$(rm -rf x)'",
        "echo '`rm x`'",
        "echo '$((1 + $(rm x)))'",
        "printf '%s' '; rm x'",
    ] {
        let found = posix(line);
        assert_eq!(found.len(), 1, "{line}: {found:?}");
    }
}

#[test]
fn assignment_only_input_yields_no_command() {
    assert_eq!(posix("A=1 B=$HOME"), Vec::<String>::new());
    assert_eq!(posix("A=1; B=2"), Vec::<String>::new());
}

#[test]
fn redirect_targets_belong_to_their_own_command() {
    assert_eq!(
        redirects("a > x | b 2> y; c >> z && d"),
        [
            vec!["x".to_owned()],
            vec!["y".to_owned()],
            vec!["z".to_owned()],
            vec![]
        ]
    );
    assert_eq!(redirects("a>x"), [vec!["x".to_owned()]]);
    assert_eq!(redirects("A=1 a \\\n > out"), [vec!["out".to_owned()]]);
    assert_eq!(posix("a | A=1 b > out"), ["a", "b"]);
}

#[test]
fn a_redirect_before_a_directory_operand_still_yields_the_directory() {
    assert_eq!(
        paths("cd > ../log ../dir", Dialect::Posix),
        ["../dir", "../log"]
    );
    assert_eq!(paths("> ../alone", Dialect::Posix), ["../alone"]);
}

#[test]
fn directory_targets_are_found_across_compound_forms_and_end_of_options() {
    let found = paths(
        "cd -- ../out && command cd ../other; (cd ../third)",
        Dialect::Posix,
    );
    assert!(
        contains_all(&found, &["../out", "../other", "../third"]),
        "{found:?}"
    );
    let declared = paths("export X=$(cd /tmp/work && ls)", Dialect::Posix);
    assert!(declared.contains(&"/tmp/work".to_owned()), "{declared:?}");
}

#[test]
#[ignore = "parity ledger: directory arguments built from variables are not expanded"]
fn static_variables_in_directory_arguments_are_expanded() {
    let found = paths("D=../out; cd $D", Dialect::Posix);
    assert!(found.contains(&"../out".to_owned()), "{found:?}");
}

#[test]
#[ignore = "parity ledger: here-document body lines are scanned as commands"]
fn here_document_bodies_never_become_commands() {
    let found = posix("cat <<EOF > out.txt\nhello; rm -rf x\nEOF\necho done");
    assert!(!found.contains(&"rm".to_owned()), "{found:?}");
    assert!(contains_all(&found, &["cat", "echo"]), "{found:?}");
    let quoted = posix("cat <<A <<'B' | grep x\none\nA\n$(rm y)\nB\nls");
    assert!(!quoted.contains(&"rm".to_owned()), "{quoted:?}");
    assert!(contains_all(&quoted, &["cat", "grep", "ls"]), "{quoted:?}");
}

#[test]
#[ignore = "parity ledger: malformed or unterminated input is split best-effort instead of refused"]
fn malformed_input_is_refused_rather_than_partly_accepted() {
    for line in [
        "echo 'open",
        "echo \"open; rm x",
        "echo $(ls",
        "a && && b",
        "echo ${x",
        "X=$(\"unterminated) cmd",
        "cat > \"out",
        "(a; b",
    ] {
        assert!(split_commands(line, Dialect::Posix).is_empty(), "{line}");
    }
}

#[test]
fn a_long_argument_list_still_gives_the_short_prefix() {
    let args: String = (0..500).map(|index| format!(" file{index}.rs")).collect();
    assert_eq!(
        prefixes(&format!("git add{args}"), Dialect::Posix),
        ["git add"]
    );
}

#[test]
fn dynamic_command_heads_are_kept_as_written() {
    assert_eq!(posix("$TOOL --flag"), ["$TOOL"]);
    let found = posix("\"$(which x)\" arg");
    assert!(found.contains(&"which".to_owned()), "{found:?}");
}

#[test]
fn wrappers_are_reported_at_their_boundary() {
    for (line, head) in [
        ("sudo rm -rf x", "sudo"),
        ("env A=1 rm x", "env"),
        ("xargs rm < list", "xargs"),
        ("bash -c 'rm x'", "bash"),
        ("time make", "make"),
    ] {
        assert_eq!(
            posix(line).first().map(String::as_str),
            Some(head),
            "{line}"
        );
    }
}

#[test]
fn groups_conditionals_and_loops_reveal_their_commands() {
    let cases: [(&str, &[&str]); 8] = [
        ("{ a; } > f; b", &["a", "b"]),
        ("{ a; } && : > f", &["a", ":"]),
        (
            "if a; then b; elif c; then d; else e; fi",
            &["a", "b", "c", "d", "e"],
        ),
        ("while a; do b; done", &["a", "b"]),
        ("until a; do b; done", &["a", "b"]),
        ("for f in *; do rm \"$f\"; done", &["rm"]),
        ("f() { a; }; f", &["a", "f"]),
        ("x=$(while c; do d; done)", &["c", "d"]),
    ];
    for (line, wanted) in cases {
        let found = posix(line);
        assert!(contains_all(&found, wanted), "{line}: {found:?}");
    }
}

#[test]
fn function_definitions_of_every_spelling_reveal_their_bodies() {
    for line in [
        "f() { body; }",
        "function f() { body; }",
        "f () \\\n{ body; }",
        "f() # note\n{ body; }",
        "my-func.name() { body; }",
        "f() ( body )",
    ] {
        let found = posix(line);
        assert!(found.contains(&"body".to_owned()), "{line}: {found:?}");
    }
}

#[test]
fn a_function_keyword_definition_without_parentheses_reveals_its_body() {
    let found = posix("function f { body; }");
    assert!(found.contains(&"body".to_owned()), "{found:?}");
}

#[test]
fn process_substitutions_and_nested_groupings_reveal_their_commands() {
    for (line, wanted) in [
        ("diff <(sort a) >(tee b)", ["sort", "tee"]),
        ("[[ -n x ]] && cat <(inner)", ["cat", "inner"]),
        ("( ( a ) ; { b; } )", ["a", "b"]),
    ] {
        let found = posix(line);
        assert!(contains_all(&found, &wanted), "{line}: {found:?}");
    }
}

#[test]
fn quoted_text_that_looks_like_a_substitution_is_never_a_command() {
    for line in ["echo '$(rm x)'", "grep '`date`' f", "echo 'a <(rm x)'"] {
        let found = posix(line);
        assert!(
            !found.contains(&"rm".to_owned()) && !found.contains(&"date".to_owned()),
            "{line}: {found:?}"
        );
    }
}

#[test]
#[ignore = "parity ledger: an escaped dollar sign still opens a command substitution"]
fn an_escaped_substitution_is_never_a_command() {
    let found = posix("echo \\$(rm x)");
    assert!(!found.contains(&"rm".to_owned()), "{found:?}");
}

#[test]
#[ignore = "parity ledger: a repeat loop is reported as the command `repeat`"]
fn a_repeat_loop_reveals_its_command() {
    let found = posix("repeat 3 echo hi");
    assert!(found.contains(&"echo".to_owned()), "{found:?}");
}

#[test]
fn shell_dialect_extensions_are_accepted() {
    for (line, head) in [
        ("ls !(skip)", "ls"),
        ("print ${(U)name}", "print"),
        ("ls *(.)", "ls"),
        ("echo $'a\\nb'", "echo"),
        ("echo $\"translated\"", "echo"),
        ("((x++)); next", "next"),
    ] {
        let found = posix(line);
        assert!(found.contains(&head.to_owned()), "{line}: {found:?}");
    }
}

#[test]
fn quoting_spellings_keep_their_raw_words() {
    let segment = &split_commands("echo $'a\\tb' \"x y\" 'z'", Dialect::Posix)[0];
    assert_eq!(segment.words.len(), 4, "{:?}", segment.words);
    assert_eq!(segment.words[2], "x y");
    assert_eq!(segment.words[3], "z");
}

#[test]
fn repeated_expansion_flags_are_scanned_in_bounded_time() {
    let flags = format!("echo {{({}q)x}}", "q".repeat(20_000));
    quick(|| split_commands(&flags, Dialect::Posix));
}

#[test]
fn deep_arithmetic_is_scanned_in_bounded_time() {
    let arithmetic = format!("echo {}1{}", "$((".repeat(2_000), "))".repeat(2_000));
    quick(|| split_commands(&arithmetic, Dialect::Posix));
}

#[test]
fn deeply_nested_conditionals_are_scanned_in_bounded_time() {
    let nested = format!(
        "{} true {}",
        "if true; then ".repeat(2_000),
        "; fi".repeat(2_000)
    );
    quick(|| split_commands(&nested, Dialect::Posix));
}

#[test]
fn deeply_nested_substitutions_are_scanned_in_bounded_time() {
    let substitutions = format!("{}x{}", "$(".repeat(5_000), ")".repeat(5_000));
    quick(|| split_commands(&substitutions, Dialect::Posix));
}

#[test]
fn very_long_input_is_scanned_in_bounded_time() {
    let long = "a ".repeat(200_000);
    quick(|| split_commands(&long, Dialect::Posix));
}

#[test]
#[ignore = "parity ledger: glob qualifiers that run code are not refused"]
fn glob_qualifiers_that_run_code_are_refused() {
    assert!(split_commands("ls *(e:'rm x':)", Dialect::Posix).is_empty());
}

#[test]
fn separators_split_commands_at_line_ends_too() {
    assert_eq!(posix("a\nb\r\nc"), ["a", "b", "c"]);
}

#[test]
fn empty_lines_blank_lines_and_comments_yield_no_command() {
    for line in ["", "   ", "\n\n", "# only a comment", "\n# c\n\n"] {
        assert_eq!(posix(line), Vec::<String>::new(), "{line:?}");
    }
    assert_eq!(posix("a\n\n# note\nb"), ["a", "b"]);
}

#[test]
fn whitespace_that_is_not_shell_whitespace_does_not_separate_words() {
    let segments = split_commands("echo\u{a0}x", Dialect::Posix);
    assert_eq!(segments[0].words, ["echo\u{a0}x"]);
}

#[test]
#[ignore = "parity ledger: Lexer::redirect takes any all-digit word before a redirect as a descriptor, so a quoted or escaped 123 is dropped instead of staying the command name"]
fn quoted_digits_before_a_redirect_stay_the_command_name() {
    for head in ["'123'", "\"123\"", "1\\23"] {
        let segments = split_commands(&format!("{head}>/dev/null argument"), Dialect::Posix);
        assert_eq!(segments.len(), 1, "{head}: {segments:?}");
        assert_eq!(segments[0].words, ["123", "argument"], "{head}");
        assert_eq!(segments[0].redirects, ["/dev/null"], "{head}");
    }
    let quoted = split_commands("F\"OO\"=bar '123'>out", Dialect::Posix);
    assert_eq!(quoted.len(), 1, "{quoted:?}");
    assert_eq!(quoted[0].head(Dialect::Posix).as_deref(), Some("FOO=bar"));
    assert_eq!(quoted[0].redirects, ["out"]);
}

#[test]
fn a_descriptor_duplication_on_a_group_adds_no_command() {
    for line in [
        "{ printf ok; } 2>&1",
        "(printf ok) 2>&1",
        "(printf ok) 2>&-",
    ] {
        assert_eq!(posix(line), ["printf"], "{line}");
    }
}

#[test]
#[ignore = "parity ledger: the lexer has no here-document handling; Lexer::redirect takes the delimiter as a redirect target and the body lines are scanned as commands"]
fn an_ansi_c_quoted_here_document_delimiter_ends_the_body() {
    let found = posix("cat <<$'E\\[F'\nE[F\nE\\[F\nprintf done");
    assert_eq!(found, ["cat", "printf"], "{found:?}");
}

#[test]
fn directory_targets_are_found_whatever_redirects_or_quotes_surround_them() {
    for template in [
        "cd >/dev/null TARGET",
        "cd 2>/dev/null TARGET",
        "cd \"TARGET\"",
    ] {
        let line = template.replace("TARGET", "/tmp/scan-target");
        let found = paths(&line, Dialect::Posix);
        assert!(
            found.contains(&"/tmp/scan-target".to_owned()),
            "{line}: {found:?}"
        );
    }
}

#[test]
#[ignore = "parity ledger: Lexer::posix has no ANSI-C quoting, so $'...' keeps a literal dollar sign and path_like does not report the target"]
fn an_ansi_c_quoted_directory_target_is_found() {
    let found = paths("cd $'/tmp/scan-target'", Dialect::Posix);
    assert!(found.contains(&"/tmp/scan-target".to_owned()), "{found:?}");
}

#[test]
#[ignore = "parity ledger: Segment::command_words skips the words of POSIX_SKIPPED, which holds time but not coproc, so coproc is reported as the command"]
fn the_command_after_coproc_is_found() {
    let found = posix("coproc git status");
    assert!(contains_all(&found, &["git"]), "{found:?}");
}

#[test]
fn commands_inside_redirect_targets_and_groups_keep_their_place() {
    let cases: [(&str, &[&str]); 8] = [
        ("(cat >out) | tail >log", &["cat", "tail"]),
        ("{ cat >out; } && tail >log", &["cat", "tail"]),
        ("echo $(pwd | cat >out) >outer", &["echo", "pwd", "cat"]),
        ("echo `pwd | cat >out` >outer", &["echo", "pwd", "cat"]),
        ("pwd | cat >$(printf out)", &["pwd", "cat", "printf"]),
        ("cat <(pwd | cat >out) | cat >tail", &["cat", "pwd", "cat"]),
        (
            "if true; then printf ok && git >out status; else cat >log; fi",
            &["true", "printf", "git", "cat"],
        ),
        ("time git status", &["git"]),
    ];
    for (line, wanted) in cases {
        let found = posix(line);
        assert!(contains_all(&found, wanted), "{line}: {found:?}");
    }
    assert_eq!(prefixes("time git status", Dialect::Posix), ["git status"]);
    let quoted = split_commands(
        "pwd | cat '>' \"2>out\" escaped\\>word >out",
        Dialect::Posix,
    );
    assert_eq!(quoted.len(), 2, "{quoted:?}");
    assert_eq!(quoted[1].redirects, ["out"]);
    assert_eq!(quoted[1].words, ["cat", ">", "2>out", "escaped>word"]);
}

#[test]
fn a_trailing_line_continuation_leaves_one_command() {
    for line in [
        "cat\\\n",
        "cat \\\n",
        "cat\\\n\\\n",
        "cat\\\n;",
        "cat >out\\\n",
        "cat >out \\\n",
    ] {
        assert_eq!(posix(line), ["cat"], "{line:?}");
        assert_eq!(prefixes(line, Dialect::Posix), ["cat"], "{line:?}");
    }
    for line in [
        "printf 'literal\\\n'\\\n",
        "printf \"literal\\\n\"\\\n",
        "printf a\\\nb\\\n",
    ] {
        let segments = split_commands(line, Dialect::Posix);
        assert_eq!(segments.len(), 1, "{line:?}: {segments:?}");
        assert_eq!(segments[0].words.len(), 2, "{line:?}: {segments:?}");
    }
}

#[test]
fn an_operator_at_a_line_end_takes_the_command_after_blank_lines_and_comments() {
    for operator in ["&&", "||", "|", "|&"] {
        let line = format!("printf ok {operator}\n\n# comment\npwd\n\n");
        assert_eq!(posix(&line), ["printf", "pwd"], "{line:?}");
    }
}
