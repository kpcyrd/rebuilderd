use crate::api::v1::util::auth;
use crate::api::v1::util::filters::{IntoIdentityFilter, IntoOriginFilter};
use crate::api::v1::util::pagination::PaginateDsl;
use crate::config::Config;
use crate::db::Pool;
use crate::models::NewQueued;
use crate::schema::{
    binary_packages, build_inputs, queue, rebuilds, source_packages, tag_rules, worker_tags,
    workers,
};
use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post};
use chrono::{Duration, NaiveDateTime, Utc};
use diesel::dsl::update;
use diesel::sql_types::{Nullable, Text};
use diesel::{
    BoolExpressionMethods, JoinOnDsl, QueryResult, SqliteConnection, TextExpressionMethods,
};
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl};
use diesel::{ExpressionMethods, SqliteExpressionMethods, define_sql_function};
use rebuilderd_common::api::v1::{
    BuildStatus, IdentityFilter, JobAssignment, OriginFilter, Page, PopQueuedJobRequest, Priority,
    QueueJobRequest, QueuedJob, QueuedJobArtifact, QueuedJobWithArtifacts, ResultPage,
};
use rebuilderd_common::config::PING_DEADLINE;
use rebuilderd_common::errors::*;
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
            build_inputs::next_retry,
            queue::priority,
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

    let records = queue_base()
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
        .order_by((
            queue::priority,
            diesel::dsl::date(queue::queued_at),
            sqlite_random(),
        ))
        .paginate(page.into_inner())
        .load::<QueuedJob>(connection.as_mut())
        .map_err(Error::from)?;

    let total = queue_base()
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
        .filter(
            origin_filter
                .clone()
                .into_filter(build_inputs::architecture),
        )
        .filter(
            identity_filter
                .clone()
                .into_filter(source_packages::name, source_packages::version),
        )
        .select(build_inputs::id)
        .into_boxed();

    if let Some(status) = queue_request.status {
        if status == BuildStatus::Unknown {
            sql = sql.filter(rebuilds::status.is_null());
        } else {
            sql = sql.filter(rebuilds::status.is(status));
        }
    }

    let build_input_ids = sql
        .get_results::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    let now = Utc::now();
    for build_input_id in build_input_ids {
        diesel::update(build_inputs::table)
            .filter(build_inputs::id.eq(build_input_id))
            .set(build_inputs::next_retry.eq((now - Duration::minutes(1)).naive_utc()))
            .execute(connection.as_mut())
            .map_err(Error::from)?;

        let new_queued_job = NewQueued {
            build_input_id,
            priority: queue_request.priority.unwrap_or(Priority::manual()),
            queued_at: now.naive_utc(),
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

    let ids = queue::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
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
        .select(queue::id)
        .load::<i32>(connection.as_mut())
        .map_err(Error::from)?;

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
        .filter(source_packages::id.is(id.into_inner()))
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

    diesel::delete(queue::table.filter(queue::id.is(id.into_inner())))
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
                .is(id.into_inner())
                .and(queue::worker.is(worker.id)),
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

define_sql_function! {
    #[sql_name = "RANDOM"]
    fn sqlite_random() -> Integer
}

define_sql_function! {
    #[sql_name = "COALESCE"]
    fn sqlite_coalesce(x: Nullable<Text>, y: Text) -> Text
}

/// Gets a queued job eligible for rebuilding by the worker. Eligibility is determined by the worker's supported
/// architectures, backends, and tags (if any).
fn get_eligible_job(
    connection: &mut SqliteConnection,
    worker_id: i32,
    max_retries: i32,
    supported_architectures: &Vec<String>,
    supported_backends: &Vec<String>,
) -> QueryResult<Option<QueuedJobWithArtifacts>> {
    let worker_tag_ids = worker_tags::table
        .filter(worker_tags::worker_id.eq(worker_id))
        .select(worker_tags::tag_id)
        .get_results::<i32>(connection)?;

    let mut base_query = queue_base()
        .left_join(tag_rules::table.on(
            source_packages::name.like(tag_rules::name_pattern).and(
                source_packages::version.like(sqlite_coalesce(tag_rules::version_pattern, "%")),
            ),
        ))
        .into_boxed();

    base_query = if !worker_tag_ids.is_empty() {
        // worker has tags - only offer tagged work applicable to us
        base_query.filter(tag_rules::id.eq_any(worker_tag_ids))
    } else {
        // worker has no tags - only offer untagged work
        base_query.filter(tag_rules::id.is_null())
    };

    // TODO: this can produce duplicates if multiple rules match a package. It's not a breaking issue, but it does make
    // those packages slightly more likely to be picked. Unfortunately, diesel doesn't let us combine into_boxed and
    // group_by, which would be the easy solution.
    let maybe_job = base_query
        .filter(queue::worker.is_null())
        .filter(
            build_inputs::next_retry
                .is_null()
                .or(build_inputs::next_retry.le(diesel::dsl::now)),
        )
        .filter(build_inputs::retries.lt(max_retries))
        .filter(build_inputs::architecture.eq_any(supported_architectures))
        .filter(build_inputs::backend.eq_any(supported_backends))
        .order_by((
            queue::priority,
            diesel::dsl::date(queue::queued_at),
            sqlite_random(),
        ))
        .first::<QueuedJob>(connection)
        .optional()?;

    if let Some(job) = maybe_job {
        let artifacts = queue::table
            .filter(queue::id.is(job.id))
            .inner_join(
                binary_packages::table
                    .on(queue::build_input_id.is(binary_packages::build_input_id)),
            )
            .select((
                binary_packages::name,
                binary_packages::version,
                binary_packages::architecture,
                binary_packages::artifact_url,
            ))
            .get_results::<QueuedJobArtifact>(connection)?;

        Ok(Some(QueuedJobWithArtifacts { job, artifacts }))
    } else {
        // TODO: offer untagged work to a worker with tags if no tagged work was available? Solve by adding a catch-all
        // rule to workers which should also accept untagged work?
        Ok(None)
    }
}

/// Reclaims stale jobs, making them available to workers again. A job is considered stale if it hasn't been pinged
/// within the [PING_DEADLINE]. Reclamation is done by removing the worker assignment of the job and resetting the start
/// and ping times.
fn reclaim_stale_jobs(connection: &mut SqliteConnection) -> QueryResult<()> {
    let now = Utc::now();
    let then = now - Duration::seconds(PING_DEADLINE);

    debug!("Clearing stale jobs last pinged before {then:?}...");

    update(
        queue::table.filter(
            queue::last_ping
                .is_not_null()
                .and(queue::last_ping.lt(then.naive_utc())),
        ),
    )
    .set((
        queue::worker.eq(None::<i32>),
        queue::started_at.eq(None::<NaiveDateTime>),
        queue::last_ping.eq(None::<NaiveDateTime>),
    ))
    .execute(connection)?;

    Ok(())
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

    // clear any stale jobs before we consider available jobs in the queue
    reclaim_stale_jobs(connection.as_mut()).map_err(Error::from)?;

    // see if we can dig up any available work for this worker
    let pop_request = request.into_inner();
    let supported_architectures = standardize_architectures(&pop_request.supported_architectures);

    debug!(
        "Trying to find work for worker {:?}... ({supported_architectures:?})",
        worker.name
    );

    let max_retries = cfg.schedule.max_retries().unwrap_or(i32::MAX);
    if let Some(record) =
        connection.transaction::<Option<QueuedJobWithArtifacts>, _, _>(|conn| {
            if let Some(record) = get_eligible_job(
                conn.as_mut(),
                worker.id,
                max_retries,
                &supported_architectures,
                &pop_request.supported_backends,
            )? {
                let now = Utc::now().naive_utc();
                let status = format!("working hard on {} {}", record.job.name, record.job.version);

                debug!(
                    "Marking job as taken for worker {:?}: {:?}",
                    worker.name, record
                );
                diesel::update(queue::table)
                    .filter(queue::id.is(record.job.id))
                    .set((
                        queue::started_at.eq(now),
                        queue::worker.eq(worker.id),
                        queue::last_ping.eq(now),
                    ))
                    .execute(conn)?;

                diesel::update(workers::table)
                    .filter(workers::id.is(worker.id))
                    .set((
                        workers::online.eq(true),
                        workers::last_ping.eq(now),
                        workers::status.eq(status),
                    ))
                    .execute(conn)?;

                Ok::<Option<QueuedJobWithArtifacts>, Error>(Some(record))
            } else {
                debug!(
                    "Could not find any item in work queue for worker {:?}",
                    worker.name
                );
                Ok(None)
            }
        })?
    {
        Ok(HttpResponse::Ok().json(JobAssignment::Rebuild(Box::new(record))))
    } else {
        Ok(HttpResponse::Ok().json(JobAssignment::Nothing))
    }
}
