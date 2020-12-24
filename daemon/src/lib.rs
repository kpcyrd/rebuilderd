#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;

use actix_web::{App, HttpServer, FromRequest};
use actix_web::middleware::Logger;
use crate::config::Config;
use crate::dashboard::DashboardState;
use rebuilderd_common::api::{BuildReport, SuiteImport};
use rebuilderd_common::errors::*;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use structopt::StructOpt;
use structopt::clap::AppSettings;

pub mod api;
pub mod auth;
pub mod config;
pub mod dashboard;
pub mod db;
pub mod schema;
pub mod sync;
pub mod models;
pub mod versions;
pub mod web;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    /// Verbose logging
    #[structopt(short)]
    verbose: bool,
    /// Configuration file path
    #[structopt(short, long)]
    config: Option<PathBuf>,
}

pub async fn run_config(config: Config) -> Result<()> {
    let pool = db::setup_pool("rebuilderd.db")?;
    let bind_addr = config.bind_addr.clone();

    let dashboard_cache = Arc::new(RwLock::new(DashboardState::new()));

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .data(pool.clone())
            .data(config.clone())
            .data(dashboard_cache.clone())
            .service(api::list_workers)
            .service(api::list_pkgs)
            .service(api::list_queue)
            .service(api::push_queue)
            .service(api::pop_queue)
            .service(api::drop_from_queue)
            .service(api::requeue_pkg)
            .service(api::ping_build)
            .service(api::get_build_log)
            .service(api::get_diffoscope)
            .service(api::get_dashboard)
            .service(
                web::resource("/api/v0/build/report").app_data(
                    // change json extractor configuration
                    web::Json::<BuildReport>::configure(|cfg| {
                        cfg.limit(128 * 1024 * 1024)
                    })
                )
                .route(web::post().to(api::report_build))
            )
            .service(
                web::resource("/api/v0/pkgs/sync").app_data(
                    // change json extractor configuration
                    web::Json::<SuiteImport>::configure(|cfg| {
                        cfg.limit(128 * 1024 * 1024)
                    })
                )
                .route(web::post().to(api::sync_work))
            )
    }).bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
