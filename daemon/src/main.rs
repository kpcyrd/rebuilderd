#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;

use env_logger::Env;
use rebuilderd_common::errors::*;
use actix_web::{web, App, HttpServer, FromRequest};
use actix_web::middleware::Logger;
use rebuilderd_common::api::SuiteImport;

pub mod api;
pub mod db;
pub mod schema;
pub mod sync;
pub mod models;
pub mod versions;

async fn run() -> Result<()> {
    dotenv::dotenv().ok();

    let bind = std::env::var("HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "rebuilderd.db".to_string());
    let pool = db::setup_pool(&url)?;

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .data(pool.clone())
            .service(api::list_workers)
            .service(api::list_pkgs)
            .service(api::list_queue)
            .service(api::pop_queue)
            .service(api::ping_build)
            .service(api::report_build)
            .service(
                web::resource("/api/v0/pkgs/sync").data(
                    // change json extractor configuration
                    web::Json::<SuiteImport>::configure(|cfg| {
                        cfg.limit(256 * 1024 * 1024)
                    })
                )
                .route(web::post().to(api::sync_work))
            )
    }).bind(bind)?
    .run()
    .await?;
    Ok(())
}

#[actix_rt::main]
async fn main() {
    env_logger::init_from_env(Env::default()
        .default_filter_or("info"));

    if let Err(err) = run().await {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
