extern crate diesel;
extern crate diesel_migrations;

use crate::config::Config;
use crate::dashboard::DashboardState;
use crate::models::Build;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{middleware, App, HttpServer};
use diesel::SqliteConnection;
use rebuilderd_common::errors::*;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub mod api;
pub mod auth;
pub mod config;
pub mod dashboard;
pub mod db;
pub mod models;
pub mod schema;
pub mod sync;
pub mod web;

fn db_collect_garbage(connection: &mut SqliteConnection) -> Result<()> {
    let orphaned = Build::find_orphaned(connection)?;

    if !orphaned.is_empty() {
        info!("Deleting {} orphaned builds...", orphaned.len());
        for ids in orphaned.chunks(500) {
            Build::delete_multiple(ids, connection).context("Failed to delete builds")?;
            debug!("Deleted chunk of {} builds", ids.len());
        }
        info!("Finished removing orphaned builds");
    }

    Ok(())
}

pub async fn run_config(config: Config) -> Result<()> {
    let pool = db::setup_pool("rebuilderd.db")?;
    let bind_addr = config.bind_addr.clone();

    let dashboard_cache = Arc::new(RwLock::new(DashboardState::new()));

    {
        let pool = pool.clone();
        thread::spawn(move || {
            let mut connection = pool.get().expect("Failed to get connection from pool");
            loop {
                debug!("Checking for orphaned builds...");

                if let Err(err) = db_collect_garbage(connection.as_mut()) {
                    error!("Failed to delete orphaned builds: {:#}", err);
                }

                debug!("Sleeping until next garbage collection cycle...");
                thread::sleep(Duration::from_secs(24 * 3600));
            }
        });
    }

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(middleware::Compress::default())
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(dashboard_cache.clone()))
            .service(api::list_workers)
            .service(api::list_pkgs)
            .service(api::list_queue)
            .service(api::push_queue)
            .service(api::pop_queue)
            .service(api::drop_from_queue)
            .service(api::requeue_pkgbase)
            .service(api::ping_build)
            .service(api::get_build_log)
            .service(api::get_diffoscope)
            .service(api::get_attestation)
            .service(api::get_dashboard)
            .service(
                web::resource("/api/v0/build/report")
                    .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                    .route(web::post().to(api::report_build)),
            )
            .service(
                web::resource("/api/v0/pkgs/sync")
                    .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                    .route(web::post().to(api::sync_work)),
            )
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
