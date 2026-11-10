use super::*;

const TS: &str = "2026-11-11T00:00:00.000Z";

fn text_line(message: &str, fields: &[(&str, String)]) -> String {
    format_line(TS, Level::Warn, "test", message, fields, Format::Text)
}

#[test]
fn levels_filter_overall_and_per_target() {
    let f = Filter::parse("");
    assert!(f.allows(Level::Info, "http") && !f.allows(Level::Debug, "http"));
    let f = Filter::parse("warn,mec=error,http=debug");
    assert!(!f.allows(Level::Info, "terminal"));
    assert!(f.allows(Level::Warn, "terminal"));
    assert!(!f.allows(Level::Warn, "mec") && !f.allows(Level::Warn, "mec::kube"));
    assert!(f.allows(Level::Error, "mec"));
    assert!(f.allows(Level::Debug, "http"));
    assert!(f.allows(Level::Warn, "mecca"), "a prefix that is not a parent target does not match");
    let f = Filter::parse("off,audit=warn,bogus");
    assert!(!f.allows(Level::Error, "http") && f.allows(Level::Warn, "audit"));
}

#[test]
fn text_and_json_lines_carry_level_target_message_and_fields() {
    let fields = [("route", "/api/users/{id}".to_string()), ("status", "200".into()), ("note", "two words".into())];
    let line = format_line(TS, Level::Info, "http", "request", &fields, Format::Text);
    assert_eq!(line, r#"2026-11-11T00:00:00.000Z INFO  http: request route=/api/users/{id} status=200 note="two words""#);
    let json: serde_json::Value = serde_json::from_str(&format_line(TS, Level::Info, "http", "request", &fields, Format::Json)).unwrap();
    assert_eq!(json["level"], "info");
    assert_eq!(json["target"], "http");
    assert_eq!(json["msg"], "request");
    assert_eq!(json["route"], "/api/users/{id}");
    assert_eq!(json["note"], "two words");
    let clash = format_line(TS, Level::Info, "x", "m", &[("level", "oops".into())], Format::Json);
    let clash: serde_json::Value = serde_json::from_str(&clash).unwrap();
    assert_eq!((clash["level"].as_str(), clash["field_level"].as_str()), (Some("info"), Some("oops")));
}

#[test]
fn a_newline_in_a_message_cannot_forge_a_line() {
    let line = text_line("bad\n2026-11-11T00:00:00.000Z ERROR  auth: forged", &[("v", "a\nb".into())]);
    assert_eq!(line.lines().count(), 1, "{line}");
}

#[test]
fn secrets_never_reach_a_line() {
    let secrets = ["hunter2-pass", "s3cr3t-tok", "nbm_abcdef0123456789", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.c2lnbmF0dXJl", "dbpass99", "rancher-token-x"];
    let events: Vec<(&str, Vec<(&str, String)>)> = vec![
        ("login failed for alice password=hunter2-pass", vec![]),
        ("upstream said: {\"user\":\"a\",\"token\":\"s3cr3t-tok\"}", vec![]),
        ("request with Authorization: Bearer nbm_abcdef0123456789 refused", vec![]),
        ("cookie jar", vec![("session_token", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.c2lnbmF0dXJl".into())]),
        ("cannot reach mysql://root:dbpass99@db.local:3306/x", vec![("url", "https://h/x?a=1&api_key=s3cr3t-tok".into())]),
        ("rancher configured", vec![("password", "hunter2-pass".into()), ("detail", "token: rancher-token-x".into())]),
        ("handing over eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhIn0.c2lnbmF0dXJl now", vec![("error", "bad key nbm_abcdef0123456789".into())]),
    ];
    for (message, fields) in &events {
        for format in [Format::Text, Format::Json] {
            let line = format_line(TS, Level::Error, "test", message, fields, format);
            for secret in secrets {
                assert!(!line.contains(secret), "{secret} leaked: {line}");
            }
            assert!(line.contains("[redacted]"), "{line}");
        }
    }
    // What is not a secret stays readable.
    let plain = text_line("dashboard poll failed: nodes is forbidden: User \"system:serviceaccount:nabi:default\"", &[("user", "alice".into())]);
    assert!(plain.contains("system:serviceaccount:nabi:default") && plain.contains("user=alice"), "{plain}");
    assert!(text_line("cannot reach https://example.com/api?page=2", &[]).contains("page=2"));
}
