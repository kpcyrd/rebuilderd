use crate::api::v1::util::auth;
use crate::api::v1::util::filters::DieselIdentityFilter;
use crate::api::v1::util::filters::DieselOriginFilter;
use crate::api::v1::util::pagination::PaginateDsl;
use crate::api::DEFAULT_QUEUE_PRIORITY;
use crate::config::Config;
use crate::db::Pool;
use crate::models::NewQueued;
use crate::schema::{binary_packages, build_inputs, queue, rebuilds, source_packages, workers};
use crate::web;
use actix_web::{delete, get, post, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use diesel::ExpressionMethods;
use diesel::{BoolExpressionMethods, JoinOnDsl};
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v1::{
    BuildStatus, IdentityFilter, JobAssignment, OriginFilter, Page, PopQueuedJobRequest,
    QueueJobRequest, QueuedJob, QueuedJobArtifact, QueuedJobWithArtifacts, ResultPage,
};
use rebuilderd_common::errors::Error;
use std::collections::HashSet;

#[diesel::dsl::auto_type]
fn queue_base() -> _ {
    queue::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .select((
            queue::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            build_inputs::architecture,
            build_inputs::backend,
            build_inputs::url,
            queue::queued_at,
            queue::started_at,
        ))
}

#[get("")]
pub async fn get_queued_jobs(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = queue_base().into_boxed();
    sql = origin_filter.filter(sql);

    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<QueuedJob>(connection.as_mut())
        .map_err(Error::from)?;

    let mut total_sql = queue_base().into_boxed();
    total_sql = origin_filter.filter(total_sql);

    if let Some(architecture) = &origin_filter.architecture {
        total_sql = total_sql.filter(build_inputs::architecture.eq(architecture));
    }

    total_sql = identity_filter.filter(total_sql, source_packages::name, source_packages::version);

    let total = total_sql
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[post("")]
pub async fn request_rebuild(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<QueueJobRequest>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let queue_request = request.into_inner();

    let origin_filter = OriginFilter {
        distribution: queue_request.distribution,
        release: queue_request.release,
        component: queue_request.component,
        architecture: queue_request.architecture,
    };

    let identity_filter = IdentityFilter {
        name: queue_request.name,
        version: queue_request.version,
    };

    let mut sql = source_packages::table
        .inner_join(build_inputs::table.left_join(rebuilds::table))
        .inner_join(binary_packages::table)
        .select(build_inputs::id)
        .into_boxed();

    // TODO: allow matching on binary package names and architectures
    sql = origin_filter.filter(sql);

    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    if let Some(status) = queue_request.status {
        if status == BuildStatus::Unknown {
            sql = sql.filter(rebuilds::status.is_null());
        } else {
            sql = sql.filter(rebuilds::status.eq(status));
        }
    }

    let build_input_ids = sql
        .get_results::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    for build_input_id in build_input_ids {
        let new_queued_job = NewQueued {
            build_input_id,
            priority: queue_request.priority.unwrap_or(DEFAULT_QUEUE_PRIORITY),
            queued_at: Utc::now().naive_utc(),
        };

        new_queued_job.upsert(connection.as_mut())?;
    }

    Ok(HttpResponse::NoContent())
}

#[delete("")]
pub async fn drop_queued_jobs(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = queue::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .select(queue::id)
        .into_boxed();

    sql = origin_filter.filter(sql);
    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    let ids = sql.load::<i32>(connection.as_mut()).map_err(Error::from)?;

    diesel::delete(queue::table.filter(queue::id.eq_any(ids)))
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent())
}

#[get("/{id}")]
pub async fn get_queued_job(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = queue_base()
        .filter(source_packages::id.eq(id.into_inner()))
        .get_result::<QueuedJob>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[delete("/{id}")]
pub async fn drop_queued_job(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    diesel::delete(queue::table.filter(queue::id.eq(id.into_inner())))
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent())
}

#[post("/{id}/ping")]
pub async fn ping_job(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let check_worker = auth::worker(&cfg, &req, connection.as_mut());
    if check_worker.is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let worker = check_worker?;

    let now = Utc::now();
    diesel::update(queue::table)
        .set(queue::last_ping.eq(now.naive_utc()))
        .filter(
            queue::id
                .eq(id.into_inner())
                .and(queue::worker.eq(worker.id)),
        )
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}

/// Standardizes architectures in the given list, expanding known aliases to other commonly-used architecture names.
/// Rust's builtin architecture variables don't always line up with what distros use (x86_64 vs amd64, for instance), so
/// we do some post-processing here.
fn standardize_architectures(architectures: &Vec<String>) -> Vec<String> {
    let mut new_architectures = HashSet::new();
    for architecture in architectures {
        match architecture.as_str() {
            "x86" => new_architectures.insert("i386".to_string()),
            "i386" => new_architectures.insert("x86".to_string()),
            "x86_64" => new_architectures.insert("amd64".to_string()),
            "amd64" => new_architectures.insert("x86_64".to_string()),
            "aarch64" => new_architectures.insert("arm64".to_string()),
            "arm64" => new_architectures.insert("aarch64".to_string()),
            "powerpc64" => new_architectures.insert("ppc64".to_string()),
            "ppc64" => new_architectures.insert("powerpc64".to_string()),
            _ => false,
        };

        new_architectures.insert(architecture.clone());
    }

    new_architectures.into_iter().collect()
}

#[post("/pop")]
pub async fn request_work(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<PopQueuedJobRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let check_worker = auth::worker(&cfg, &req, connection.as_mut());
    if check_worker.is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let worker = check_worker?;

    // TODO: retry logic?
    let pop_request = request.into_inner();
    let supported_architectures = standardize_architectures(&pop_request.supported_architectures);

    if let Some(record) =
        connection.transaction::<Option<QueuedJobWithArtifacts>, _, _>(|conn| {
            if let Some(record) = queue_base()
                .filter(queue::worker.is_null())
                .filter(build_inputs::architecture.eq_any(supported_architectures))
                .filter(build_inputs::backend.eq_any(pop_request.supported_backends))
                .first::<QueuedJob>(conn)
                .optional()
                .map_err(Error::from)?
            {
                let artifacts = queue::table
                    .filter(queue::id.eq(record.id))
                    .inner_join(
                        binary_packages::table
                            .on(queue::build_input_id.eq(binary_packages::build_input_id)),
                    )
                    .select((
                        binary_packages::name,
                        binary_packages::version,
                        binary_packages::architecture,
                        binary_packages::artifact_url,
                    ))
                    .get_results::<QueuedJobArtifact>(conn)
                    .map_err(Error::from)?;

                let now = Utc::now().naive_utc();
                let status = format!("working hard on {} {}", record.name, record.version);

                diesel::update(queue::table)
                    .filter(queue::id.eq(record.id))
                    .set((
                        queue::started_at.eq(now),
                        queue::worker.eq(worker.id),
                        queue::last_ping.eq(now),
                    ))
                    .execute(conn)
                    .map_err(Error::from)?;

                diesel::update(workers::table)
                    .filter(workers::id.eq(worker.id))
                    .set((
                        workers::online.eq(true),
                        workers::last_ping.eq(now),
                        workers::status.eq(status),
                    ))
                    .execute(conn)
                    .map_err(Error::from)?;

                Ok::<Option<QueuedJobWithArtifacts>, Error>(Some(QueuedJobWithArtifacts {
                    job: record,
                    artifacts,
                }))
            } else {
                Ok(None)
            }
        })?
    {
        Ok(HttpResponse::Ok().json(JobAssignment::Rebuild(Box::new(record))))
    } else {
        Ok(HttpResponse::Ok().json(JobAssignment::Nothing))
    }
}
