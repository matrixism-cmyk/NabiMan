mod server_status;
mod network;
mod accounts;
mod config_manager;
mod traffic;
mod packages;
mod terminal;
mod auth;
mod models;

use actix_cors::Cors;
use actix_files as afs;
use actix_web::{App, HttpServer, web, HttpResponse};
use actix_web::dev::{ServiceRequest, ServiceResponse, Transform, Service};
use actix_web::body::EitherBody;
use std::future::{Ready, ready, Future};
use std::pin::Pin;
use std::rc::Rc;

// --- Auth Middleware ---
pub struct AuthCheck {
    store: auth::TokenStore,
}

impl AuthCheck {
    pub fn new(store: auth::TokenStore) -> Self {
        Self { store }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = AuthCheckMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthCheckMiddleware {
            service: Rc::new(service),
            store: self.store.clone(),
        }))
    }
}

pub struct AuthCheckMiddleware<S> {
    service: Rc<S>,
    store: auth::TokenStore,
}

impl<S, B> Service<ServiceRequest> for AuthCheckMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if !auth::check_auth(&req, &self.store) {
            return Box::pin(async {
                Ok(req.into_response(
                    HttpResponse::Unauthorized()
                        .json(models::ApiResponse::<()>::error("Unauthorized"))
                ).map_into_right_body())
            });
        }

        let svc = self.service.clone();
        Box::pin(async move {
            svc.call(req).await.map(|res| res.map_into_left_body())
        })
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = std::env::var("NABIMAN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let static_dir = std::env::var("NABIMAN_STATIC")
        .unwrap_or_else(|_| "./static".to_string());

    let token_store = auth::new_token_store();

    println!("NabiMan Server starting on http://0.0.0.0:{}", port);
    println!("Static files: {}", static_dir);
    println!("Set NABIMAN_PASSWORD env to change admin password (default: nabiman)");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        let store = token_store.clone();
        let index_path = format!("{}/index.html", static_dir);

        App::new()
            .wrap(cors)
            .wrap(AuthCheck::new(store.clone()))
            .app_data(web::Data::new(store))
            .configure(auth::config)
            .configure(server_status::config)
            .configure(network::config)
            .configure(accounts::config)
            .configure(config_manager::config)
            .configure(traffic::config)
            .configure(packages::config)
            .configure(terminal::config)
            .service(
                afs::Files::new("/", &static_dir)
                    .index_file("index.html")
                    .default_handler(
                        web::to(move || {
                            let path = index_path.clone();
                            async move {
                                afs::NamedFile::open_async(path).await
                            }
                        })
                    )
            )
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
