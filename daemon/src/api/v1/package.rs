use crate::api::v1::util::filters::{IdentityFilter, OriginFilter};
use crate::api::v1::util::pagination::{Page, PaginateDsl};
use crate::api::DEFAULT_QUEUE_PRIORITY;
use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::diesel::NullableExpressionMethods;
use crate::models::{NewBinaryPackage, NewBuildInput, NewQueued, NewSourcePackage};
use crate::schema::{binary_packages, build_inputs, rebuilds, source_packages};
use crate::web;
use actix_web::{get, post, HttpResponse, Responder};
use chrono::Utc;
use diesel::{OptionalExtension, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v1::{PackageReport, ResultPage};
use rebuilderd_common::errors::Error;

#[diesel::dsl::auto_type]
fn source_packages_base() -> _ {
    source_packages::table
        .inner_join(build_inputs::table)
        .select((
            source_packages::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release.nullable(),
            source_packages::component.nullable(),
            build_inputs::architecture,
        ))
}

#[diesel::dsl::auto_type]
fn binary_packages_base() -> _ {
    binary_packages::table
        .inner_join(source_packages::table)
        .select((
            binary_packages::id,
            binary_packages::name,
            binary_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            binary_packages::architecture,
            binary_packages::artifact_url,
        ))
}

#[post("/")]
pub async fn submit_package_report(
    pool: web::Data<Pool>,
    request: web::Json<PackageReport>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let report = request.into_inner();

    for package_report in report.packages {
        let new_source_package = NewSourcePackage {
            name: package_report.name,
            version: package_report.version,
            distribution: report.distribution.clone(),
            release: report.release.clone(),
            component: report.component.clone(),
        };

        let source_package = new_source_package.upsert(connection.as_mut())?;

        let new_build_input = NewBuildInput {
            source_package_id: source_package.id,
            url: package_report.url,
            backend: report.distribution.clone(),
            architecture: report.architecture.clone(),
            retries: 0,
        };

        let build_input = new_build_input.upsert(connection.as_mut())?;

        for artifact_report in package_report.artifacts {
            let new_binary_package = NewBinaryPackage {
                source_package_id: source_package.id,
                build_input_id: build_input.id,
                name: artifact_report.name,
                version: artifact_report.version,
                architecture: report.architecture.clone(),
                artifact_url: artifact_report.url,
            };

            new_binary_package.upsert(connection.as_mut())?;
        }

        let needs_rebuild = match rebuilds::table
            .filter(rebuilds::build_input_id.eq(build_input.id))
            .select(rebuilds::status)
            .get_result::<Option<String>>(connection.as_mut())
            .optional()
            .map_err(Error::from)?
            .flatten()
        {
            None => true,
            Some(value) => value != "GOOD",
        };

        if needs_rebuild {
            let new_queued_job = NewQueued {
                build_input_id: build_input.id,
                priority: DEFAULT_QUEUE_PRIORITY,
                queued_at: Utc::now().naive_utc(),
            };

            new_queued_job.upsert(connection.as_mut())?;
        }
    }

    Ok(HttpResponse::NotImplemented())
}

#[get("/source")]
pub async fn get_source_packages(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let base = source_packages_base();
    let mut sql = base.into_boxed();

    sql = origin_filter.filter(sql, build_inputs::architecture);
    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::SourcePackage>(connection.as_mut())
        .map_err(Error::from)?;

    let total = base
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[get("/source/{id}")]
pub async fn get_source_package(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = source_packages_base()
        .filter(source_packages::id.eq(id.into_inner()))
        .get_result::<rebuilderd_common::api::v1::SourcePackage>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/binary")]
pub async fn get_binary_packages(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let base = binary_packages_base();
    let mut sql = base.into_boxed();

    sql = origin_filter.filter(sql, binary_packages::architecture);
    sql = identity_filter.filter(sql, binary_packages::name, binary_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::BinaryPackage>(connection.as_mut())
        .map_err(Error::from)?;

    let total = base
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[get("/binary/{id}")]
pub async fn get_binary_package(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = binary_packages_base()
        .filter(binary_packages::id.eq(id.into_inner()))
        .get_result::<rebuilderd_common::api::v1::BinaryPackage>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}
