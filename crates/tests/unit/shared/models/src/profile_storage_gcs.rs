use systemprompt_identifiers::SecretName;
use systemprompt_manifest::profile::{GcsCredentials, StorageBackend, StorageConfig};

use crate::profile_validation::{errors_of, valid_profile};

fn gcs(bucket: Option<&str>) -> StorageConfig {
    StorageConfig {
        backend: StorageBackend::Gcs,
        bucket: bucket.map(str::to_owned),
        ..StorageConfig::default()
    }
}

#[test]
fn yaml_shape_parses_both_credential_forms() {
    let cfg: StorageConfig = serde_yaml::from_str(
        "backend: gcs\nbucket: my-bucket\nprefix: tenant-a/files\npublic_read: true\ncredentials: \
         workload_identity\n",
    )
    .unwrap();
    assert_eq!(cfg.backend, StorageBackend::Gcs);
    assert_eq!(cfg.bucket.as_deref(), Some("my-bucket"));
    assert_eq!(cfg.prefix.as_deref(), Some("tenant-a/files"));
    assert!(cfg.public_read);
    assert_eq!(cfg.credentials, Some(GcsCredentials::WorkloadIdentity));

    let cfg: StorageConfig = serde_yaml::from_str(
        "backend: gcs\nbucket: b-1\ncredentials:\n  secret: GCS_SERVICE_ACCOUNT_KEY\n",
    )
    .unwrap();
    assert_eq!(
        cfg.credentials,
        Some(GcsCredentials::Secret(SecretName::new(
            "GCS_SERVICE_ACCOUNT_KEY"
        )))
    );
}

#[test]
fn a_well_formed_gcs_section_validates() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        prefix: Some("tenant-a/files".to_owned()),
        public_read: true,
        credentials: Some(GcsCredentials::Secret(SecretName::new("KEY"))),
        ..gcs(Some("my.bucket_1-a"))
    };
    assert!(p.validate().is_ok(), "{}", errors_of(&p));
}

#[test]
fn gcs_requires_a_bucket() {
    for bucket in [None, Some("")] {
        let mut p = valid_profile();
        p.storage = gcs(bucket);
        assert!(
            errors_of(&p).contains("requires storage.bucket"),
            "{bucket:?}"
        );
    }
}

#[test]
fn malformed_bucket_names_are_rejected() {
    for bucket in [
        "ab",
        "My-Bucket",
        "-bucket",
        "bucket-",
        "bu/cket",
        &"a".repeat(223),
    ] {
        let mut p = valid_profile();
        p.storage = gcs(Some(bucket));
        assert!(
            errors_of(&p).contains("not a valid Cloud Storage bucket name"),
            "{bucket}"
        );
    }
}

#[test]
fn malformed_prefixes_are_rejected() {
    for prefix in ["", "/lead", "trail/", "a//b", "a/../b", "./a"] {
        let mut p = valid_profile();
        p.storage = StorageConfig {
            prefix: Some(prefix.to_owned()),
            ..gcs(Some("bucket"))
        };
        assert!(errors_of(&p).contains("storage.prefix"), "{prefix:?}");
    }
}

#[test]
fn gcs_refuses_shared() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        shared: true,
        ..gcs(Some("bucket"))
    };
    assert!(errors_of(&p).contains("storage.shared applies to backend 'local' only"));
}

#[test]
fn gcs_refuses_an_empty_secret_name() {
    let mut p = valid_profile();
    p.storage = StorageConfig {
        credentials: Some(GcsCredentials::Secret(SecretName::new(" "))),
        ..gcs(Some("bucket"))
    };
    assert!(errors_of(&p).contains("storage.credentials.secret"));
}

#[test]
fn local_refuses_every_gcs_key() {
    let cases: [(&str, StorageConfig); 4] = [
        (
            "bucket",
            StorageConfig {
                bucket: Some("b".to_owned()),
                ..StorageConfig::default()
            },
        ),
        (
            "prefix",
            StorageConfig {
                prefix: Some("p".to_owned()),
                ..StorageConfig::default()
            },
        ),
        (
            "public_read",
            StorageConfig {
                public_read: true,
                ..StorageConfig::default()
            },
        ),
        (
            "credentials",
            StorageConfig {
                credentials: Some(GcsCredentials::WorkloadIdentity),
                ..StorageConfig::default()
            },
        ),
    ];
    for (key, storage) in cases {
        let mut p = valid_profile();
        p.storage = storage;
        assert!(
            errors_of(&p).contains(&format!("storage.{key} applies to backend 'gcs' only")),
            "{key}"
        );
    }
}
