//! Unit tests for frozen execution inputs and submitted evidence in
//! `crates/domain/evaluation/src/experiments/execution.rs`.

use std::collections::BTreeMap;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::{
    ArtifactEvidence, ClientCapabilities, ExecutionEvidence, ExecutionLimits, FrozenWorkspace,
};
use systemprompt_identifiers::{AiRequestId, EvalExecutionId};

fn digest_of(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn capabilities() -> ClientCapabilities {
    ClientCapabilities {
        client: ClientKind::Opencode,
        client_version: "1.2.3".to_owned(),
        adapter_version: "0.9.0".to_owned(),
        image_digest: digest_of('a'),
        supports_session_resume: true,
    }
}

fn evidence() -> ExecutionEvidence {
    ExecutionEvidence {
        execution_id: EvalExecutionId::generate(),
        fencing_token: 3,
        capabilities: capabilities(),
        installed_bundle_digest: digest_of('b'),
        candidate_bundle_digest: digest_of('c'),
        workspace_digest: digest_of('d'),
        requests: Vec::new(),
        artifacts: Vec::new(),
        exit_code: Some(0),
        elapsed_milliseconds: 42,
        cleanup_confirmed: true,
    }
}

#[test]
fn default_execution_limits_are_inside_the_supported_envelope() {
    let limits = ExecutionLimits::default();
    assert_eq!(limits.max_turns, 12);
    assert_eq!(limits.max_output_tokens, 4096);
    assert_eq!(limits.active_timeout_seconds, 1800);
    assert_eq!(limits.max_artifact_bytes, 16 * 1024 * 1024);
    limits.validate().expect("default limits are valid");
}

#[test]
fn execution_limits_reject_each_out_of_envelope_field() {
    let cases = [
        ExecutionLimits {
            max_turns: 0,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_turns: 101,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_output_tokens: 255,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_output_tokens: 32769,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            active_timeout_seconds: 0,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            active_timeout_seconds: 1801,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_artifact_bytes: 0,
            ..ExecutionLimits::default()
        },
        ExecutionLimits {
            max_artifact_bytes: 16 * 1024 * 1024 + 1,
            ..ExecutionLimits::default()
        },
    ];
    for limits in cases {
        let error = limits.validate().expect_err("out of envelope");
        assert!(
            error.to_string().contains("supported envelope"),
            "{limits:?} produced {error}"
        );
    }
    ExecutionLimits {
        max_turns: 1,
        max_output_tokens: 256,
        active_timeout_seconds: 1,
        max_artifact_bytes: 1,
    }
    .validate()
    .expect("lower bounds are inclusive");
}

#[test]
fn frozen_workspace_rejects_non_portable_paths() {
    for path in [
        "/absolute",
        "folder\\file",
        "C:file",
        "folder//file",
        "./file",
        "../escape",
        "",
    ] {
        let workspace = FrozenWorkspace {
            files: BTreeMap::from([(path.to_owned(), "body".to_owned())]),
        };
        let error = workspace.validate().expect_err("non-portable path");
        assert!(
            error.to_string().contains("portable relative paths"),
            "{path} produced {error}"
        );
    }
    let control = FrozenWorkspace {
        files: BTreeMap::from([("src/lib.rs\u{7}".to_owned(), "body".to_owned())]),
    };
    assert!(control.validate().is_err());
}

#[test]
fn frozen_workspace_rejects_oversized_manifests() {
    let too_many = FrozenWorkspace {
        files: (0..257)
            .map(|index| (format!("file-{index}"), String::new()))
            .collect(),
    };
    let error = too_many.validate().expect_err("file count");
    assert!(error.to_string().contains("256 files or 8 MiB"));

    let too_large = FrozenWorkspace {
        files: BTreeMap::from([("big".to_owned(), "x".repeat(8 * 1024 * 1024 + 1))]),
    };
    assert!(too_large.validate().is_err());
}

#[test]
fn frozen_workspace_digest_is_content_addressed_and_validates_first() {
    let workspace = FrozenWorkspace {
        files: BTreeMap::from([("src/main.rs".to_owned(), "fn main() {}".to_owned())]),
    };
    let digest = workspace.digest().expect("digest");
    assert_eq!(digest.len(), 64);
    assert_eq!(digest, workspace.digest().expect("stable digest"));

    let changed = FrozenWorkspace {
        files: BTreeMap::from([("src/main.rs".to_owned(), "fn main() { }".to_owned())]),
    };
    assert_ne!(digest, changed.digest().expect("digest"));

    let invalid = FrozenWorkspace {
        files: BTreeMap::from([("/etc/passwd".to_owned(), String::new())]),
    };
    assert!(invalid.digest().is_err());
}

#[test]
fn client_capabilities_require_a_digest_and_printable_versions() {
    capabilities().validate().expect("valid capabilities");

    let mut short_digest = capabilities();
    short_digest.image_digest = "abc".to_owned();
    assert!(
        short_digest
            .validate()
            .expect_err("short digest")
            .to_string()
            .contains("lowercase SHA-256")
    );

    let mut uppercase = capabilities();
    uppercase.image_digest = digest_of('A');
    assert!(uppercase.validate().is_err());

    let mut non_hex = capabilities();
    non_hex.image_digest = digest_of('g');
    assert!(non_hex.validate().is_err());

    for version in ["", "   ", "\u{1}", &"v".repeat(129)] {
        let mut blank_client = capabilities();
        blank_client.client_version = version.to_owned();
        assert!(
            blank_client
                .validate()
                .expect_err("bad client version")
                .to_string()
                .contains("1–128 printable bytes")
        );

        let mut blank_adapter = capabilities();
        blank_adapter.adapter_version = version.to_owned();
        assert!(blank_adapter.validate().is_err());
    }
}

#[test]
fn evidence_rejects_unfenced_leases_and_oversized_manifests() {
    evidence().validate().expect("baseline evidence");

    for token in [0, -1] {
        let mut unfenced = evidence();
        unfenced.fencing_token = token;
        assert!(
            unfenced
                .validate()
                .expect_err("unfenced")
                .to_string()
                .contains("lease or manifest size")
        );
    }

    let mut too_many_artifacts = evidence();
    too_many_artifacts.artifacts = (0..257)
        .map(|index| ArtifactEvidence {
            relative_path: format!("artifact-{index}"),
            sha256: digest_of('e'),
            bytes: 1,
        })
        .collect();
    assert!(too_many_artifacts.validate().is_err());

    let mut too_many_requests = evidence();
    too_many_requests.requests = (0..1001).map(|_| AiRequestId::generate()).collect();
    assert!(too_many_requests.validate().is_err());
}

#[test]
fn evidence_rejects_malformed_digests_in_every_position() {
    for mutate in [
        (|e: &mut ExecutionEvidence| e.installed_bundle_digest = "short".to_owned())
            as fn(&mut ExecutionEvidence),
        |e: &mut ExecutionEvidence| e.candidate_bundle_digest = digest_of('Z'),
        |e: &mut ExecutionEvidence| e.workspace_digest = String::new(),
        |e: &mut ExecutionEvidence| e.capabilities.image_digest = digest_of('x'),
    ] {
        let mut broken = evidence();
        mutate(&mut broken);
        assert!(
            broken
                .validate()
                .expect_err("bad digest")
                .to_string()
                .contains("lowercase SHA-256")
        );
    }
}

#[test]
fn evidence_rejects_duplicate_requests_and_artifacts() {
    let request = AiRequestId::generate();
    let mut duplicate_requests = evidence();
    duplicate_requests.requests = vec![request.clone(), request];
    assert!(
        duplicate_requests
            .validate()
            .expect_err("duplicate request")
            .to_string()
            .contains("Duplicate request evidence")
    );

    let artifact = ArtifactEvidence {
        relative_path: "out/report.md".to_owned(),
        sha256: digest_of('f'),
        bytes: 12,
    };
    let mut duplicate_artifacts = evidence();
    duplicate_artifacts.artifacts = vec![artifact.clone(), artifact.clone()];
    assert!(
        duplicate_artifacts
            .validate()
            .expect_err("duplicate artifact")
            .to_string()
            .contains("Duplicate artifact evidence")
    );

    let mut unique = evidence();
    unique.requests = vec![AiRequestId::generate(), AiRequestId::generate()];
    unique.artifacts = vec![
        artifact,
        ArtifactEvidence {
            relative_path: "out/other.md".to_owned(),
            sha256: digest_of('0'),
            bytes: 3,
        },
    ];
    unique.validate().expect("distinct evidence is accepted");
}

#[test]
fn evidence_rejects_artifacts_escaping_the_workspace() {
    let mut escaping = evidence();
    escaping.artifacts = vec![ArtifactEvidence {
        relative_path: "../secret".to_owned(),
        sha256: digest_of('f'),
        bytes: 1,
    }];
    assert!(
        escaping
            .validate()
            .expect_err("escaping artifact")
            .to_string()
            .contains("portable relative paths")
    );

    let mut bad_hash = evidence();
    bad_hash.artifacts = vec![ArtifactEvidence {
        relative_path: "out/report.md".to_owned(),
        sha256: "nope".to_owned(),
        bytes: 1,
    }];
    assert!(bad_hash.validate().is_err());
}
