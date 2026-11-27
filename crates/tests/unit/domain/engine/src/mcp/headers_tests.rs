use std::sync::{Arc, Mutex};

use systemprompt_engine::mcp::{BoxError, HeaderSource, McpError, RemoteHeaders};

struct Rotating(Mutex<u32>);

#[async_trait::async_trait]
impl HeaderSource for Rotating {
    async fn headers(&self) -> Result<Vec<(String, String)>, BoxError> {
        let n = {
            let mut n = self.0.lock().unwrap();
            *n += 1;
            *n
        };
        Ok(vec![("Authorization".to_owned(), format!("Bearer t{n}"))])
    }
}

struct Refusing;

#[async_trait::async_trait]
impl HeaderSource for Refusing {
    async fn headers(&self) -> Result<Vec<(String, String)>, BoxError> {
        Err("credential store locked".into())
    }
}

#[tokio::test]
async fn a_source_is_read_on_every_resolve() {
    let headers = RemoteHeaders::Source(Arc::new(Rotating(Mutex::new(0))));
    let first = headers.resolve().await.unwrap();
    let second = headers.resolve().await.unwrap();
    assert_eq!(first.get("authorization").unwrap(), "Bearer t1");
    assert_eq!(second.get("authorization").unwrap(), "Bearer t2");
    assert!(second.get("authorization").unwrap().is_sensitive());
}

#[tokio::test]
async fn static_headers_resolve_as_given_and_are_sensitive() {
    let headers = RemoteHeaders::Static(vec![("x-api-key".into(), "k1".into())]);
    let map = headers.resolve().await.unwrap();
    assert_eq!(map.get("x-api-key").unwrap(), "k1");
    assert!(map.get("x-api-key").unwrap().is_sensitive());
    assert_eq!(map.len(), 1);
}

#[test]
fn debug_hides_values() {
    let headers = RemoteHeaders::Static(vec![("x-api-key".into(), "hunter2".into())]);
    let shown = format!("{headers:?}");
    assert!(shown.contains("x-api-key"));
    assert!(!shown.contains("hunter2"));
    let source = format!("{:?}", RemoteHeaders::Source(Arc::new(Refusing)));
    assert_eq!(source, "Source(..)");
}

#[tokio::test]
async fn an_invalid_header_names_only_the_header() {
    let headers = RemoteHeaders::Static(vec![("bad name".into(), "secret\n".into())]);
    let error = headers.resolve().await.unwrap_err();
    assert!(matches!(error, McpError::Header { ref name } if name == "bad name"));
    assert!(!error.to_string().contains("secret"));
}

#[tokio::test]
async fn an_invalid_value_is_refused_without_echoing_it() {
    let headers = RemoteHeaders::Static(vec![("authorization".into(), "Bearer a\nb".into())]);
    let error = headers.resolve().await.unwrap_err();
    assert!(matches!(error, McpError::Header { ref name } if name == "authorization"));
    assert!(!error.to_string().contains("Bearer"));
}

#[tokio::test]
async fn a_failing_source_fails_the_resolve() {
    let headers = RemoteHeaders::Source(Arc::new(Refusing));
    let error = headers.resolve().await.unwrap_err();
    assert!(matches!(error, McpError::HeaderSource(_)));
}
