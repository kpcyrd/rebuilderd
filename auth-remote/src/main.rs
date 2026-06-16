mod args;
mod background;

use crate::args::Args;
use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use arc_swap::ArcSwap;
use clap::Parser;
use env_logger::Env;
use handlebars::{DirectorySourceOptions, Handlebars};
use rebuilderd_common::api::Client;
use rebuilderd_common::errors::*;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Default)]
pub struct Cache {
    binary_pkgs: BTreeSet<String>,
    source_pkgs: BTreeSet<String>,
}

#[get("/")]
async fn index(
    hbs: web::Data<Handlebars<'_>>,
    cache: web::Data<Arc<ArcSwap<Cache>>>,
) -> impl Responder {
    let cache = cache.load();
    let binary_pkgs = &cache.binary_pkgs;
    let source_pkgs = &cache.source_pkgs;

    let Ok(html) = hbs
        .render(
            "index.html",
            &json!({
                "binary_pkgs": binary_pkgs,
                "source_pkgs": source_pkgs,
            }),
        )
        .inspect_err(|err| error!("Template error: {err:#}"))
    else {
        return HttpResponse::InternalServerError().body("Template error");
    };
    HttpResponse::Ok().body(html)
}

#[get("/auth")]
async fn auth_login() -> impl Responder {
    // Generate state
    // Build GitLab authorization URL
    // Redirect user there

    HttpResponse::Found()
        .append_header(("Location", "https://TODO"))
        .finish()
}

#[derive(Debug, Deserialize)]
struct OAuthCallback {
    code: String,
    state: String,
}

#[get("/auth/callback")]
async fn auth_callback(query: web::Query<OAuthCallback>) -> impl Responder {
    // Verify state
    // Exchange code for access token
    // Fetch user info from GitLab
    // Create session/JWT

    println!("query={query:?}");

    HttpResponse::Ok().body("Logged in")
}

#[post("/schedule")]
async fn schedule() -> impl Responder {
    // Verify user session/JWT
    // Schedule a build for the given package

    HttpResponse::Ok().body("Build scheduled")
}

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    let client = Client::new(Default::default(), Some(args.endpoint))?;

    let mut handlebars = Handlebars::new();
    handlebars.register_templates_directory("templates", DirectorySourceOptions::default())?;
    let handlebars_ref = web::Data::new(handlebars);

    let cache = Arc::new(ArcSwap::from_pointee(Cache::default()));
    let cache_ref = web::Data::new(cache.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(handlebars_ref.clone())
            .app_data(cache_ref.clone())
            .service(index)
            .service(auth_login)
            .service(auth_callback)
            .service(schedule)
    })
    .bind(args.bind)?;

    tokio::select! {
        res = server.run() => res.map_err(|err| err.into()),
        res = background::run(client, cache) => Ok(res),
    }
}
