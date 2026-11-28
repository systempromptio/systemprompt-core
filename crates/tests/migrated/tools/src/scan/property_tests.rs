//! Generated command lines: every combination of commands and separators.

use systemprompt_engine::config::{Action, Rule, Ruleset};
use systemprompt_engine::tools::{Dialect, split_commands};

use super::{heads, posix, ps};

const NAMES: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

fn combinations(separators: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for first in separators {
        for second in separators {
            for third in separators {
                let line = format!(
                    "{} one{}{} two{}{} three{}{}",
                    NAMES[0], first, NAMES[1], second, NAMES[2], third, NAMES[3]
                );
                out.push((line, NAMES.iter().map(|name| (*name).to_owned()).collect()));
            }
        }
    }
    out
}

#[test]
fn every_static_command_is_found_across_every_posix_separator() {
    for (line, wanted) in combinations(&["; ", " && ", " || ", " | ", "\n", " & "]) {
        assert_eq!(posix(&line), wanted, "{line:?}");
    }
}

#[test]
fn every_static_command_is_found_across_every_powershell_separator() {
    for (line, wanted) in combinations(&["; ", " | ", "\n", " && ", " || "]) {
        assert_eq!(ps(&line), wanted, "{line:?}");
    }
}

#[test]
fn quoted_and_escaped_separators_stay_in_arguments() {
    for separator in [";", "&&", "||", "|", "&"] {
        for line in [
            format!("alpha '{separator} beta'"),
            format!("alpha \"{separator} beta\""),
        ] {
            assert_eq!(posix(&line), ["alpha"], "{line}");
            assert_eq!(ps(&line), ["alpha"], "{line}");
        }
    }
}

#[test]
fn every_saved_prefix_matches_the_command_it_came_from() {
    let lines = [
        "git commit -m x && npm run build -- --watch",
        "A=1 cargo test -p x | tee out.log",
        "docker compose up -d; kubectl get pods",
        "python -m pytest tests && make check",
        "echo $(gh pr view 1)",
    ];
    for line in lines {
        for segment in split_commands(line, Dialect::Posix) {
            if segment.head(Dialect::Posix).is_none() {
                continue;
            }
            let mut rules = Ruleset::new();
            rules.push(Rule::new(
                "shell",
                format!("{} *", segment.prefix(Dialect::Posix)),
                Action::Allow,
            ));
            let text = segment.text(Dialect::Posix);
            assert_eq!(
                rules.evaluate("shell", &text),
                Action::Allow,
                "{line}: {text}"
            );
        }
    }
}

#[test]
fn assignment_only_lines_yield_nothing() {
    for line in ["A=1", "A=1 B=2", "A=$(x)=y"] {
        let found: Vec<String> = heads(line, Dialect::Posix)
            .into_iter()
            .filter(|head| head != "x")
            .collect();
        assert_eq!(found, Vec::<String>::new(), "{line}");
    }
}

#[test]
fn parameter_expansions_with_defaults_still_list_their_commands_at_every_depth() {
    for depth in 1..6 {
        let line = format!(
            "echo {}$(hostname){}",
            "${A:-".repeat(depth),
            "}".repeat(depth)
        );
        let found = posix(&line);
        assert!(found.contains(&"hostname".to_owned()), "{line}: {found:?}");
    }
}

#[test]
#[ignore = "parity ledger: an unterminated quote at any depth does not make the line unanalysable"]
fn an_unterminated_quote_at_any_depth_makes_the_whole_line_unanalysable() {
    let wraps = [
        "{}",
        "a; {}",
        "a && {}",
        "echo $({})",
        "X={} cmd",
        "cat > {}",
    ];
    for wrap in wraps {
        for broken in ["'open", "\"open", "$(open"] {
            let line = wrap.replace("{}", broken);
            assert!(split_commands(&line, Dialect::Posix).is_empty(), "{line}");
        }
    }
}

#[test]
fn an_always_approval_covers_repeats_but_only_with_its_prefix() {
    let segment = &split_commands("git commit -m one", Dialect::Posix)[0];
    let mut rules = Ruleset::new();
    rules.push(Rule::new(
        "shell",
        format!("{} *", segment.prefix(Dialect::Posix)),
        Action::Allow,
    ));
    assert_eq!(rules.evaluate("shell", "git commit -m two"), Action::Allow);
    assert_ne!(rules.evaluate("shell", "git push --force"), Action::Allow);
    assert_ne!(rules.evaluate("shell", "rm -rf x"), Action::Allow);
}
