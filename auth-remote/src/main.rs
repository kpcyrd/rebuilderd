use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use env_logger::Env;
use handlebars::{DirectorySourceOptions, Handlebars};
use rebuilderd_common::errors::*;
use serde::Deserialize;

#[get("/")]
async fn index(hbs: web::Data<Handlebars<'_>>) -> impl Responder {
    let Ok(html) = hbs
        .render("index.html", &())
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
    let log_level = "info";
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    let mut handlebars = Handlebars::new();
    handlebars.register_templates_directory("templates", DirectorySourceOptions::default())?;

    let handlebars_ref = web::Data::new(handlebars);

    HttpServer::new(move || {
        App::new()
            .app_data(handlebars_ref.clone())
            .service(index)
            .service(auth_login)
            .service(auth_callback)
            .service(schedule)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;
    Ok(())
}
