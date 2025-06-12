use crate::api::forward_compressed_data;
use crate::api::v1::filters::{IdentityFilter, OriginFilter};
use crate::api::v1::pagination::{Page, SortDirection};
use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::diesel::QueryDsl;
use crate::schema::{build_inputs, rebuild_artifacts, rebuilds, source_packages};
use crate::web;
use actix_web::{get, post, HttpRequest, HttpResponse, Responder};
use diesel::dsl::InnerJoinQuerySource;
use diesel::sql_types::Bool;
use diesel::sqlite::Sqlite;
use diesel::{BoxableExpression, OptionalExtension, RunQueryDsl};
use rebuilderd_common::api;
use rebuilderd_common::api::v1::{Rebuild, RebuildReport};
use rebuilderd_common::errors::Error;

type BuildsFilter = Box<
    dyn BoxableExpression<
        InnerJoinQuerySource<
            rebuilds::table,
            InnerJoinQuerySource<build_inputs::table, source_packages::table>,
        >,
        Sqlite,
        SqlType = Bool,
    >,
>;

#[get("/api/v1/builds")]
pub async fn get_builds(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = rebuilds::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
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
        .into_boxed();

    sql = origin_filter.filter(sql, build_inputs::architecture);
    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    // apply pagination
    if let Some(sort) = &page.sort {
        let direction = &page.direction.clone().unwrap_or(SortDirection::Ascending);

        if *direction == SortDirection::Ascending {
            sql = match sort.as_str() {
                "id" => sql.order(rebuilds::id),
                "name" => sql.order(source_packages::name),
                "version" => sql.order(source_packages::version),
                "distribution" => sql.order(source_packages::distribution),
                "release" => sql.order(source_packages::release),
                "component" => sql.order(source_packages::component),
                "architecture" => sql.order(build_inputs::architecture),
                "backend" => sql.order(build_inputs::backend),
                "retries" => sql.order(build_inputs::retries),
                "started_at" => sql.order(rebuilds::started_at),
                "built_at" => sql.order(rebuilds::built_at),
                "status" => sql.order(rebuilds::status),
                _ => return Ok(HttpResponse::BadRequest().finish()),
            };
        } else {
            sql = match sort.as_str() {
                "id" => sql.order(rebuilds::id.desc()),
                "name" => sql.order(source_packages::name.desc()),
                "version" => sql.order(source_packages::version.desc()),
                "distribution" => sql.order(source_packages::distribution.desc()),
                "release" => sql.order(source_packages::release.desc()),
                "component" => sql.order(source_packages::component.desc()),
                "architecture" => sql.order(build_inputs::architecture.desc()),
                "backend" => sql.order(build_inputs::backend.desc()),
                "retries" => sql.order(build_inputs::retries.desc()),
                "started_at" => sql.order(rebuilds::started_at.desc()),
                "built_at" => sql.order(rebuilds::built_at.desc()),
                "status" => sql.order(rebuilds::status.desc()),
                _ => return Ok(HttpResponse::BadRequest().finish()),
            };
        }

        sql = sql.then_order_by(rebuilds::id);
    }

    let rebuilds = sql
        .get_results::<Rebuild>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(rebuilds))
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

    let build_log = rebuilds::table
        .filter(rebuilds::id.eq(id.into_inner()))
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

    let artifacts = rebuilds::table
        .inner_join(rebuild_artifacts::table)
        .filter(rebuilds::id.eq(id.into_inner()))
        .select((
            rebuild_artifacts::id,
            rebuild_artifacts::name,
            rebuild_artifacts::diffoscope.is_not_null(),
            rebuild_artifacts::attestation.is_not_null(),
            rebuild_artifacts::status,
        ))
        .get_results::<api::v1::RebuildArtifact>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(artifacts))
}

#[get("/api/v1/builds/{id}/artifacts/{artifact_id}")]
pub async fn get_build_artifact(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    artifact_id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let artifact = rebuilds::table
        .inner_join(rebuild_artifacts::table)
        .filter(rebuilds::id.eq(id.into_inner()))
        .filter(rebuild_artifacts::id.eq(artifact_id.into_inner()))
        .select((
            rebuild_artifacts::id,
            rebuild_artifacts::name,
            rebuild_artifacts::diffoscope.is_not_null(),
            rebuild_artifacts::attestation.is_not_null(),
            rebuild_artifacts::status,
        ))
        .first::<api::v1::RebuildArtifact>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(artifact) = artifact {
        Ok(HttpResponse::Ok().json(artifact))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/api/v1/builds/{id}/artifacts/{artifact_id}/diffoscope")]
pub async fn get_build_artifact_diffoscope(
    req: HttpRequest,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    artifact_id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let diffoscope = rebuilds::table
        .inner_join(rebuild_artifacts::table)
        .filter(rebuilds::id.eq(id.into_inner()))
        .filter(rebuild_artifacts::id.eq(artifact_id.into_inner()))
        .select(rebuild_artifacts::diffoscope)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(diffoscope) = diffoscope.flatten() {
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

    let attestation = rebuilds::table
        .inner_join(rebuild_artifacts::table)
        .filter(rebuilds::id.eq(id.into_inner()))
        .filter(rebuild_artifacts::id.eq(artifact_id.into_inner()))
        .select(rebuild_artifacts::attestation)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(attestation) = attestation.flatten() {
        forward_compressed_data(req, "application/json; charset=utf-8", attestation).await
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}
