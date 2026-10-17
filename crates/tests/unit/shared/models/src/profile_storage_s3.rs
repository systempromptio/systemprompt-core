use systemprompt_identifiers::SecretName;
use systemprompt_manifest::profile::{GcsCredentials, StorageBackend, StorageConfig};

use crate::profile_validation::{errors_of, valid_profile};

fn s3() -> StorageConfig {
    StorageConfig {
        backend: StorageBackend::S3,
        bucket: Some("files".to_owned()),
        credentials: Some(GcsCredentials::Secret(SecretName::new("S3_FILES"))),
        ..StorageConfig::default()
    }
}

#[test]
fn yaml_shape_parses_the_s3_block() {
    let cfg: StorageConfig = serde_yaml::from_str(
        "backend: s3\nbucket: dyn-files\nprefix: files\nendpoint: https://s3.example.internal\nregion: \
         eu-west-1\npath_style: true\ncredentials:\n  secret: S3_FILES\n",
    )
    .unwrap();
    assert_eq!(cfg.backend, StorageBackend::S3);
    assert_eq!(cfg.endpoint.as_deref(), Some("https://s3.example.internal"));
    assert_eq!(cfg.region.as_deref(), Some("eu-west-1"));
    assert!(cfg.path_style);
    assert_eq!(
        cfg.credentials,
        Some(GcsCredentials::Secret(SecretName::new("S3_FILES")))
    );
}

#[test]
fn a_complete_s3_block_validates() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        endpoint: Some("http://minio:9000".to_owned()),
        path_style: true,
        ..s3()
    };
    let errors = errors_of(&p);
    assert!(!errors.contains("storage."), "{errors}");
}

#[test]
fn s3_requires_a_secret_credential() {
    for credentials in [None, Some(GcsCredentials::WorkloadIdentity)] {
        let mut p = valid_profile();
        p.storage = StorageConfig {
            credentials,
            ..s3()
        };
        assert!(errors_of(&p).contains("requires storage.credentials: { secret: NAME }"));
    }
}

#[test]
fn s3_requires_a_bucket_and_rejects_shared() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        bucket: None,
        shared: true,
        ..s3()
    };
    let errors = errors_of(&p);
    assert!(errors.contains("storage.backend 's3' requires storage.bucket"));
    assert!(errors.contains("storage.shared applies to backend 'local' only"));
}

#[test]
fn s3_rejects_a_non_http_endpoint() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        endpoint: Some("s3.example.internal".to_owned()),
        ..s3()
    };
    assert!(errors_of(&p).contains("must be an http:// or https:// URL"));
}

#[test]
fn s3_only_keys_are_rejected_on_other_backends() {
    for backend in [StorageBackend::Local, StorageBackend::Gcs] {
        let mut p = valid_profile();
        p.storage = StorageConfig {
            backend,
            bucket: (backend == StorageBackend::Gcs).then(|| "b-1".to_owned()),
            endpoint: Some("https://s3.example.internal".to_owned()),
            region: Some("eu-west-1".to_owned()),
            path_style: true,
            ..StorageConfig::default()
        };
        let errors = errors_of(&p);
        for key in ["endpoint", "region", "path_style"] {
            assert!(
                errors.contains(&format!("storage.{key} applies to backend 's3' only")),
                "{backend:?} {key}: {errors}"
            );
        }
    }
}

#[test]
fn s3_dotted_or_underscored_bucket_requires_path_style() {
    for bucket in ["my.files", "my_files"] {
        let mut p = valid_profile();
        p.storage = StorageConfig {
            bucket: Some(bucket.to_owned()),
            ..s3()
        };
        assert!(
            errors_of(&p).contains("set storage.path_style: true"),
            "{bucket}"
        );
        p.storage.path_style = true;
        assert!(!errors_of(&p).contains("path_style"), "{bucket}");
    }
}
