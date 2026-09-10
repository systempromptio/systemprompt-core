use systemprompt_config::{VaultError, VaultKvProvider};
use systemprompt_models::profile::SecretsConfig;

use crate::vault_fixture as fx;

#[test]
fn a_plaintext_non_loopback_address_is_refused_without_a_trust_entry() {
    fx::remove_env("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS");

    let cfg = fx::config("http://vault:8200", fx::token_auth());
    let err = VaultKvProvider::from_config(&cfg, |_name| None).unwrap_err();

    assert!(matches!(err, VaultError::Address { .. }));
}

#[test]
fn a_plaintext_address_is_accepted_once_the_host_is_trusted() {
    fx::set_env("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS", "vault");

    let cfg = fx::config("http://vault:8200", fx::token_auth());
    let provider = VaultKvProvider::from_config(&cfg, |_name| None);

    fx::remove_env("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS");
    assert!(provider.is_ok());
}

#[test]
fn there_is_no_tls_verification_escape_hatch_in_the_profile() {
    let yaml = r"source: vault
vault:
  address: https://vault.example.com
  path: systemprompt/prod
  skip_verify: true
  auth:
    method: token
";

    let err = serde_yaml::from_str::<SecretsConfig>(yaml).unwrap_err();
    assert!(err.to_string().contains("skip_verify"));
}
