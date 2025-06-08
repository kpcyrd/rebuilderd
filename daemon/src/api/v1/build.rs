use crate::api::forward_compressed_data;
use crate::api::v1::filters::{IdentityFilter, OriginFilter};
use crate::api::v1::pagination::Page;
use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::schema::{build_inputs, rebuild_artifacts, rebuilds, source_packages};
use crate::web;
use actix_web::{get, post, HttpRequest, HttpResponse, Responder};
use diesel::{OptionalExtension, RunQueryDsl};
use rebuilderd_common::api::v1::{Rebuild, RebuildReport};
use rebuilderd_common::errors::Error;

#[get("/api/v1/builds")]
pub async fn get_builds(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[post("/api/v1/builds")]
pub async fn submit_rebuild_report(
    pool: web::Data<Pool>,
    request: web::Json<RebuildReport>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/builds/{id}")]
pub async fn get_build(pool: web::Data<Pool>, id: web::Path<i32>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(rebuild) = rebuilds::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .filter(rebuilds::id.eq(*id))
        .select((
            rebuilds::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            build_inputs::architecture,
            build_inputs::backend,
            build_inputs::retries,
            rebuilds::started_at,
            rebuilds::built_at,
            rebuilds::status,
        ))
        .get_result::<Rebuild>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(rebuild))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/api/v1/builds/{id}/log")]
pub async fn get_build_log(
    req: HttpRequest,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let build_log = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .inner_join(rebuilds::table)
        .select(rebuilds::build_log)
        .first::<Vec<u8>>(connection.as_mut())
        .map_err(Error::from)?;

    forward_compressed_data(req, "text/plain; charset=utf-8", build_log).await
}

#[get("/api/v1/builds/{id}/artifacts")]
pub async fn get_build_artifacts(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/builds/{id}/artifacts/{artifact_id}")]
pub async fn get_build_artifact(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    artifact_id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    Ok(HttpResponse::NotImplemented())
}

#[get("/api/v1/builds/{id}/artifacts/{artifact_id}/diffoscope")]
pub async fn get_build_artifact_diffoscope(
    req: HttpRequest,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    artifact_id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let diffoscope = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .select(rebuild_artifacts::diffoscope)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .map_err(Error::from)?;

    if let Some(diffoscope) = diffoscope {
        forward_compressed_data(req, "text/plain; charset=utf-8", diffoscope).await
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/api/v1/builds/{id}/artifacts/{artifact_id}/attestation")]
pub async fn get_build_artifact_attestation(
    req: HttpRequest,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    artifact_id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let attestation = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .select(rebuild_artifacts::attestation)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .map_err(Error::from)?;

    if let Some(attestation) = attestation {
        forward_compressed_data(req, "application/json; charset=utf-8", attestation).await
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}
