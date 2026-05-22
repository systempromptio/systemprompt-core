use systemprompt_models::net::{OutboundUrlError, validate_outbound_url};

mod validate_outbound_url_tests {
    use super::*;

    #[test]
    fn accepts_https() {
        let url = validate_outbound_url("https://example.com/hook").expect("https allowed");
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn accepts_loopback_http() {
        assert!(validate_outbound_url("http://localhost:8080/h").is_ok());
        assert!(validate_outbound_url("http://127.0.0.1/h").is_ok());
        assert!(validate_outbound_url("http://[::1]/h").is_ok());
    }

    #[test]
    fn rejects_cloud_metadata_ip() {
        assert!(matches!(
            validate_outbound_url("https://169.254.169.254/latest/meta-data"),
            Err(OutboundUrlError::BlockedHost(_))
        ));
    }

    #[test]
    fn rejects_rfc1918_ranges() {
        for url in [
            "https://10.0.0.5/h",
            "https://192.168.1.1/h",
            "https://172.20.0.1/h",
        ] {
            assert!(
                matches!(validate_outbound_url(url), Err(OutboundUrlError::BlockedHost(_))),
                "{url} should be blocked",
            );
        }
    }

    #[test]
    fn allows_172_outside_private_block() {
        assert!(validate_outbound_url("https://172.32.0.1/h").is_ok());
        assert!(validate_outbound_url("https://172.15.0.1/h").is_ok());
    }

    #[test]
    fn rejects_non_loopback_http() {
        assert!(matches!(
            validate_outbound_url("http://example.com/h"),
            Err(OutboundUrlError::NonLoopbackHttp)
        ));
    }

    #[test]
    fn rejects_non_http_scheme() {
        assert!(matches!(
            validate_outbound_url("ftp://example.com/h"),
            Err(OutboundUrlError::Scheme(_))
        ));
    }

    #[test]
    fn rejects_malformed_url() {
        assert!(matches!(
            validate_outbound_url("not a url"),
            Err(OutboundUrlError::Parse(_))
        ));
    }
}
