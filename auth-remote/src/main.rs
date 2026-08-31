mod args;
mod background;
mod oidc;

use crate::args::Args;
use crate::oidc::Oidc;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use arc_swap::ArcSwap;
use clap::Parser;
use env_logger::Env;
use handlebars::{DirectorySourceOptions, Handlebars};
use rebuilderd_common::api::Client as ApiClient;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::collections::BTreeSet;
use std::sync::Arc;

handlebars::handlebars_helper!(to_json: |v: JsonValue| {
    serde_json::to_string(&v)
        .inspect_err(|_err| error!("Failed to serialize value to JSON: {v:?}"))
        .unwrap_or_default()
});

#[derive(Default)]
pub struct Cache {
    binary_pkgs: BTreeSet<Binary>,
    source_pkgs: BTreeSet<Source>,
    architectures: BTreeSet<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binary {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    pub name: String,
    pub version: String,
}

#[get("/")]
async fn index(
    hbs: web::Data<Handlebars<'_>>,
    cache: web::Data<Arc<ArcSwap<Cache>>>,
    oidc: web::Data<Oidc>,
) -> impl Responder {
    let cache = cache.load();

    let Ok(html) = hbs
        .render(
            "index.html",
            &json!({
                "binary_pkgs": cache.binary_pkgs,
                "source_pkgs": cache.source_pkgs,
                "architectures": cache.architectures,
                "authed": false, // TODO
            }),
        )
        .inspect_err(|err| error!("Template error: {err:#}"))
    else {
        return HttpResponse::InternalServerError().body("Template error");
    };
    HttpResponse::Ok().body(html)
}

#[get("/auth")]
async fn auth_login(oidc: web::Data<Oidc>) -> impl Responder {
    // Generate state
    // Build GitLab authorization URL
    // Redirect user there

    let (auth_url, cookie) = oidc.auth_url().await;

    HttpResponse::Found()
        .append_header(("Location", auth_url.as_str()))
        .cookie(cookie)
        .finish()
}

#[derive(Debug, Deserialize)]
struct OAuthCallback {
    code: String,
    state: String,
}

#[get("/auth/callback")]
async fn auth_callback(
    req: HttpRequest,
    query: web::Query<OAuthCallback>,
    oidc: web::Data<Oidc>,
) -> impl Responder {
    // Verify state
    // Exchange code for access token
    // Fetch user info from GitLab
    // Create session/JWT

    let Some(cookie) = req.cookie(oidc::COOKIE_NAME) else {
        return HttpResponse::BadRequest().body("Missing cookie");
    };

    if !oidc.verify(&cookie, &query.code, &query.state).await {
        return HttpResponse::BadRequest().body("Login failed");
    }

    HttpResponse::Ok().body("Logged in")
}

#[post("/schedule")]
async fn schedule(oidc: web::Data<Oidc>) -> impl Responder {
    // Verify user session/JWT
    // Schedule a build for the given package

    HttpResponse::Ok().body("Build scheduled")
}

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    let http = ApiClient::new(Default::default(), Some(args.endpoint))?;

    let mut handlebars = Handlebars::new();
    handlebars.register_helper("to_json", Box::new(to_json));
    handlebars.register_templates_directory("templates", DirectorySourceOptions::default())?;
    let handlebars_ref = web::Data::new(handlebars);

    let cache = Arc::new(ArcSwap::from_pointee(Cache::default()));
    let cache_ref = web::Data::new(cache.clone());

    let oidc = oidc::client(
        args.oidc_client_id,
        args.oidc_client_secret,
        args.oidc_issuer,
        args.oidc_redirect_uri,
    )
    .await?;
    let oidc_ref = web::Data::new(oidc);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(handlebars_ref.clone())
            .app_data(cache_ref.clone())
            .app_data(oidc_ref.clone())
            .service(index)
            .service(auth_login)
            .service(auth_callback)
            .service(schedule)
    })
    .bind(args.bind)?;

    tokio::select! {
        res = server.run() => res.map_err(|err| err.into()),
        res = background::run(http, cache) => Ok(res),
    }
}
