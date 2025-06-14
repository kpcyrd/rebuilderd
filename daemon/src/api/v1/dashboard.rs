use crate::db::Pool;
use crate::web;
use actix_web::{get, HttpResponse, Responder};
use rebuilderd_common::errors::Error;

#[get("/")]
pub async fn get_dashboard(pool: web::Data<Pool>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}
