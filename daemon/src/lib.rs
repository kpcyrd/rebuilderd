#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;

use actix_web::middleware::Logger;
use actix_web::{App, HttpServer};
use actix_web::web::Data;
use crate::config::Config;
use crate::dashboard::DashboardState;
use diesel::RunQueryDsl;
use rebuilderd_common::errors::*;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub mod api;
pub mod auth;
pub mod config;
pub mod dashboard;
pub mod db;
pub mod schema;
pub mod sync;
pub mod models;
pub mod web;

pub async fn run_config(config: Config) -> Result<()> {
    let pool = db::setup_pool("rebuilderd.db")?;
    let bind_addr = config.bind_addr.clone();

    let dashboard_cache = Arc::new(RwLock::new(DashboardState::new()));

    {
        let pool = pool.clone();
        thread::spawn(move || {
            let connection = pool.get().expect("Failed to get connection from pool");
            loop {
                let query = diesel::sql_query("delete from builds as b where not exists (select 1 from packages as p where p.build_id = b.id);");
                info!("Deleting orphaned builds...");
                match query.execute(&connection) {
                    Ok(affected) => {
                        info!("Deleted {} orphaned builds", affected);
                    }
                    Err(err) => {
                        error!("Failed to delete orphaned builds: {:#}", err);
                    }
                }
                thread::sleep(Duration::from_secs(24 * 3600));
            }
        });
    }

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
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
            .service(web::resource("/api/v0/build/report")
                .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                .route(web::post().to(api::report_build))
            )
            .service(web::resource("/api/v0/pkgs/sync")
                .app_data(web::JsonConfig::default().limit(config.post_body_size_limit))
                .route(web::post().to(api::sync_work))
            )
    }).bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
