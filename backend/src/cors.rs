//! Which browser origins may call the API (`NABIMAN_CORS_ORIGINS`).

use actix_cors::Cors;

pub(crate) fn build_cors() -> Cors {
    match std::env::var("NABIMAN_CORS_ORIGINS") {
        Ok(origins) if !origins.is_empty() => {
            let mut cors = Cors::default()
                .allow_any_method()
                .expose_headers(["X-Nabiman-MFA-Required"])
                .allowed_headers([
                    "Content-Type",
                    "Authorization",
                    "X-Auth-Token",
                    "X-API-Key",
                    "X-File-Path",
                ])
                .max_age(3600);
            for origin in origins.split(',') {
                cors = cors.allowed_origin(origin.trim());
            }
            cors
        }
        _ => Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .expose_headers(["X-Nabiman-MFA-Required"])
            .allowed_headers([
                "Content-Type",
                "Authorization",
                "X-Auth-Token",
                "X-API-Key",
                "X-File-Path",
            ])
            .max_age(3600),
    }
}
