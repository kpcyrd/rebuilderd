use crate::api::v1::util::auth;
use crate::api::v1::util::filters::IntoOriginFilter;
use crate::api::v1::util::filters::{IntoFilter, IntoIdentityFilter};
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
use diesel::dsl::{delete, exists, not, select, update};
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::sql_types::Integer;
use diesel::{
    BoolExpressionMethods, Connection, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    OptionalExtension, QueryDsl, RunQueryDsl, SqliteConnection, SqliteExpressionMethods,
};
use rebuilderd_common::api::v1::{
    BuildStatus, FreshnessFilter, IdentityFilter, OriginFilter, PackageReport, Page, ResultPage,
    SourcePackageReport,
};
use rebuilderd_common::errors::Error;

use crate::api::v1::util::friends::build_input_friends;
use aliases::*;

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
    diesel::alias!(crate::schema::source_packages as sp: SourcePackagesAlias);
}

#[diesel::dsl::auto_type]
fn source_packages_base() -> _ {
    source_packages::table
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).is(build_inputs::id)))
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).is(build_inputs::id).and(
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
            source_packages::last_seen,
            source_packages::seen_in_last_sync,
        ))
}

#[diesel::dsl::auto_type]
fn binary_packages_base() -> _ {
    binary_packages::table
        .inner_join(source_packages::table)
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).is(build_inputs::id)))
        .left_join(
            rebuild_artifacts::table.on(rebuild_artifacts::rebuild_id
                .is(r1.field(rebuilds::id))
                .and(rebuild_artifacts::name.is(binary_packages::name))),
        )
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).is(build_inputs::id).and(
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
            source_packages::last_seen,
            source_packages::seen_in_last_sync,
        ))
}

/// Marks packages potentially affected by the given report as not having been
/// seen in the last sync.
///
/// The expectation is that all seen flags are set to false just before a sync
/// runs, which will flip the flag back to true for seen packages.
fn mark_scoped_packages_unseen(
    connection: &mut SqliteConnection,
    report: &PackageReport,
) -> Result<(), Error> {
    // mark all packages potentially affected by this report as unseen
    update(source_packages::table)
        .filter(
            source_packages::id.eq_any(
                build_inputs::table
                    .inner_join(
                        sp.on(sp
                            .field(source_packages::id)
                            .is(build_inputs::source_package_id)),
                    )
                    .filter(
                        sp.field(source_packages::distribution)
                            .is(&report.distribution),
                    )
                    .filter(sp.field(source_packages::release).is(&report.release))
                    .filter(sp.field(source_packages::component).is(&report.component))
                    .filter(build_inputs::architecture.is(&report.architecture))
                    .group_by(sp.field(source_packages::id))
                    .select(sp.field(source_packages::id)),
            ),
        )
        .set(source_packages::seen_in_last_sync.eq(false))
        .execute(connection)
        .map_err(Error::from)?;

    Ok(())
}

/// Drops enqueued rebuild jobs for source packages potentially affected by the
/// given report that were not seen in the last sync.
///
/// The expectation is that all jobs belonging to an unseen package are dropped
/// after a sync completes. Jobs that have already been picked up by a worker
/// are unaffected, however.
fn drop_unseen_scoped_jobs(
    connection: &mut SqliteConnection,
    report: &PackageReport,
) -> Result<(), Error> {
    delete(
        queue::table.filter(queue::worker.is_null()).filter(
            queue::build_input_id.eq_any(
                build_inputs::table
                    .inner_join(source_packages::table)
                    .filter(source_packages::distribution.is(&report.distribution))
                    .filter(source_packages::release.is(&report.release))
                    .filter(source_packages::component.is(&report.component))
                    .filter(build_inputs::architecture.is(&report.architecture))
                    .filter(source_packages::seen_in_last_sync.is(false))
                    .group_by(build_inputs::id)
                    .select(build_inputs::id),
            ),
        ),
    )
    .execute(connection)
    .map_err(Error::from)?;

    Ok(())
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

    let now = Utc::now();
    let report = request.into_inner();
    connection.transaction(|conn| {
        mark_scoped_packages_unseen(conn.as_mut(), &report)?;

        for package_report in &report.packages {
            // check if this package already exists - this is used later to determine if we should copy over existing build
            // results to this package.
            let is_new_package = is_new_package(&report, conn, package_report)?;

            let new_source_package = NewSourcePackage {
                name: package_report.name.clone(),
                version: package_report.version.clone(),
                distribution: report.distribution.clone(),
                release: report.release.clone(),
                component: report.component.clone(),
                last_seen: now.naive_utc(),
                seen_in_last_sync: true,
            };

            let source_package = new_source_package.upsert(conn.as_mut())?;

            let new_build_input = NewBuildInput {
                source_package_id: source_package.id,
                url: package_report.url.clone(),
                backend: report.distribution.clone(),
                architecture: report.architecture.clone(),
                retries: 0,
            };

            let build_input = new_build_input.upsert(conn.as_mut())?;

            for artifact_report in &package_report.artifacts {
                let new_binary_package = NewBinaryPackage {
                    source_package_id: source_package.id,
                    build_input_id: build_input.id,
                    name: artifact_report.name.clone(),
                    version: artifact_report.version.clone(),
                    architecture: report.architecture.clone(),
                    artifact_url: artifact_report.url.clone(),
                };

                new_binary_package.upsert(conn.as_mut())?;
            }

            if is_new_package {
                // in order to avoid additional rebuilds in distributions that copy existing packages between releases, we
                // want to also copy any results relevant to newly-imported versions. This only applies within a single
                // build backend and matches on the URL of the input artifact and its architecture.
                copy_existing_rebuilds(conn, &build_input)?;
            }

            let current_status = get_current_rebuild_status(conn, &build_input)?;
            let has_queued_friend = has_queued_friend(conn, &build_input)?;

            if current_status != BuildStatus::Good && !has_queued_friend {
                let priority = match current_status {
                    BuildStatus::Bad => DEFAULT_QUEUE_PRIORITY + 1,
                    _ => DEFAULT_QUEUE_PRIORITY,
                };

                let new_queued_job = NewQueued {
                    build_input_id: build_input.id,
                    priority,
                    queued_at: Utc::now().naive_utc(),
                };

                new_queued_job.upsert(conn.as_mut())?;
            }
        }

        drop_unseen_scoped_jobs(conn.as_mut(), &report)?;

        Ok::<(), Error>(())
    })?;

    Ok(HttpResponse::NoContent().finish())
}

fn is_new_package(
    report: &PackageReport,
    conn: &mut PooledConnection<ConnectionManager<SqliteConnectionWrap>>,
    source_package_report: &SourcePackageReport,
) -> Result<bool, Error> {
    let is_new_package = select(not(exists(
        source_packages::table
            .filter(source_packages::name.is(&source_package_report.name))
            .filter(source_packages::version.is(&source_package_report.version))
            .filter(source_packages::distribution.is(&report.distribution))
            .filter(source_packages::release.is(&report.release))
            .filter(source_packages::component.is(&report.component)),
    )))
    .get_result::<bool>(conn.as_mut())?;

    Ok(is_new_package)
}

fn get_current_rebuild_status(
    conn: &mut PooledConnection<ConnectionManager<SqliteConnectionWrap>>,
    build_input: &BuildInput,
) -> Result<BuildStatus, Error> {
    let current_status = rebuilds::table
        .filter(rebuilds::build_input_id.is(&build_input.id))
        .select(rebuilds::status)
        .order_by(rebuilds::built_at.desc())
        .get_result::<Option<BuildStatus>>(conn.as_mut())
        .optional()
        .map_err(Error::from)?
        .flatten()
        .unwrap_or(BuildStatus::Unknown);

    Ok(current_status)
}

fn has_queued_friend(
    conn: &mut PooledConnection<ConnectionManager<SqliteConnectionWrap>>,
    build_input: &BuildInput,
) -> Result<bool, Error> {
    let has_queued_friend = select(exists(
        queue::table
            .filter(
                queue::build_input_id.eq_any(
                    build_inputs::table
                        .filter(build_inputs::url.is(&build_input.url))
                        .filter(build_inputs::backend.is(&build_input.backend))
                        .filter(build_inputs::architecture.is(&build_input.architecture))
                        .select(build_inputs::id),
                ),
            )
            .select(queue::id),
    ))
    .get_result::<bool>(conn.as_mut())
    .map_err(Error::from)?;

    Ok(has_queued_friend)
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
            rebuilds::table.filter(rebuilds::build_input_id.is(existing_build_input)),
        ))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

        if has_existing_rebuild {
            let existing_rebuild_ids = rebuilds::table
                .filter(rebuilds::build_input_id.is(existing_build_input))
                .select(rebuilds::id)
                .get_results::<i32>(connection.as_mut())
                .map_err(Error::from)?;

            for existing_rebuild_id in existing_rebuild_ids {
                let new_rebuild_id = diesel::dsl::insert_into(rebuilds::table)
                    .values(
                        rebuilds::table
                            .filter(rebuilds::id.is(existing_rebuild_id))
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
                            .filter(rebuild_artifacts::rebuild_id.is(existing_rebuild_id))
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
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let records = source_packages_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(
            identity_filter
                .clone()
                .into_inner()
                .into_filter(source_packages::name, source_packages::version),
        )
        .filter(freshness_filter.clone().into_inner().into_filter())
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::SourcePackage>(connection.as_mut())
        .map_err(Error::from)?;

    let total = source_packages_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(
            identity_filter
                .clone()
                .into_inner()
                .into_filter(source_packages::name, source_packages::version),
        )
        .filter(freshness_filter.into_inner().into_filter())
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
        .filter(source_packages::id.is(id.into_inner()))
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
    freshness_filter: web::Query<FreshnessFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let records = binary_packages_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(binary_packages::architecture),
        )
        .filter(
            identity_filter
                .clone()
                .into_inner()
                .into_filter(binary_packages::name, binary_packages::version),
        )
        .filter(freshness_filter.clone().into_inner().into_filter())
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::BinaryPackage>(connection.as_mut())
        .map_err(Error::from)?;

    let total = binary_packages_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(freshness_filter.into_inner().into_filter())
        .filter(
            identity_filter
                .clone()
                .into_inner()
                .into_filter(binary_packages::name, binary_packages::version),
        )
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
        .filter(binary_packages::id.is(id.into_inner()))
        .get_result::<rebuilderd_common::api::v1::BinaryPackage>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}
