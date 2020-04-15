#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;

use actix_web::{web, App, HttpServer, FromRequest};
use actix_web::middleware::Logger;
use env_logger::Env;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd_common::api::SuiteImport;
use rebuilderd_common::errors::*;

pub mod api;
pub mod db;
pub mod schema;
pub mod sync;
pub mod models;
pub mod versions;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(short)]
    verbose: bool,
}

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
            .service(api::push_queue)
            .service(api::pop_queue)
            .service(api::drop_from_queue)
            .service(api::ping_build)
            .service(api::report_build)
            .service(
                web::resource("/api/v0/pkgs/sync").app_data(
                    // change json extractor configuration
                    web::Json::<SuiteImport>::configure(|cfg| {
                        cfg.limit(128 * 1024 * 1024)
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
    let args = Args::from_args();

    let logging = if args.verbose {
        "actix_web=debug,rebuilderd=debug,info"
    } else {
        "actix_web=debug,info"
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    if let Err(err) = run().await {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
