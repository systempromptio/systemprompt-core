use std::time::{Duration, SystemTime};

use harness_llm::retry::{MAX_DELAY, MAX_RETRY_AFTER, Retry, classify, delay, retry_after};
use harness_llm::{LlmError, TransportError};

fn status(status: u16, retry_after: Option<Duration>) -> LlmError {
    LlmError::Status {
        status,
        retry_after,
        body_excerpt: String::new(),
    }
}

#[test]
fn statuses_are_classified() {
    let seven = Duration::from_secs(7);
    assert_eq!(classify(&status(429, Some(seven))), Retry::After(seven));
    assert_eq!(classify(&status(429, None)), Retry::Backoff);
    assert_eq!(classify(&status(503, None)), Retry::Backoff);
    let hour = Some(Duration::from_secs(3_600));
    assert_eq!(classify(&status(529, hour)), Retry::After(MAX_RETRY_AFTER));
    assert_eq!(classify(&status(401, None)), Retry::No);
    assert_eq!(classify(&status(400, None)), Retry::No);
}

#[test]
fn other_errors_are_classified() {
    assert_eq!(classify(&LlmError::Aborted), Retry::No);
    let overflow = LlmError::ContextOverflow {
        status: Some(400),
        message: String::new(),
    };
    assert_eq!(classify(&overflow), Retry::No);
    assert_eq!(
        classify(&LlmError::Transport(TransportError::Timeout)),
        Retry::Backoff
    );
    assert_eq!(classify(&LlmError::Truncated), Retry::Backoff);
    let overloaded = LlmError::Provider {
        kind: Some("overloaded_error".into()),
        message: "Overloaded".into(),
    };
    assert_eq!(classify(&overloaded), Retry::Backoff);
    let invalid = LlmError::Provider {
        kind: Some("invalid_request_error".into()),
        message: "bad tool".into(),
    };
    assert_eq!(classify(&invalid), Retry::No);
}

#[test]
fn delay_doubles_up_to_the_cap() {
    assert_eq!(delay(0, None), Duration::from_secs(2));
    assert_eq!(delay(1, None), Duration::from_secs(2));
    assert_eq!(delay(2, None), Duration::from_secs(4));
    assert_eq!(delay(4, None), Duration::from_secs(16));
    assert_eq!(delay(5, None), MAX_DELAY);
    assert_eq!(delay(u32::MAX, None), MAX_DELAY);
    assert_eq!(
        delay(3, Some(Duration::from_millis(1_500))),
        Duration::from_millis(1_500)
    );
}

#[test]
fn retry_after_headers_are_read() {
    let h = |name: &str, value: &str| vec![(name.to_owned(), value.to_owned())];
    assert_eq!(
        retry_after(&h("Retry-After-Ms", "250")),
        Some(Duration::from_millis(250))
    );
    assert_eq!(
        retry_after(&h("retry-after", "3")),
        Some(Duration::from_secs(3))
    );
    assert_eq!(retry_after(&h("retry-after", "-1")), None);
    assert_eq!(
        retry_after(&h("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")),
        None
    );
    let future = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(120));
    let parsed = retry_after(&h("Retry-After", &future));
    assert!(parsed.is_some_and(|d| d > Duration::from_secs(60)));
    assert_eq!(retry_after(&[]), None);
}
