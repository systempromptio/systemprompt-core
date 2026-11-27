use std::sync::atomic::{AtomicUsize, Ordering};

use sp_client::{ClientError, CredentialError, CredentialSource, Freshness, with_jwt};
use sp_protocol::Jwt;

struct Source {
    mints: AtomicUsize,
    rejections: AtomicUsize,
}

#[async_trait::async_trait]
impl CredentialSource for Source {
    async fn jwt(&self, freshness: Freshness) -> Result<Jwt, CredentialError> {
        if freshness == Freshness::Fresh {
            self.mints.fetch_add(1, Ordering::SeqCst);
            return Ok(Jwt::new("fresh.token.value").unwrap());
        }
        Ok(Jwt::new("held.token.value").unwrap())
    }

    async fn rejected(&self) {
        self.rejections.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn with_jwt_re_mints_once() {
    let source = Source {
        mints: AtomicUsize::new(0),
        rejections: AtomicUsize::new(0),
    };
    let value = with_jwt(&source, |jwt| async move {
        if jwt.expose() == "held.token.value" {
            Err(ClientError::Unauthorized {
                endpoint: "GET /v1/bridge/whoami",
                error_key: None,
            })
        } else {
            Ok(jwt.expose().to_owned())
        }
    })
    .await
    .unwrap();
    assert_eq!(value, "fresh.token.value");
    assert_eq!(source.mints.load(Ordering::SeqCst), 1);
    assert_eq!(source.rejections.load(Ordering::SeqCst), 0);

    let calls = AtomicUsize::new(0);
    let refused: Result<(), ClientError> = with_jwt(&source, |_jwt| {
        calls.fetch_add(1, Ordering::SeqCst);
        async {
            Err(ClientError::Unauthorized {
                endpoint: "GET /v1/bridge/whoami",
                error_key: None,
            })
        }
    })
    .await;
    assert!(refused.unwrap_err().is_unauthorized());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(source.rejections.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_source_that_cannot_hand_out_a_token_fails_the_call() {
    struct SignedOut;

    #[async_trait::async_trait]
    impl CredentialSource for SignedOut {
        async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
            Err(CredentialError::SignedOut)
        }

        async fn rejected(&self) {}
    }

    let calls = AtomicUsize::new(0);
    let result: Result<(), ClientError> = with_jwt(&SignedOut, |_jwt| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .await;
    assert!(matches!(
        result,
        Err(ClientError::Credential(CredentialError::SignedOut))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
