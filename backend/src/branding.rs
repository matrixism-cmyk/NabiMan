//! What the web app shows that belongs to the install, not to NabiMan: its
//! product name, an optional feedback widget, and the NOC's on-call contact.
//! `GET /api/branding` (anyone on the staff); the CSP's allowances for the
//! widget are derived from the same settings (see `security_headers`).
//!
//! The widget is off on a new install. `NABIMAN_FEEDBACK_ORIGIN` (the panel's
//! origin, framed in the page) and `NABIMAN_FEEDBACK_SITE` (the site's id there)
//! turn it on; `NABIMAN_FEEDBACK_SCRIPT` is the launcher the SPA loads, by
//! default the one bundled at `/fixhub-admin.js`.

use std::sync::OnceLock;

use actix_web::{web, HttpResponse};
use serde::Serialize;

use crate::install_config::values::{env, text_in, Lookup};
use crate::models::ApiResponse;

/// The launcher shipped with the web app.
const BUNDLED_SCRIPT: &str = "/fixhub-admin.js";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct FeedbackWidget {
    /// The launcher script the SPA adds to the page.
    pub script_url: String,
    /// Where the panel comes from (framed in the page).
    pub origin: String,
    /// The site's id with the feedback service.
    pub site: String,
}

#[derive(Serialize)]
struct Branding {
    product_name: String,
    feedback_widget: Option<FeedbackWidget>,
    noc_contact: Option<String>,
}

/// `scheme://host[:port]` with nothing after it, as CSP takes a source.
pub(crate) fn origin_of(text: &str) -> Option<String> {
    let url = reqwest::Url::parse(text.trim()).ok()?;
    let plain = matches!(url.scheme(), "https" | "http")
        && url.host_str().is_some_and(|h| !h.is_empty())
        && url.username().is_empty()
        && url.password().is_none();
    plain.then(|| url.origin().ascii_serialization())
}

/// The widget's settings, or `None` (off) unless both its origin and site are set and valid.
pub(crate) fn feedback_in(get: Lookup) -> Option<FeedbackWidget> {
    let origin = text_in(get, "NABIMAN_FEEDBACK_ORIGIN").and_then(|o| origin_of(&o));
    let site = text_in(get, "NABIMAN_FEEDBACK_SITE")
        .filter(|s| s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'));
    let script_url = match text_in(get, "NABIMAN_FEEDBACK_SCRIPT") {
        None => BUNDLED_SCRIPT.to_string(),
        Some(own) if own.starts_with('/') && !own.starts_with("//") => own,
        Some(other) => {
            origin_of(&other)?;
            other
        }
    };
    let (origin, site) = (origin?, site?);
    Some(FeedbackWidget { script_url, origin, site })
}

/// The widget as this process was started with (read once).
pub(crate) fn feedback() -> Option<&'static FeedbackWidget> {
    static WIDGET: OnceLock<Option<FeedbackWidget>> = OnceLock::new();
    WIDGET.get_or_init(|| feedback_in(&env)).as_ref()
}

/// The sources the CSP must add for `widget`: (frame-src, script-src).
pub(crate) fn csp_sources(widget: Option<&FeedbackWidget>) -> (Option<String>, Option<String>) {
    let Some(w) = widget else { return (None, None) };
    let script = (!w.script_url.starts_with('/')).then(|| origin_of(&w.script_url)).flatten();
    (Some(w.origin.clone()), script)
}

fn branding_in(get: Lookup) -> Branding {
    Branding {
        product_name: text_in(get, "NABIMAN_PRODUCT_NAME").unwrap_or_else(|| "NabiMan".into()),
        feedback_widget: feedback_in(get),
        noc_contact: text_in(get, "NABIMAN_NOC_CONTACT"),
    }
}

async fn get_branding() -> HttpResponse {
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(branding_in(&env)))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/branding", web::get().to(get_branding));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
    }

    #[test]
    fn the_widget_is_off_unless_configured() {
        assert_eq!(feedback_in(&with(&[])), None);
        assert_eq!(feedback_in(&with(&[("NABIMAN_FEEDBACK_ORIGIN", "https://feedback.example")])), None, "no site");
        assert_eq!(feedback_in(&with(&[("NABIMAN_FEEDBACK_ORIGIN", "javascript:x"), ("NABIMAN_FEEDBACK_SITE", "s")])), None);
        assert_eq!(feedback_in(&with(&[("NABIMAN_FEEDBACK_ORIGIN", "https://f.example"), ("NABIMAN_FEEDBACK_SITE", "a b")])), None);
        let b = branding_in(&with(&[]));
        assert_eq!((b.product_name.as_str(), b.feedback_widget, b.noc_contact), ("NabiMan", None, None));
        assert_eq!(csp_sources(None), (None, None));
    }

    #[test]
    fn a_configured_widget_and_its_csp_sources() {
        let get = with(&[("NABIMAN_FEEDBACK_ORIGIN", "https://feedback.example/panel"), ("NABIMAN_FEEDBACK_SITE", "fs_Site-1")]);
        let w = feedback_in(&get).unwrap();
        assert_eq!(w, FeedbackWidget { script_url: "/fixhub-admin.js".into(), origin: "https://feedback.example".into(), site: "fs_Site-1".into() });
        assert_eq!(csp_sources(Some(&w)), (Some("https://feedback.example".into()), None), "the bundled launcher is this origin's");
        let get = with(&[
            ("NABIMAN_FEEDBACK_ORIGIN", "https://feedback.example:8443"),
            ("NABIMAN_FEEDBACK_SITE", "s"),
            ("NABIMAN_FEEDBACK_SCRIPT", "https://cdn.example/w.js"),
        ]);
        let w = feedback_in(&get).unwrap();
        assert_eq!(csp_sources(Some(&w)), (Some("https://feedback.example:8443".into()), Some("https://cdn.example".into())));
        let bad = with(&[("NABIMAN_FEEDBACK_ORIGIN", "https://f.example"), ("NABIMAN_FEEDBACK_SITE", "s"), ("NABIMAN_FEEDBACK_SCRIPT", "//evil.example/x.js")]);
        assert_eq!(feedback_in(&bad), None);
    }

    #[actix_web::test]
    async fn the_endpoint_answers_with_the_install_branding() {
        use actix_web::{test, App};
        let app = test::init_service(App::new().configure(config)).await;
        let res = test::call_service(&app, test::TestRequest::get().uri("/api/branding").to_request()).await;
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = test::read_body_json(res).await;
        assert!(body["data"]["product_name"].is_string());
        assert!(body["data"].get("feedback_widget").is_some() && body["data"].get("noc_contact").is_some());
        if std::env::var_os("NABIMAN_FEEDBACK_ORIGIN").is_none() {
            assert!(body["data"]["feedback_widget"].is_null());
        }
    }
}
