mod server_status;
mod network;
mod accounts;
mod config_manager;
mod traffic;
mod models;

use actix_cors::Cors;
use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("NabiMan Server starting on http://0.0.0.0:8080");

    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .configure(server_status::config)
            .configure(network::config)
            .configure(accounts::config)
            .configure(config_manager::config)
            .configure(traffic::config)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
