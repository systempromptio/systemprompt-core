use std::path::PathBuf;
use std::time::Duration;

use systemprompt_device::view::KeyFingerprint;
use systemprompt_engine::run::Reply;
use systemprompt_harness::command::{
    Command, DEFAULT_SYNC_TIMEOUT, LoginArgs, PermissionMode, PromptSource, RunArgs, UsageError,
    parse,
};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|argument| (*argument).to_owned()).collect()
}

fn run_args(list: &[&str]) -> RunArgs {
    match parse(args(list)).unwrap() {
        Command::Run(run) => *run,
        Command::Login(login) => panic!("expected run, got {login:?}"),
    }
}

fn refusal(list: &[&str]) -> UsageError {
    parse(args(list)).unwrap_err()
}

#[test]
fn run_with_the_least_it_needs_takes_the_defaults() {
    let run = run_args(&["run", "--workspace", "/work/project", "--prompt", "hello"]);
    assert_eq!(
        run,
        RunArgs {
            prompts: vec![PromptSource::Text("hello".to_owned())],
            workspace: PathBuf::from("/work/project"),
            permission_mode: PermissionMode::Ask,
            answers: None,
            model: None,
            agent: None,
            data_root: None,
            sync_timeout: DEFAULT_SYNC_TIMEOUT,
            timeout: None,
        }
    );
}

#[test]
fn run_reads_every_argument_in_either_spelling() {
    let run = run_args(&[
        "run",
        "--workspace=/work/project",
        "--permission-mode",
        "allow",
        "--answers=/tmp/answers.json",
        "--model",
        "model-a",
        "--agent=build",
        "--data-root",
        "/tmp/root",
        "--sync-timeout=5",
        "--timeout",
        "90",
        "--prompt",
        "first",
    ]);
    assert_eq!(run.permission_mode, PermissionMode::Allow);
    assert_eq!(run.answers, Some(PathBuf::from("/tmp/answers.json")));
    assert_eq!(run.model.as_deref(), Some("model-a"));
    assert_eq!(run.agent.as_deref(), Some("build"));
    assert_eq!(run.data_root, Some(PathBuf::from("/tmp/root")));
    assert_eq!(run.sync_timeout, Duration::from_secs(5));
    assert_eq!(run.timeout, Some(Duration::from_secs(90)));
}

#[test]
fn prompts_keep_the_order_they_were_given_in() {
    let run = run_args(&[
        "run",
        "--prompt",
        "first",
        "--workspace",
        "/w",
        "--prompt-file",
        "second.txt",
        "--prompt=third=3",
    ]);
    assert_eq!(
        run.prompts,
        [
            PromptSource::Text("first".to_owned()),
            PromptSource::File(PathBuf::from("second.txt")),
            PromptSource::Text("third=3".to_owned()),
        ]
    );
}

#[test]
fn a_prompt_may_look_like_an_argument() {
    let run = run_args(&["run", "--workspace", "/w", "--prompt", "--workspace=x"]);
    assert_eq!(
        run.prompts,
        [PromptSource::Text("--workspace=x".to_owned())]
    );
    assert_eq!(run.workspace, PathBuf::from("/w"));
}

#[test]
fn login_reads_its_instance_and_optional_fingerprint() {
    let parsed = parse(args(&[
        "login",
        "--stdin",
        "--instance",
        "https://instance.example",
        "--fingerprint=sha256:abc",
        "--data-root",
        "/tmp/root",
    ]))
    .unwrap();
    assert_eq!(
        parsed,
        Command::Login(LoginArgs {
            instance: "https://instance.example".to_owned(),
            fingerprint: Some(KeyFingerprint::new("sha256:abc")),
            data_root: Some(PathBuf::from("/tmp/root")),
        })
    );
}

#[test]
fn login_is_refused_without_stdin_or_an_instance() {
    assert!(matches!(
        refusal(&["login", "--instance", "https://instance.example"]),
        UsageError::Missing {
            command: "login",
            flag: "--stdin"
        }
    ));
    assert!(matches!(
        refusal(&["login", "--stdin"]),
        UsageError::Missing {
            command: "login",
            flag: "--instance"
        }
    ));
    assert!(matches!(
        refusal(&["login", "--stdin=yes", "--instance", "x"]),
        UsageError::UnexpectedValue { flag: "--stdin" }
    ));
}

#[test]
fn a_token_is_never_an_argument() {
    assert!(matches!(
        refusal(&["login", "--stdin", "--instance", "x", "--token", "sp-live-1"]),
        UsageError::UnknownArgument { command: "login", argument } if argument == "--token"
    ));
    assert!(matches!(
        refusal(&["login", "--stdin", "--instance", "x", "sp-live-1"]),
        UsageError::UnknownArgument { command: "login", argument } if argument == "sp-live-1"
    ));
}

#[test]
fn the_command_must_be_named_and_known() {
    assert!(matches!(refusal(&[]), UsageError::NoCommand));
    assert!(matches!(
        refusal(&["serve"]),
        UsageError::UnknownCommand { name } if name == "serve"
    ));
}

#[test]
fn run_refuses_what_it_cannot_use() {
    assert!(matches!(
        refusal(&["run", "--prompt", "hello"]),
        UsageError::Missing {
            command: "run",
            flag: "--workspace"
        }
    ));
    assert!(matches!(
        refusal(&["run", "--workspace", "/w"]),
        UsageError::Missing {
            command: "run",
            flag: "--prompt"
        }
    ));
    assert!(matches!(
        refusal(&["run", "--workspace", "/w", "--prompt"]),
        UsageError::MissingValue { flag: "--prompt" }
    ));
    assert!(matches!(
        refusal(&[
            "run",
            "--workspace",
            "/w",
            "--workspace",
            "/x",
            "--prompt",
            "p"
        ]),
        UsageError::Repeated {
            flag: "--workspace"
        }
    ));
    assert!(matches!(
        refusal(&["run", "--workspace", "/w", "--prompt", "p", "--stdin"]),
        UsageError::UnknownArgument { command: "run", argument } if argument == "--stdin"
    ));
}

#[test]
fn run_refuses_values_outside_their_range() {
    assert!(matches!(
        refusal(&["run", "--workspace", "/w", "--prompt", "p", "--permission-mode", "yolo"]),
        UsageError::Invalid { flag: "--permission-mode", value, .. } if value == "yolo"
    ));
    for value in ["0", "-1", "soon", "1.5"] {
        assert!(matches!(
            refusal(&[
                "run",
                "--workspace",
                "/w",
                "--prompt",
                "p",
                "--timeout",
                value
            ]),
            UsageError::Invalid {
                flag: "--timeout",
                ..
            }
        ));
    }
    assert!(matches!(
        refusal(&[
            "run",
            "--workspace",
            "/w",
            "--prompt",
            "p",
            "--sync-timeout",
            "0"
        ]),
        UsageError::Invalid {
            flag: "--sync-timeout",
            ..
        }
    ));
}

#[test]
fn load_reads_prompt_files_and_answers_and_makes_the_workspace_absolute() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("project");
    std::fs::create_dir(&workspace).unwrap();
    let prompt = dir.path().join("prompt.txt");
    std::fs::write(&prompt, "from a file\nsecond line\r\n\n").unwrap();
    let answers = dir.path().join("answers.json");
    std::fs::write(&answers, r#"{"permissions":[{"type":"always"}]}"#).unwrap();

    let request = run_args(&[
        "run",
        "--workspace",
        workspace.to_str().unwrap(),
        "--prompt",
        "typed",
        "--prompt-file",
        prompt.to_str().unwrap(),
        "--answers",
        answers.to_str().unwrap(),
        "--permission-mode",
        "deny",
    ])
    .load()
    .unwrap();
    assert_eq!(request.prompts, ["typed", "from a file\nsecond line"]);
    assert!(request.workspace.is_absolute());
    assert_eq!(request.workspace, workspace);
    assert_eq!(request.permission_mode, PermissionMode::Deny);
    assert_eq!(request.answers.permissions, [Reply::Always]);
}

#[test]
fn load_refuses_a_missing_workspace_prompt_file_or_a_bad_answers_file() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().to_str().unwrap().to_owned();
    let missing = dir.path().join("missing");
    let missing = missing.to_str().unwrap();

    assert!(matches!(
        run_args(&["run", "--workspace", missing, "--prompt", "p"]).load(),
        Err(UsageError::NotADirectory { .. })
    ));
    assert!(matches!(
        run_args(&["run", "--workspace", &workspace, "--prompt-file", missing]).load(),
        Err(UsageError::Unreadable { .. })
    ));
    let answers = dir.path().join("answers.json");
    std::fs::write(&answers, "{\"permissions\": 3}").unwrap();
    assert!(matches!(
        run_args(&[
            "run",
            "--workspace",
            &workspace,
            "--prompt",
            "p",
            "--answers",
            answers.to_str().unwrap()
        ])
        .load(),
        Err(UsageError::Answers { .. })
    ));
}
