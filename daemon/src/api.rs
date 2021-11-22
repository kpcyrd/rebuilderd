use actix_web::{get, post, HttpRequest, HttpResponse, Responder, http};
use chrono::prelude::*;
use crate::auth;
use crate::config::Config;
use crate::dashboard::DashboardState;
use crate::db::Pool;
use crate::models;
use crate::sync;
use crate::web;
use diesel::SqliteConnection;
use rebuilderd_common::{PkgRelease, Status};
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden()
        .body("Authentication failed\n")
}

fn not_found() -> HttpResponse {
    HttpResponse::NotFound()
        .body("Not found\n")
}

fn not_modified() -> HttpResponse {
    HttpResponse::NotModified().body("")
}

pub fn header<'a>(req: &'a HttpRequest, key: &str) -> Result<&'a str> {
    let value = req.headers().get(key)
        .ok_or_else(|| format_err!("Missing header"))?
        .to_str()
        .context("Failed to decode header value")?;
    Ok(value)
}

fn modified_since_duration(req: &HttpRequest, datetime: DateTime<Utc>) -> Option<chrono::Duration> {
    header(req, http::header::IF_MODIFIED_SINCE.as_str()).ok()
        .and_then(|value| chrono::DateTime::parse_from_rfc2822(value).ok())
        .map(|value| value.signed_duration_since(datetime))
}

#[get("/api/v0/workers")]
pub async fn list_workers(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let connection = pool.get().map_err(Error::from)?;
    models::Worker::mark_stale_workers_offline(connection.as_ref())?;
    let workers = models::Worker::list(connection.as_ref())?;
    Ok(HttpResponse::Ok().json(workers))
}

// this route is configured in src/main.rs so we can reconfigure the json extractor
// #[post("/api/v0/job/sync")]
pub async fn sync_work(
    req: HttpRequest,
    cfg: web::Data<Config>,
    import: web::Json<SuiteImport>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let import = import.into_inner();
    let connection = pool.get().map_err(Error::from)?;

    sync::run(import, connection.as_ref())?;

    Ok(HttpResponse::Ok().json(JobAssignment::Nothing))
}

fn opt_filter(this: &str, filter: Option<&str>) -> bool {
    if let Some(filter) = filter {
        if this != filter {
            return true;
        }
    }
    false
}

#[get("/api/v0/pkgs/list")]
pub async fn list_pkgs(
    req: HttpRequest,
    query: web::Query<ListPkgs>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;
    let mut builder = HttpResponse::Ok();

    // Set Last-Modified header to the most recent build package time
    // If If-Modified-Since header is set, compare it to the latest built time.
    if let Some(latest_built_at) = models::Package::most_recent_built_at(connection.as_ref())? {
        let latest_built_at = DateTime::from_utc(latest_built_at, Utc);
        if let Some(duration) = modified_since_duration(&req, latest_built_at) {
            if duration.num_seconds() >= 0 {
                return Ok(not_modified());
            }
        }
        let latest_built_at = SystemTime::from(latest_built_at);
        builder.set(http::header::LastModified(latest_built_at.into()));
    }

    let mut pkgs = Vec::<PkgRelease>::new();
    for pkg in models::Package::list(connection.as_ref())? {
        if opt_filter(&pkg.name, query.name.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.status, query.status.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.distro, query.distro.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.suite, query.suite.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.architecture, query.architecture.as_deref()) {
            continue;
        }

        pkgs.push(pkg.into_api_item()?);
    }

    Ok(builder.json(pkgs))
}

#[post("/api/v0/queue/list")]
pub async fn list_queue(
    query: web::Json<ListQueue>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

    models::Queued::free_stale_jobs(connection.as_ref())?;
    let queue = models::Queued::list(query.limit, connection.as_ref())?;
    let queue: Vec<QueueItem> = queue.into_iter()
        .map(|x| x.into_api_item(connection.as_ref()))
        .collect::<Result<_>>()?;

    let now = Utc::now().naive_utc();

    Ok(HttpResponse::Ok().json(QueueList {
        now,
        queue,
    }))
}

fn get_worker_from_request(req: &HttpRequest, cfg: &Config, connection: &SqliteConnection) -> web::Result<models::Worker> {
    let key = header(req, WORKER_KEY_HEADER)
        .context("Failed to get worker key")?;

    let ip = if let Some(real_ip_header) = &cfg.real_ip_header {
        let ip = header(req, real_ip_header)
            .context("Failed to locate real ip header")?;
        ip.parse::<IpAddr>()
            .context("Can't parse real ip header as ip address")?
    } else {
        let ci = req.peer_addr()
            .ok_or_else(|| format_err!("Can't determine client ip"))?;
        ci.ip()
    };
    debug!("detected worker ip for {:?} as {}", key, ip);

    if let Some(mut worker) = models::Worker::get(key, connection)? {
        worker.bump_last_ping(&ip);
        Ok(worker)
    } else {
        let worker = models::NewWorker::new(key.to_string(), ip, None);
        worker.insert(connection)?;
        get_worker_from_request(req, cfg, connection)
    }
}

#[post("/api/v0/queue/push")]
pub async fn push_queue(
    req: HttpRequest,
    cfg: web::Data<Config>,
    query: web::Json<PushQueue>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let query = query.into_inner();
    let connection = pool.get().map_err(Error::from)?;

    debug!("searching pkg: {:?}", query);
    let pkgs = models::Package::get_by(&query.name, &query.distro, &query.suite, query.architecture.as_deref(), connection.as_ref())?;

    for pkg in pkgs {
        debug!("found pkg: {:?}", pkg);
        let version = query.version.as_ref().unwrap_or(&pkg.version);

        let item = models::NewQueued::new(pkg.id, version.to_string(), query.distro.to_string(), query.priority);
        debug!("adding to queue: {:?}", item);
        item.insert(connection.as_ref())?;
    }

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/queue/pop")]
pub async fn pop_queue(
    req: HttpRequest,
    cfg: web::Data<Config>,
    query: web::Json<WorkQuery>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_ref())?;

    models::Queued::free_stale_jobs(connection.as_ref())?;
    let (resp, status) = if let Some(item) = models::Queued::pop_next(worker.id, &query.supported_backends, connection.as_ref())? {


        // TODO: claim item correctly


        let status = format!("working hard on {} {}", item.pkgbase.name, item.pkgbase.version);
        (JobAssignment::Rebuild(Box::new(item)), Some(status))
    } else {
        (JobAssignment::Nothing, None)
    };

    worker.status = status;
    worker.update(connection.as_ref())?;

    Ok(HttpResponse::Ok().json(resp))
}

#[post("/api/v0/queue/drop")]
pub async fn drop_from_queue(
    req: HttpRequest,
    cfg: web::Data<Config>,
    query: web::Json<DropQueueItem>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let query = query.into_inner();
    let connection = pool.get().map_err(Error::from)?;

    let pkgbases = models::PkgBase::get_by(&query.name, &query.distro, &query.suite, None, query.architecture.as_deref(), connection.as_ref())?;
    let pkgbases = pkgbases.iter()
        .map(|p| p.id)
        .collect::<Vec<_>>();

    models::Queued::drop_for_pkgbases(&pkgbases, connection.as_ref())?;

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/pkg/requeue")]
pub async fn requeue_pkgbase(
    req: HttpRequest,
    cfg: web::Data<Config>,
    query: web::Json<RequeueQuery>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut pkg_ids = Vec::new();
    let mut pkgbase_ids = HashSet::new();
    for pkg in models::Package::list(connection.as_ref())? {
        // TODO: this should be filtered in the database
        if opt_filter(&pkg.name, query.name.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.status, query.status.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.distro, query.distro.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.suite, query.suite.as_deref()) {
            continue;
        }
        if opt_filter(&pkg.architecture, query.architecture.as_deref()) {
            continue;
        }

        debug!("Adding pkgbase to be requeued for {:?} {:?}: pkgbase={:?}", pkg.name, pkg.version, pkg.base_id);
        pkg_ids.push(pkg.id);
        if let Some(base_id) = pkg.base_id {
            pkgbase_ids.insert(base_id);
        }
    }

    let pkgbase_ids = pkgbase_ids.into_iter().collect::<Vec<_>>();
    let pkgbases = models::PkgBase::get_id_list(&pkgbase_ids, connection.as_ref())?;

    let to_be_queued = pkgbases.into_iter()
        .map(|pkgbase| {
            models::NewQueued::new(pkgbase.id,
                                   pkgbase.version.to_string(),
                                   pkgbase.distro,
                                   query.priority)
        })
        .collect::<Vec<_>>();

    models::Queued::insert_batch(&to_be_queued, connection.as_ref())?;

    if query.reset {
        models::Package::reset_status_for_requeued_list(&pkg_ids, connection.as_ref())?;
    }

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/build/ping")]
pub async fn ping_build(
    req: HttpRequest,
    cfg: web::Data<Config>,
    item: web::Json<QueueItem>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let connection = pool.get().map_err(Error::from)?;

    let worker = get_worker_from_request(&req, &cfg, connection.as_ref())?;
    debug!("ping from worker: {:?}", worker);
    let mut item = models::Queued::get_id(item.id, connection.as_ref())?;
    debug!("trying to ping item: {:?}", item);

    if item.worker_id != Some(worker.id) {
        return Err(anyhow!("Trying to write to item we didn't assign").into());
    }

    debug!("updating database (item)");
    item.ping_job(connection.as_ref())?;
    debug!("updating database (worker)");
    worker.update(connection.as_ref())?;
    debug!("successfully pinged job");

    Ok(HttpResponse::Ok().json(()))
}

// this route is configured in src/main.rs so we can reconfigure the json extractor
// #[post("/api/v0/build/report")]
pub async fn report_build(
    req: HttpRequest,
    cfg: web::Data<Config>,
    report: web::Json<BuildReport>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_ref())?;
    let queue_item = models::Queued::get_id(report.queue.id, connection.as_ref())?;
    let mut pkgbase = models::PkgBase::get_id(queue_item.pkgbase_id, connection.as_ref())?;

    let mut needs_retry = false;
    for (artifact, rebuild) in &report.rebuilds {
        let mut packages = models::Package::get_by(&artifact.name,
                                               &pkgbase.distro,
                                               &pkgbase.suite,
                                               None,
                                               connection.as_ref())?;

        packages.retain(|x| x.base_id == Some(pkgbase.id));
        if packages.len() != 1 {
            error!("rebuilt artifact didn't match a unique package in database. matches={:?} instead of 1", packages.len());
            continue;
        }
        let mut pkg = packages.remove(0);

        // adding build to package
        let build = models::NewBuild::from_api(rebuild);
        let build_id = build.insert(connection.as_ref())?;
        pkg.build_id = Some(build_id);

        pkg.status = match rebuild.status {
            BuildStatus::Good => Status::Good.to_string(),
            _ => Status::Bad.to_string(),
        };
        pkg.built_at = Some(Utc::now().naive_utc());

        pkg.has_diffoscope = rebuild.diffoscope.is_some();
        pkg.has_attestation = rebuild.attestation.is_some();

        if rebuild.status != BuildStatus::Good {
            needs_retry = true;
        }

        pkg.update(connection.as_ref())?;
    }

    // update pkgbase
    if needs_retry {
        pkgbase.retries += 1;
        pkgbase.schedule_retry(cfg.schedule.retry_delay_base());
    } else {
        pkgbase.next_retry = None;
    }
    pkgbase.update(connection.as_ref())?;

    // cleanup queue item and worker status
    queue_item.delete(connection.as_ref())?;
    worker.status = None;
    worker.update(connection.as_ref())?;

    Ok(HttpResponse::Ok().json(()))
}

#[get("/api/v0/builds/{id}/log")]
pub async fn get_build_log(
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_ref()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    let resp = HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .header("Content-Security-Policy", "default-src 'none'")
        .body(build.build_log);
    Ok(resp)
}

#[get("/api/v0/builds/{id}/attestation")]
pub async fn get_attestation(
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_ref()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    if let Some(attestation) = build.attestation {
        let resp = HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .header("X-Content-Type-Options", "nosniff")
            .header("Content-Security-Policy", "default-src 'none'")
            .body(attestation);
        Ok(resp)
    } else {
        Ok(not_found())
    }
}

#[get("/api/v0/builds/{id}/diffoscope")]
pub async fn get_diffoscope(
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_ref()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    if let Some(diffoscope) = build.diffoscope {
        let resp = HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .header("X-Content-Type-Options", "nosniff")
            .header("Content-Security-Policy", "default-src 'none'")
            .body(diffoscope);
        Ok(resp)
    } else {
        Ok(not_found())
    }
}

#[get("/api/v0/dashboard")]
pub async fn get_dashboard(
    pool: web::Data<Pool>,
    lock: web::Data<Arc<RwLock<DashboardState>>>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;
    let stale = {
        let state = lock.read().unwrap();
        !state.is_fresh()
    };
    if stale {
        let mut state = lock.write().unwrap();
        debug!("Updating cached dashboard");
        state.update(connection.as_ref())?;
    }
    let state = lock.read().unwrap();
    let resp = state.get_response()?;
    Ok(HttpResponse::Ok().json(resp))
}
