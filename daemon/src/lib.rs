extern crate diesel;
extern crate diesel_migrations;

use crate::config::Config;
use crate::dashboard::DashboardState;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{middleware, App, HttpServer};
use rebuilderd_common::errors::*;
use std::sync::{Arc, RwLock};

pub mod api;
pub mod auth;
pub mod code_migrations;
pub mod config;
pub mod dashboard;
pub mod db;
pub mod models;
pub mod schema;
pub mod sync;
pub mod util;
pub mod web;

pub async fn run_config(config: Config) -> Result<()> {
    let pool = db::setup_pool("rebuilderd.db")?;
    let bind_addr = config.bind_addr.clone();

    let dashboard_cache = Arc::new(RwLock::new(DashboardState::new()));

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(middleware::Compress::default())
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(dashboard_cache.clone()))
            .service(api::v0::list_workers)
            .service(api::v0::list_pkgs)
            .service(api::v0::list_queue)
            .service(api::v0::push_queue)
            .service(api::v0::pop_queue)
            .service(api::v0::drop_from_queue)
            .service(api::v0::requeue_pkgbase)
            .service(api::v0::ping_build)
            .service(api::v0::get_build_log)
            .service(api::v0::get_diffoscope)
            .service(api::v0::get_attestation)
            .service(api::v0::get_dashboard)
            .service(
                web::resource("/api/v0/build/report")
                    .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                    .route(web::post().to(api::v0::report_build)),
            )
            .service(
                web::resource("/api/v0/pkgs/sync")
                    .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                    .route(web::post().to(api::v0::sync_work)),
            )
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
