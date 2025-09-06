use crate::api::v1::util::filters::DieselFreshnessFilter;
use crate::db::Pool;
use crate::schema::{build_inputs, source_packages};
use crate::{attestation, web};
use actix_web::{get, HttpResponse, Responder};
use diesel::{QueryDsl, RunQueryDsl, SqliteExpressionMethods};
use in_toto::crypto::PrivateKey;
use rebuilderd_common::api::v1::FreshnessFilter;
use rebuilderd_common::errors::Error;
use serde_json::json;
use std::sync::Arc;

#[get("/distributions")]
pub async fn get_distributions(
    pool: web::Data<Pool>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .select(source_packages::distribution)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distributions = sql
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distributions))
}

#[get("/distributions/{distribution}/releases")]
pub async fn get_distribution_releases(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .select(source_packages::release)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_releases = sql
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_releases))
}

#[get("/distributions/{distribution}/architectures")]
pub async fn get_distribution_architectures(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .select(build_inputs::architecture)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_architectures = sql
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_architectures))
}

#[get("/distributions/{distribution}/components")]
pub async fn get_distribution_components(
    pool: web::Data<Pool>,
    distribution: web::Path<String>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .select(source_packages::component)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_components = sql
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_components))
}

#[get("/distributions/{distribution}/{release}/architectures")]
pub async fn get_distribution_release_architectures(
    pool: web::Data<Pool>,
    path: web::Path<(String, String)>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(&path.0))
        .filter(source_packages::release.is(&path.1))
        .select(build_inputs::architecture)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_release_architectures = sql
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_architectures))
}

#[get("/distributions/{distribution}/{release}/components")]
pub async fn get_distribution_release_components(
    pool: web::Data<Pool>,
    path: web::Path<(String, String)>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .filter(source_packages::distribution.is(&path.0))
        .filter(source_packages::release.is(&path.1))
        .select(source_packages::component)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_release_components = sql
        .distinct()
        .load::<Option<String>>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_components))
}

#[get("/distributions/{distribution}/{release}/components/{component}/architectures")]
pub async fn get_distribution_release_component_architectures(
    pool: web::Data<Pool>,
    path: web::Path<(String, String, String)>,
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(&path.0))
        .filter(source_packages::release.is(&path.1))
        .filter(source_packages::component.is(&path.2))
        .select(build_inputs::architecture)
        .into_boxed();

    sql = freshness_filter.filter(sql);

    let distribution_release_component_architectures = sql
        .distinct()
        .load::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(distribution_release_component_architectures))
}

#[get("/public-keys")]
pub async fn get_public_key(
    private_key: web::Data<Arc<PrivateKey>>,
) -> web::Result<impl Responder> {
    let public_key = attestation::pubkey_to_pem(private_key.public())?;

    Ok(HttpResponse::Ok().json(json!({
        "current": vec![public_key],
    })))
}
