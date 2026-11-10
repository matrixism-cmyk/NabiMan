use super::*;
use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in pairs {
        map.append(HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap());
    }
    map
}

#[test]
fn requests_are_counted_by_route_and_status_with_a_cumulative_histogram() {
    record_request("/api/__metrics_test/{id}", 200, Duration::from_millis(3));
    record_request("/api/__metrics_test/{id}", 200, Duration::from_millis(300));
    record_request("/api/__metrics_test/{id}", 404, Duration::from_secs(60));
    let text = render(&Gauges { terminal_sessions: 4, backup_last_success: Some(1_700_000_000) });
    assert!(text.contains("nabiman_http_requests_total{route=\"/api/__metrics_test/{id}\",status=\"200\"} 2"), "{text}");
    assert!(text.contains("nabiman_http_requests_total{route=\"/api/__metrics_test/{id}\",status=\"404\"} 1"));
    let bucket = |le: &str| -> u64 {
        let prefix = format!("nabiman_http_request_duration_seconds_bucket{{le=\"{le}\"}} ");
        text.lines().find_map(|l| l.strip_prefix(&prefix)).unwrap().parse().unwrap()
    };
    // Other tests count too; the buckets only ever grow from the smaller to the larger.
    assert!(bucket("0.005") >= 1 && bucket("0.5") >= bucket("0.005") + 1 && bucket("+Inf") >= bucket("30") + 1);
    assert!(text.contains("nabiman_terminal_sessions 4"));
    assert!(text.contains("nabiman_backup_last_success_timestamp_seconds 1700000000"));
    for name in [
        "nabiman_websocket_connections", "nabiman_child_process_timeouts_total", "nabiman_audit_write_failures_total",
        "nabiman_login_failures_total", "nabiman_rate_limited_total",
    ] {
        assert!(text.contains(&format!("# TYPE {name} ")), "{name}");
    }
}

#[test]
fn refusals_count_as_login_failures_and_429s_as_rate_limited() {
    let (logins, limited) = (LOGIN_FAILURES.load(Relaxed), RATE_LIMITED.load(Relaxed));
    record_request("/api/auth/login", 401, Duration::ZERO);
    record_request("/api/auth/login", 429, Duration::ZERO);
    record_request("/api/auth/login", 200, Duration::ZERO);
    assert!(LOGIN_FAILURES.load(Relaxed) > logins);
    assert!(RATE_LIMITED.load(Relaxed) > limited);
}

#[test]
fn a_websocket_is_open_while_its_guard_lives() {
    let opened = WS_OPENED.load(Relaxed);
    let guard = WsOpen::begin();
    assert!(WS_OPEN.load(Relaxed) >= 1);
    drop(guard);
    assert!(WS_OPENED.load(Relaxed) > opened);
}

#[test]
fn the_scrape_token_opens_metrics_only_when_set_and_exact() {
    let set = |name: &str| (name == "NABIMAN_METRICS_TOKEN").then(|| "scrape-0123456789abcdef".to_string());
    let unset = |_: &str| None;
    let good = headers(&[("Authorization", "Bearer scrape-0123456789abcdef")]);
    assert!(scrape_token_in(&good, &set));
    assert!(!scrape_token_in(&good, &unset), "no setting, no token access");
    assert!(!scrape_token_in(&headers(&[("Authorization", "Bearer scrape-0123456789abcdeX")]), &set));
    assert!(!scrape_token_in(&headers(&[("Authorization", "scrape-0123456789abcdef")]), &set));
    assert!(!scrape_token_in(&headers(&[]), &set));
    let twice = headers(&[("Authorization", "Bearer scrape-0123456789abcdef"), ("Authorization", "Bearer x")]);
    assert!(!scrape_token_in(&twice, &set));
}
