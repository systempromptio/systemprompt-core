use std::net::{IpAddr, Ipv6Addr};
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, HeaderValue, header};
use systemprompt_api::services::middleware::BodyBudget;
use systemprompt_api::services::middleware::rate_limit::{KeyedLimiter, client_bucket};

const EMISSION: Duration = Duration::from_millis(100);

#[test]
fn idle_keys_are_evicted_once_their_budget_has_refilled() {
    let limiter = KeyedLimiter::new(EMISSION, 2, 10_000);
    let start = Instant::now();
    for n in 0..500u32 {
        limiter
            .check(&format!("ip:10.0.{}.{}", n / 256, n % 256), start)
            .expect("a first request is admitted");
    }
    assert_eq!(limiter.tracked_keys(), 500);

    limiter.evict_idle(start + EMISSION);

    assert_eq!(limiter.tracked_keys(), 0);
}

#[test]
fn a_key_still_inside_its_window_survives_eviction() {
    let limiter = KeyedLimiter::new(EMISSION, 2, 10_000);
    let start = Instant::now();
    limiter.check("ip:10.0.0.1", start).expect("first");
    limiter
        .check("ip:10.0.0.1", start)
        .expect("second, within burst");

    let later = start + EMISSION / 2;
    limiter.evict_idle(later);

    assert_eq!(limiter.tracked_keys(), 1);
    assert!(
        limiter.check("ip:10.0.0.1", later).is_err(),
        "eviction must not refill a key that is still spending its budget"
    );
}

#[test]
fn the_tracked_key_map_never_exceeds_its_cap() {
    let cap = 64;
    let limiter = KeyedLimiter::new(EMISSION, 1_000, cap);
    let now = Instant::now();
    for n in 0..10_000u32 {
        let _ = limiter.check(
            &format!("ip:10.{}.{}.{}", n >> 16, (n >> 8) & 255, n & 255),
            now,
        );
        assert!(limiter.tracked_keys() <= cap);
    }
}

#[test]
fn a_flood_of_new_keys_at_the_cap_cannot_lock_out_a_tracked_client() {
    let cap = 8;
    let limiter = KeyedLimiter::new(EMISSION, 3, cap);
    let now = Instant::now();
    limiter.check("u:alice", now).expect("alice is tracked");
    for n in 0..1_000u32 {
        let _ = limiter.check(&format!("ip:203.0.113.{}", n % 250), now);
        let _ = limiter.check(&format!("ip:198.51.{}.{}", n / 250, n % 250), now);
    }

    assert!(
        limiter.check("u:alice", now).is_ok(),
        "alice keeps her own budget"
    );
    assert!(limiter.tracked_keys() <= cap);
}

#[test]
fn a_thousand_addresses_in_one_ipv6_slash_64_share_one_budget() {
    let limiter = KeyedLimiter::new(EMISSION, 5, 10_000);
    let now = Instant::now();
    let mut admitted = 0;
    for n in 0..1_000u16 {
        let addr = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0xabcd, 0x12, n, n, 0, 1));
        if limiter.check(&client_bucket(addr), now).is_ok() {
            admitted += 1;
        }
    }

    assert_eq!(admitted, 5, "one /64 gets one burst");
    assert_eq!(limiter.tracked_keys(), 1);
}

#[test]
fn ipv4_is_keyed_by_address_and_a_mapped_address_by_the_ipv4_it_carries() {
    let v4: IpAddr = "198.51.100.7".parse().expect("ip");
    let mapped: IpAddr = "::ffff:198.51.100.7".parse().expect("ip");
    let other: IpAddr = "198.51.100.8".parse().expect("ip");

    assert_eq!(client_bucket(v4), client_bucket(mapped));
    assert_ne!(client_bucket(v4), client_bucket(other));
    assert_ne!(
        client_bucket("2001:db8:1:2::1".parse().expect("ip")),
        client_bucket("2001:db8:1:3::1".parse().expect("ip")),
        "different /64s are different clients"
    );
}

#[tokio::test]
async fn the_sweeper_evicts_idle_keys_and_stops_with_its_limiter() {
    let limiter = std::sync::Arc::new(KeyedLimiter::new(Duration::from_millis(1), 1, 10_000));
    for n in 0..100u32 {
        limiter
            .check(&format!("ip:10.1.0.{n}"), Instant::now())
            .expect("first request");
    }
    let sweeper = limiter.spawn_sweeper(Duration::from_millis(20));
    for _ in 0..100 {
        if limiter.tracked_keys() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(limiter.tracked_keys(), 0);

    drop(limiter);
    for _ in 0..100 {
        if sweeper.is_finished() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        sweeper.is_finished(),
        "the sweeper ends once its limiter is gone"
    );
}

fn declaring(length: usize) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).expect("header"),
    );
    headers
}

#[test]
fn the_body_budget_refuses_once_reserved_bytes_reach_it() {
    let mib = 1024 * 1024;
    let budget = BodyBudget::new(64 * mib);

    let first = budget.reserve(&declaring(40 * mib), 100 * mib);
    let second = budget.reserve(&declaring(30 * mib), 100 * mib);
    assert!(first.is_some());
    assert!(second.is_none(), "40 + 30 MiB exceeds a 64 MiB budget");

    drop(first);
    assert!(budget.reserve(&declaring(30 * mib), 100 * mib).is_some());
}

#[test]
fn a_body_without_a_declared_length_reserves_the_full_route_limit() {
    let mib = 1024 * 1024;
    let budget = BodyBudget::new(64 * mib);

    let chunked = budget.reserve(&HeaderMap::new(), 48 * mib);
    assert!(chunked.is_some());
    assert_eq!(budget.available_bytes(), 16 * mib);
}
