use super::*;
use crate::models::NabimanUser;

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
const FILLS: [&str; 5] = ["x", "services", "me", "share", "cancel"];

fn filled(pattern: &str, fill: &str) -> String {
    pattern.split('/').map(|s| if variable(s) { fill } else { s }).collect::<Vec<_>>().join("/")
}

#[actix_web::test]
async fn every_registered_route_is_classified_and_every_listed_one_exists() {
    let registered = inventory::api_patterns().await;
    let listed: std::collections::BTreeSet<&str> = all().map(|r| r.pattern).collect();
    let missing: Vec<&String> = registered.iter().filter(|p| !listed.contains(p.as_str())).collect();
    assert!(
        missing.is_empty(),
        "Routes with no access class (add them to backend/src/route_access/, then regenerate the doc): {missing:#?}"
    );
    let stale: Vec<&&str> = listed.iter().filter(|p| !registered.iter().any(|r| r == **p)).collect();
    assert!(stale.is_empty(), "Listed routes that are not registered any more: {stale:#?}");
    assert_eq!(listed.len(), all().count(), "a route is listed twice");
}

#[test]
fn the_inventory_reader_follows_nested_scopes() {
    // The shape actix prints, cut down: a scope with two leaves and a named map.
    let debug = r#"ResourceMap { pattern: ResourceDef { id: 0, name: None, patterns: Single(""), is_prefix: true, pat_type: Static(""), segments: [] }, named: {}, parent: RefCell { value: (Weak) }, nodes: Some([ResourceMap { pattern: ResourceDef { id: 1, name: None, patterns: Single("/api/x"), is_prefix: true, pat_type: Static("/api/x"), segments: [Const("/api/x")] }, named: {"n": ResourceMap { pattern: ResourceDef { id: 9, name: Some("n"), patterns: Single("/dup"), is_prefix: false, pat_type: Static("/dup"), segments: [] }, named: {}, parent: RefCell { value: (Weak) }, nodes: None }}, parent: RefCell { value: (Weak) }, nodes: Some([ResourceMap { pattern: ResourceDef { id: 2, name: None, patterns: Single("/{id}"), is_prefix: false, pat_type: Dynamic(Regex("^/(?P<id>[^/]+)$"), ["id"]), segments: [Var("id")] }, named: {}, parent: RefCell { value: (Weak) }, nodes: None }, ResourceMap { pattern: ResourceDef { id: 3, name: None, patterns: List(["/a", "/b"]), is_prefix: false, pat_type: Static("/a"), segments: [] }, named: {}, parent: RefCell { value: (Weak) }, nodes: None }]) }]) }"#;
    assert_eq!(inventory::leaves(debug), ["/api/x/{id}", "/api/x/a", "/api/x/b"]);
}

#[test]
fn operators_and_viewers_keep_exactly_what_they_had() {
    for route in all().filter(|r| r.class != Class::Public) {
        for fill in FILLS {
            let path = filled(route.pattern, fill);
            for role in [UserRole::Admin, UserRole::Operator, UserRole::Viewer, UserRole::Remote] {
                for method in METHODS {
                    assert_eq!(
                        crate::rbac::check_permission(&role, method, &path),
                        legacy::check(&role, method, &path),
                        "{role:?} {method} {path}"
                    );
                }
            }
        }
    }
}

fn granted_member() -> NabimanUser {
    let mut m: NabimanUser = serde_json::from_value(serde_json::json!({
        "id": "route-access-member", "username": "route-access-member", "password_hash": "",
        "role": "remote", "created_at": "now", "status": "active"
    }))
    .unwrap();
    let p = &mut m.remote_permissions;
    (p.dictation, p.register_servers, p.copy_between_servers, p.assigned_file_write) = (true, true, true, true);
    (p.share_sessions, p.share_files) = (true, true);
    m
}

#[test]
fn members_keep_everything_their_rules_allowed() {
    let m = granted_member();
    for route in all().filter(|r| matches!(r.member, Member::No | Member::Only(_))) {
        for fill in FILLS {
            let path = filled(route.pattern, fill);
            for method in METHODS {
                if crate::member_servers::rules(&m, method, &path) {
                    assert!(member_reachable(method, &path), "{method} {path} was open to members");
                }
            }
        }
    }
}

#[test]
fn members_reach_nothing_unlisted() {
    let m = granted_member();
    for path in ["/api/remote/consoles/x/new-thing", "/api/terminal/sessions/x/y/z", "/api/terminal/brand-new"] {
        assert!(!crate::member_servers::permitted(&m, "POST", path), "{path}");
    }
    assert!(crate::member_servers::permitted(&m, "GET", "/api/terminal/sessions"));
    assert!(!crate::member_servers::permitted(&m, "DELETE", "/api/terminal/sessions"));
}

#[test]
fn literal_segments_win_and_no_two_routes_overlap_alike() {
    assert_eq!(lookup("/api/config/services").unwrap().pattern, "/api/config/services");
    assert_eq!(lookup("/api/config/nginx").unwrap().pattern, "/api/config/{service_id}");
    assert!(lookup("/api/config/").is_none() && lookup("/api/config/nginx/x/y").is_none());
    let mut shapes = std::collections::BTreeMap::new();
    for route in all() {
        let shape: Vec<&str> = route.pattern.split('/').map(|s| if variable(s) { "{}" } else { s }).collect();
        assert!(shapes.insert(shape, route.pattern).is_none(), "{} has the same shape as another route", route.pattern);
    }
}

#[test]
fn the_document_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/security/route-access.md");
    let want = doc::render();
    if std::env::var_os("NABIMAN_WRITE_ROUTE_DOC").is_some() {
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, &want).unwrap();
    }
    let have = std::fs::read_to_string(path).unwrap_or_default();
    assert!(have == want, "docs/security/route-access.md is out of date: run NABIMAN_WRITE_ROUTE_DOC=1 cargo test route_access");
}
