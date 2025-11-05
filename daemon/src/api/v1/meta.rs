use crate::api::v1::util::filters::IntoFilter;
use crate::db::Pool;
use crate::schema::{build_inputs, source_packages};
use crate::{attestation, web};
use actix_web::{get, HttpResponse, Responder};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteExpressionMethods};
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

    let distributions = source_packages::table
        .filter(freshness_filter.into_inner().into_filter())
        .select(source_packages::distribution)
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

    let distribution_releases = source_packages::table
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .filter(freshness_filter.into_inner().into_filter())
        .select(source_packages::release)
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

    let distribution_architectures = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .filter(freshness_filter.into_inner().into_filter())
        .select(build_inputs::architecture)
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

    let distribution_components = source_packages::table
        .filter(source_packages::distribution.is(distribution.into_inner()))
        .filter(freshness_filter.into_inner().into_filter())
        .select(source_packages::component)
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

    let mut query = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(&path.0))
        .into_boxed();

    // Handle "null" string as NULL
    if path.1 == "null" {
        query = query.filter(source_packages::release.is_null());
    } else {
        query = query.filter(source_packages::release.is(&path.1));
    }

    let distribution_release_architectures = query
        .filter(freshness_filter.into_inner().into_filter())
        .select(build_inputs::architecture)
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

    let mut query = source_packages::table
        .filter(source_packages::distribution.is(&path.0))
        .into_boxed();

    // Handle "null" string as NULL
    if path.1 == "null" {
        query = query.filter(source_packages::release.is_null());
    } else {
        query = query.filter(source_packages::release.is(&path.1));
    }

    let distribution_release_components = query
        .filter(freshness_filter.into_inner().into_filter())
        .select(source_packages::component)
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

    let mut query = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.is(&path.0))
        .into_boxed();

    // Handle empty release or "null" string as NULL
    if path.1.is_empty() || path.1 == "null" {
        query = query.filter(source_packages::release.is_null());
    } else {
        query = query.filter(source_packages::release.is(&path.1));
    }

    // Handle empty component or "null" string as NULL
    if path.2.is_empty() || path.2 == "null" {
        query = query.filter(source_packages::component.is_null());
    } else {
        query = query.filter(source_packages::component.is(&path.2));
    }

    let distribution_release_component_architectures = query
        .filter(freshness_filter.into_inner().into_filter())
        .select(build_inputs::architecture)
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
