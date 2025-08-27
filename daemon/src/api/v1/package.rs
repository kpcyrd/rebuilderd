use crate::api::v1::util::auth;
use crate::api::v1::util::filters::DieselIdentityFilter;
use crate::api::v1::util::filters::DieselOriginFilter;
use crate::api::v1::util::pagination::PaginateDsl;
use crate::api::DEFAULT_QUEUE_PRIORITY;
use crate::config::Config;
use crate::db::{Pool, SqliteConnectionWrap};
use crate::models::{BuildInput, NewBinaryPackage, NewBuildInput, NewQueued, NewSourcePackage};
use crate::schema::{
    binary_packages, build_inputs, queue, rebuild_artifacts, rebuilds, source_packages,
};
use crate::web;
use actix_web::{get, post, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::dsl::{exists, not, select};
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::sql_types::Integer;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    OptionalExtension, QueryDsl, RunQueryDsl,
};
use rebuilderd_common::api::v1::{IdentityFilter, OriginFilter, PackageReport, Page, ResultPage};
use rebuilderd_common::errors::Error;

use crate::api::v1::util::friends::build_input_friends;
use aliases::*;

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
}

#[diesel::dsl::auto_type]
fn source_packages_base() -> _ {
    source_packages::table
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).eq(build_inputs::id)))
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).eq(build_inputs::id).and(
                r1.field(rebuilds::built_at)
                    .lt(r2.field(rebuilds::built_at))
                    .or(r1.fields(
                        rebuilds::built_at
                            .eq(r2.field(rebuilds::built_at))
                            .and(r1.field(rebuilds::id).lt(r2.field(rebuilds::id))),
                    )),
            )),
        )
        .filter(r2.field(rebuilds::id).is_null())
        .select((
            source_packages::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release.nullable(),
            source_packages::component.nullable(),
            r1.field(rebuilds::status).nullable(),
            r1.field(rebuilds::id).nullable(),
        ))
}

#[diesel::dsl::auto_type]
fn binary_packages_base() -> _ {
    binary_packages::table
        .inner_join(source_packages::table)
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).eq(build_inputs::id)))
        .left_join(
            rebuild_artifacts::table.on(rebuild_artifacts::rebuild_id
                .eq(r1.field(rebuilds::id))
                .and(rebuild_artifacts::name.eq(binary_packages::name))),
        )
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).eq(build_inputs::id).and(
                r1.field(rebuilds::built_at)
                    .lt(r2.field(rebuilds::built_at))
                    .or(r1.fields(
                        rebuilds::built_at
                            .eq(r2.field(rebuilds::built_at))
                            .and(r1.field(rebuilds::id).lt(r2.field(rebuilds::id))),
                    )),
            )),
        )
        .filter(r2.field(rebuilds::id).is_null())
        .select((
            binary_packages::id,
            binary_packages::name,
            binary_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            binary_packages::architecture,
            binary_packages::artifact_url,
            rebuild_artifacts::status.nullable(),
            r1.field(rebuilds::id).nullable(),
            rebuild_artifacts::id.nullable(),
        ))
}

#[post("")]
pub async fn submit_package_report(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<PackageReport>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let report = request.into_inner();

    for package_report in report.packages {
        // check if this package already exists - this is used later to determine if we should copy over existing build
        // results to this package.
        let mut sql = source_packages::table
            .filter(source_packages::name.eq(&package_report.name))
            .filter(source_packages::version.eq(&package_report.version))
            .filter(source_packages::distribution.eq(&report.distribution))
            .into_boxed();

        if let Some(release) = &report.release {
            sql = sql.filter(source_packages::release.eq(release))
        } else {
            sql = sql.filter(source_packages::release.is_null())
        }

        if let Some(component) = &report.component {
            sql = sql.filter(source_packages::component.eq(component))
        } else {
            sql = sql.filter(source_packages::component.is_null())
        }

        let is_new_package = select(not(exists(sql)))
            .get_result::<bool>(connection.as_mut())
            .map_err(Error::from)?;

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

        if is_new_package {
            // in order to avoid additional rebuilds in distributions that copy existing packages between releases, we
            // want to also copy any results relevant to newly-imported versions. This only applies within a single
            // build backend and matches on the URL of the input artifact and its architecture.
            copy_existing_rebuilds(&mut connection, &build_input)?;
        }

        let needs_rebuild = match rebuilds::table
            .filter(rebuilds::build_input_id.eq(&build_input.id))
            .select(rebuilds::status)
            .order_by(rebuilds::built_at.desc())
            .get_result::<Option<String>>(connection.as_mut())
            .optional()
            .map_err(Error::from)?
            .flatten()
        {
            None => true,
            Some(value) => value != "GOOD",
        };

        let has_queued_friend = select(exists(
            queue::table
                .filter(queue::build_input_id.ne(&build_input.id))
                .filter(
                    queue::build_input_id.eq_any(
                        build_inputs::table
                            .filter(build_inputs::url.eq(&build_input.url))
                            .filter(build_inputs::backend.eq(&build_input.backend))
                            .filter(build_inputs::architecture.eq(&build_input.architecture))
                            .select(build_inputs::id),
                    ),
                )
                .select(queue::id),
        ))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

        if needs_rebuild && !has_queued_friend {
            let new_queued_job = NewQueued {
                build_input_id: build_input.id,
                priority: DEFAULT_QUEUE_PRIORITY,
                queued_at: Utc::now().naive_utc(),
            };

            new_queued_job.upsert(connection.as_mut())?;
        }
    }

    Ok(HttpResponse::NoContent().finish())
}

fn copy_existing_rebuilds(
    connection: &mut PooledConnection<ConnectionManager<SqliteConnectionWrap>>,
    build_input: &BuildInput,
) -> Result<(), Error> {
    // check if we have any existing rebuilds that match this package
    let existing_build_input = build_input_friends(build_input.id)
        .filter(build_inputs::id.ne(build_input.id))
        .order_by(build_inputs::id)
        .limit(1)
        .get_result::<i32>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(existing_build_input) = existing_build_input {
        // copy rebuilds
        let has_existing_rebuild = select(exists(
            rebuilds::table.filter(rebuilds::build_input_id.eq(existing_build_input)),
        ))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

        if has_existing_rebuild {
            let existing_rebuild_ids = rebuilds::table
                .filter(rebuilds::build_input_id.eq(existing_build_input))
                .select(rebuilds::id)
                .get_results::<i32>(connection.as_mut())
                .map_err(Error::from)?;

            for existing_rebuild_id in existing_rebuild_ids {
                let new_rebuild_id = diesel::dsl::insert_into(rebuilds::table)
                    .values(
                        rebuilds::table
                            .filter(rebuilds::id.eq(existing_rebuild_id))
                            .select((
                                diesel::dsl::sql::<Integer>("").bind::<Integer, _>(build_input.id),
                                rebuilds::started_at,
                                rebuilds::built_at,
                                rebuilds::build_log_id,
                                rebuilds::status,
                            )),
                    )
                    .into_columns((
                        rebuilds::build_input_id,
                        rebuilds::started_at,
                        rebuilds::built_at,
                        rebuilds::build_log_id,
                        rebuilds::status,
                    ))
                    .returning(rebuilds::id)
                    .get_result::<i32>(connection.as_mut())
                    .map_err(Error::from)?;

                // copy artifacts
                diesel::dsl::insert_into(rebuild_artifacts::table)
                    .values(
                        rebuild_artifacts::table
                            .filter(rebuild_artifacts::rebuild_id.eq(existing_rebuild_id))
                            .select((
                                diesel::dsl::sql::<Integer>("").bind::<Integer, _>(new_rebuild_id),
                                rebuild_artifacts::name,
                                rebuild_artifacts::diffoscope_log_id,
                                rebuild_artifacts::attestation_log_id,
                                rebuild_artifacts::status,
                            )),
                    )
                    .into_columns((
                        rebuild_artifacts::rebuild_id,
                        rebuild_artifacts::name,
                        rebuild_artifacts::diffoscope_log_id,
                        rebuild_artifacts::attestation_log_id,
                        rebuild_artifacts::status,
                    ))
                    .execute(connection.as_mut())
                    .map_err(Error::from)?;
            }
        }
    }

    Ok(())
}

#[get("/source")]
pub async fn get_source_packages(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages_base().into_boxed();
    sql = origin_filter.filter(sql);
    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::SourcePackage>(connection.as_mut())
        .map_err(Error::from)?;

    let mut total_sql = source_packages_base().into_boxed();
    total_sql = origin_filter.filter(total_sql);
    total_sql = identity_filter.filter(total_sql, source_packages::name, source_packages::version);

    let total = total_sql
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

    let mut sql = binary_packages_base().into_boxed();
    sql = origin_filter.filter(sql);
    sql = identity_filter.filter(sql, binary_packages::name, binary_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::BinaryPackage>(connection.as_mut())
        .map_err(Error::from)?;

    let mut total_sql = binary_packages_base().into_boxed();
    total_sql = origin_filter.filter(total_sql);
    total_sql = identity_filter.filter(total_sql, binary_packages::name, binary_packages::version);

    let total = total_sql
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
