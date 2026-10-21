//! Where a product built on NabiMan plugs in, so that NabiMan never reaches
//! into it. Remote is the first (see `remote.rs`): its public share links,
//! its one-use URL tickets, its parts of the support report, and its servers
//! as a membership scope.
//!
//! The points are app data. An app that registers none still runs: nothing
//! is public, no URL ticket admits anyone, the report leaves those parts
//! out, and a server scope names no server.

use actix_web::{dev::ServiceRequest, web, HttpRequest};

use crate::auth::Claims;

/// A way in for a URL the browser opens itself, without the Authorization
/// header (a WebSocket, a file dragged to the desktop). `None`: not this
/// pass's URL, carry on. `Some(None)`: its URL, refused. `Some(Some(claims))`:
/// admitted, as the sign-in that asked for the ticket.
pub(crate) type UrlPass = fn(&ServiceRequest) -> Option<Option<Claims>>;

/// A part of the support report: it takes what it needs from the request,
/// then collects away from the async threads.
pub(crate) type ReportPart = fn(&HttpRequest) -> Box<dyn FnOnce() -> serde_json::Value + Send>;

/// Whether a server exists, for a membership scoped to one; an error when
/// the servers cannot be read.
pub(crate) type ServerExists = fn(&str) -> Result<bool, String>;

pub(crate) struct ExtensionPoints {
    /// API paths, by prefix, that are public: each request proves itself (a
    /// share link's own secret) and sign-in does not guard them.
    pub public_prefixes: Vec<&'static str>,
    pub url_passes: Vec<UrlPass>,
    /// By the name each part has in the report.
    pub report_parts: Vec<(&'static str, ReportPart)>,
    pub server_exists: Option<ServerExists>,
}

static NONE: ExtensionPoints = ExtensionPoints {
    public_prefixes: Vec::new(),
    url_passes: Vec::new(),
    report_parts: Vec::new(),
    server_exists: None,
};

impl ExtensionPoints {
    pub(crate) fn of(data: Option<&web::Data<ExtensionPoints>>) -> &ExtensionPoints {
        data.map_or(&NONE, |d| d.get_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{check_auth, AuthDecision};
    use crate::models::{Membership, ScopeKind, UserRole};
    use actix_web::test::TestRequest;
    use std::sync::{Arc, Mutex};

    fn decision(uri: &str, points: Option<ExtensionPoints>) -> AuthDecision {
        let mut req = TestRequest::get().uri(uri);
        if let Some(p) = points {
            req = req.app_data(web::Data::new(p));
        }
        let secret: crate::auth::JwtSecret = Arc::new("points-fixture-secret".into());
        let users = Arc::new(Mutex::new(Vec::new()));
        check_auth(&req.to_srv_request(), &secret, &crate::jwt_sessions::new_session_store(), &users)
    }

    #[test]
    fn without_remote_its_paths_and_tickets_open_nothing() {
        assert!(decision("/api/share/abc", None) == AuthDecision::Unauthorized);
        assert!(decision("/api/share/abc", Some(crate::remote::extension_points())) == AuthDecision::Allow);
        let refused = |_: &ServiceRequest| Some(None);
        let points = ExtensionPoints { url_passes: vec![refused], ..crate::remote::extension_points() };
        assert!(decision("/api/terminal?ticket=x", Some(points)) == AuthDecision::Unauthorized);
        assert!(decision("/api/terminal?ticket=x", None) == AuthDecision::Unauthorized);
    }

    #[test]
    fn without_remote_a_server_scope_names_no_server() {
        let m = Membership { kind: ScopeKind::Server, scope_id: "one".into(), role: UserRole::Admin, enabled: true, allow_exec: None, expires_at: None };
        assert!(crate::membership::validate_scope(&TestRequest::default().to_http_request(), &m).is_err());
        let here = ExtensionPoints { server_exists: Some(|id| Ok(id == "one")), ..crate::remote::extension_points() };
        let req = TestRequest::default().app_data(web::Data::new(here)).to_http_request();
        assert!(crate::membership::validate_scope(&req, &m).is_ok());
    }
}
