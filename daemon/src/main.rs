#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;

use env_logger::Env;
// use structopt::StructOpt;
use rebuilderd_common::errors::*;
use actix_web::{web, App, HttpServer};
use rebuilderd_common::api::SuiteImport;
use actix_web::FromRequest;

mod api;
mod db;
mod schema;
mod models;
mod versions;

/*
#[derive(Debug, StructOpt)]
//#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, StructOpt)]
enum SubCommand {
    /// Rebuild an individual package
    Build(Build),
    /// Connect to a central rebuilderd daemon for work
    Connect(Connect),
}

#[derive(Debug, StructOpt)]
struct Build {
    pub distro: rebuilderd_common::Distro,
    pub inputs: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub endpoint: String,
}
*/

fn run() -> Result<()> {
    dotenv::dotenv().ok();

    let bind = std::env::var("HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "rebuilderd.db".to_string());
    let pool = db::setup_pool(&url)?;

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .service(api::list_workers)
            .service(api::fetch_work)
            .service(api::list_pkgs)
            // .service(api::sync_work)
            .service(
                web::resource("/api/v0/job/sync").data(
                    // change json extractor configuration
                    web::Json::<SuiteImport>::configure(|cfg| {
                        cfg.limit(256 * 1024 * 1024)
                    })
                )
                .route(web::post().to(api::sync_work))
            )
    }).bind(bind)?
    .run()?;
    Ok(())
}

fn main() {
    env_logger::init_from_env(Env::default()
        .default_filter_or("info"));

    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
