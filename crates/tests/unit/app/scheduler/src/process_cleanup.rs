//! Tests for ProcessCleanup primitives and its ProcessCleanupProvider adapter.
//!
//! Focuses on safety guards (protected ports/processes), pure return-value
//! paths (lookups against PIDs/ports that do not exist), and the provider
//! trait adapter that exposes the same primitives behind a stable contract.

use systemprompt_scheduler::ProcessCleanup;
use systemprompt_traits::ProcessCleanupProvider;

const NONEXISTENT_PID: u32 = 0xFFFF_FFFF;
const POSTGRES_PORT: u16 = 5432;
const PGBOUNCER_PORT: u16 = 6432;
const UNLIKELY_PORT: u16 = 1;

#[test]
fn check_port_returns_none_for_protected_postgres() {
    assert!(ProcessCleanup::check_port(POSTGRES_PORT).is_none());
}

#[test]
fn check_port_returns_none_for_protected_pgbouncer() {
    assert!(ProcessCleanup::check_port(PGBOUNCER_PORT).is_none());
}

#[test]
fn check_port_returns_none_for_unbound_port() {
    assert!(ProcessCleanup::check_port(UNLIKELY_PORT).is_none());
}

#[test]
fn kill_port_protected_returns_empty() {
    assert!(ProcessCleanup::kill_port(POSTGRES_PORT).is_empty());
    assert!(ProcessCleanup::kill_port(PGBOUNCER_PORT).is_empty());
}

#[test]
fn kill_port_unbound_returns_empty() {
    assert!(ProcessCleanup::kill_port(UNLIKELY_PORT).is_empty());
}

#[test]
fn kill_by_pattern_rejects_protected_postgres() {
    assert_eq!(ProcessCleanup::kill_by_pattern("postgres"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("pgbouncer"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("psql"), 0);
}

#[test]
fn kill_by_pattern_rejects_pattern_containing_protected_substring() {
    assert_eq!(ProcessCleanup::kill_by_pattern("my-postgres-runner"), 0);
}

#[test]
fn kill_by_pattern_rejects_unsafe_characters() {
    assert_eq!(ProcessCleanup::kill_by_pattern("foo; rm -rf /"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern("foo$bar"), 0);
    assert_eq!(ProcessCleanup::kill_by_pattern(""), 0);
}

#[test]
fn kill_by_pattern_accepts_safe_nonexistent_pattern() {
    // Safe pattern that should never match any real process; pkill returns
    // non-zero status which the helper maps to 0.
    let unique = format!("systemprompt-test-no-match-{}", std::process::id());
    assert_eq!(ProcessCleanup::kill_by_pattern(&unique), 0);
}

#[test]
fn process_exists_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::process_exists(NONEXISTENT_PID));
}

#[test]
fn process_exists_true_for_current_pid() {
    assert!(ProcessCleanup::process_exists(std::process::id()));
}

#[test]
fn kill_process_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::kill_process(NONEXISTENT_PID));
}

#[test]
fn get_process_by_port_returns_none_for_unbound_port() {
    assert!(ProcessCleanup::get_process_by_port(UNLIKELY_PORT).is_none());
}

#[tokio::test]
async fn wait_for_port_free_returns_ok_when_unbound() {
    let result = ProcessCleanup::wait_for_port_free(UNLIKELY_PORT, 1, 1).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn wait_for_port_free_returns_ok_for_protected_port() {
    // Protected ports are reported as "not occupied" by check_port, so the
    // wait loop exits with Ok on the first iteration.
    let result = ProcessCleanup::wait_for_port_free(POSTGRES_PORT, 2, 1).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn terminate_gracefully_false_for_nonexistent_pid() {
    assert!(!ProcessCleanup::terminate_gracefully(NONEXISTENT_PID, 1).await);
}

mod provider_adapter {
    use super::{
        NONEXISTENT_PID, POSTGRES_PORT, ProcessCleanup, ProcessCleanupProvider, UNLIKELY_PORT,
    };

    fn provider() -> Arc<dyn ProcessCleanupProvider> {
        Arc::new(ProcessCleanup)
    }

    use std::sync::Arc;

    #[test]
    fn provider_process_exists_matches_inherent() {
        let p = provider();
        assert!(!p.process_exists(NONEXISTENT_PID));
        assert!(p.process_exists(std::process::id()));
    }

    #[test]
    fn provider_check_port_matches_inherent() {
        let p = provider();
        assert!(p.check_port(POSTGRES_PORT).is_none());
        assert!(p.check_port(UNLIKELY_PORT).is_none());
    }

    #[test]
    fn provider_kill_process_false_for_nonexistent_pid() {
        let p = provider();
        assert!(!p.kill_process(NONEXISTENT_PID));
    }

    #[tokio::test]
    async fn provider_terminate_gracefully_false_for_nonexistent_pid() {
        let p = provider();
        assert!(!p.terminate_gracefully(NONEXISTENT_PID, 1).await);
    }

    #[tokio::test]
    async fn provider_wait_for_port_free_returns_ok_when_unbound() {
        let p = provider();
        assert!(p.wait_for_port_free(UNLIKELY_PORT, 1, 1).await.is_ok());
    }
}
