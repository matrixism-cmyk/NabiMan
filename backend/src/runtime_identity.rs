//! Identify the executable this process actually started, including after atomic replacement.
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
};

/// Set at compile time by build.rs; `unknown` when it did not run (the
/// Dockerfile copies only Cargo.toml and src/).
const GIT_COMMIT: &str = match option_env!("NABIMAN_GIT_COMMIT") {
    Some(commit) => commit,
    None => "unknown",
};

#[derive(Serialize, Clone)]
pub struct RuntimeIdentity {
    package_version: &'static str,
    /// The commit it was built from (`-dirty`: with uncommitted changes), or
    /// `unknown` for a build without git (see build.rs).
    git_commit: &'static str,
    executable_sha256: String,
    instance_id: String,
    started_at: String,
}

impl RuntimeIdentity {
    pub fn capture() -> io::Result<Self> {
        // Do not follow argv[0], an environment-supplied build label, or the
        // installation pathname: an update can already have replaced that file.
        let mut executable = File::open("/proc/self/exe")?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = executable.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        Ok(Self {
            package_version: env!("CARGO_PKG_VERSION"),
            git_commit: GIT_COMMIT,
            executable_sha256: hex::encode(digest.finalize()),
            instance_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        })
    }
}

/// The identity, with the startup report of external programs (`tools`) and
/// where the settings came from (`settings`: the file and names, never values).
#[derive(Serialize)]
struct Runtime<'a> {
    #[serde(flatten)]
    identity: &'a RuntimeIdentity,
    tools: Option<crate::external_tools::Report>,
    settings: crate::install_config::Loaded,
}

#[derive(serde::Deserialize)]
struct Ask {
    #[serde(default)]
    refresh: Option<String>,
}

async fn inspect(identity: web::Data<RuntimeIdentity>, ask: web::Query<Ask>) -> HttpResponse {
    let refresh = ask.refresh.as_deref().is_some_and(|v| v == "1" || v == "true");
    let runtime = Runtime {
        identity: identity.get_ref(),
        tools: crate::external_tools::report(refresh),
        settings: crate::install_config::loaded(),
    };
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(runtime))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/runtime", web::get().to(inspect));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_commit_is_reported_next_to_the_version() {
        let ok = GIT_COMMIT == "unknown" || {
            let id = GIT_COMMIT.strip_suffix("-dirty").unwrap_or(GIT_COMMIT);
            id.len() == 12 && id.bytes().all(|b| b.is_ascii_hexdigit())
        };
        assert!(ok, "{GIT_COMMIT}");
        let value = serde_json::to_value(RuntimeIdentity::capture().unwrap()).unwrap();
        assert_eq!(value["package_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["git_commit"], GIT_COMMIT);
    }

    #[actix_web::test]
    async fn the_runtime_endpoint_answers_with_it() {
        use actix_web::{test, App};
        let identity = web::Data::new(RuntimeIdentity::capture().unwrap());
        let app = test::init_service(App::new().app_data(identity).configure(config)).await;
        let res = test::call_service(&app, test::TestRequest::get().uri("/runtime").to_request()).await;
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = test::read_body_json(res).await;
        assert_eq!(body["data"]["git_commit"], GIT_COMMIT);
        // The programs' report and the settings' source come with it (no check ran in tests).
        assert!(body["data"].get("tools").is_some() && body["data"]["settings"].get("from_file").is_some());
    }
}
