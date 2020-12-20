use actix_web::{get, post, HttpRequest, HttpResponse, Responder};
use crate::web;
use chrono::prelude::*;
use crate::auth;
use crate::config::Config;
use crate::models;
use crate::db::Pool;
use crate::sync;
use diesel::SqliteConnection;
use rebuilderd_common::{Status, PkgRelease};
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const DASHBOARD_UPDATE_INTERVAL: u64 = 1; // seconds

fn forbidden() -> web::Result<HttpResponse> {
    Ok(HttpResponse::Forbidden()
        .body("Authentication failed\n"))
}

fn not_found() -> web::Result<HttpResponse> {
    Ok(HttpResponse::NotFound()
        .body("Not found\n"))
}

pub fn header<'a>(req: &'a HttpRequest, key: &str) -> Result<&'a str> {
    let value = req.headers().get(key)
        .ok_or_else(|| format_err!("Missing header"))?
        .to_str()
        .context("Failed to decode header value")?;
    Ok(value)
}

#[get("/api/v0/workers")]
pub async fn list_workers(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return forbidden();
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
        return forbidden();
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
    query: web::Query<ListPkgs>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

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

    Ok(HttpResponse::Ok().json(pkgs))
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
        get_worker_from_request(req, &cfg, connection)
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
        return forbidden();
    }

    let query = query.into_inner();
    let connection = pool.get().map_err(Error::from)?;

    debug!("searching pkg: {:?}", query);
    let pkgs = models::Package::get_by(&query.name, &query.distro, &query.suite, query.architecture.as_deref(), connection.as_ref())?;

    for pkg in pkgs {
        debug!("found pkg: {:?}", pkg);
        let version = query.version.as_ref().unwrap_or(&pkg.version);

        let item = models::NewQueued::new(pkg.id, version.to_string(), query.priority);
        debug!("adding to queue: {:?}", item);
        item.insert(connection.as_ref())?;
    }

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/queue/pop")]
pub async fn pop_queue(
    req: HttpRequest,
    cfg: web::Data<Config>,
    _query: web::Json<WorkQuery>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return forbidden();
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_ref())?;

    models::Queued::free_stale_jobs(connection.as_ref())?;
    let (resp, status) = if let Some(item) = models::Queued::pop_next(worker.id, connection.as_ref())? {


        // TODO: claim item correctly


        let status = format!("working hard on {} {}", item.package.name, item.package.version);
        (JobAssignment::Rebuild(item), Some(status))
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
        return forbidden();
    }

    let query = query.into_inner();
    let connection = pool.get().map_err(Error::from)?;

    let pkgs = models::Package::get_by(&query.name, &query.distro, &query.suite, query.architecture.as_deref(), connection.as_ref())?;
    let pkgs = pkgs.iter()
        .map(|p| p.id)
        .collect::<Vec<_>>();

    models::Queued::drop_for_pkgs(&pkgs, connection.as_ref())?;

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/pkg/requeue")]
pub async fn requeue_pkg(
    req: HttpRequest,
    cfg: web::Data<Config>,
    query: web::Json<RequeueQuery>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return forbidden();
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut pkgs = Vec::new();
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

        debug!("pkg is going to be requeued: {:?} {:?}", pkg.name, pkg.version);
        pkgs.push((pkg.id, pkg.version));
    }

    // TODO: use queue_batch after https://github.com/diesel-rs/diesel/pull/1884 is released
    // models::Queued::queue_batch(&pkgs, connection.as_ref())?;
    for (id, version) in &pkgs {
        let q = models::NewQueued::new(*id, version.to_string(), query.priority);
        q.insert(connection.as_ref()).ok();
    }

    if query.reset {
        let reset = pkgs.into_iter()
            .map(|x| x.0)
            .collect::<Vec<_>>();
        models::Package::reset_status_for_requeued_list(&reset, connection.as_ref())?;
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
        return forbidden();
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
        return forbidden();
    }

    let connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_ref())?;
    let item = models::Queued::get_id(report.queue.id, connection.as_ref())?;
    let mut pkg = models::Package::get_id(item.package_id, connection.as_ref())?;

    let build = models::NewBuild::from_api(&report);
    let build = build.insert(&connection.as_ref())?;
    pkg.build_id = Some(build);

    if report.rebuild.status == BuildStatus::Good {
        pkg.next_retry = None;
    } else {
        pkg.schedule_retry(cfg.schedule.retry_delay_base());
    }
    pkg.update_status_safely(&report.rebuild, connection.as_ref())?;
    item.delete(connection.as_ref())?;

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
        Err(_) => return not_found(),
    };

    let resp = HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .header("Content-Security-Policy", "default-src 'none'")
        .body(build.build_log);
    Ok(resp)
}

#[get("/api/v0/builds/{id}/diffoscope")]
pub async fn get_diffoscope(
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_ref()) {
        Ok(build) => build,
        Err(_) => return not_found(),
    };

    if let Some(diffoscope) = build.diffoscope {
        let resp = HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .header("X-Content-Type-Options", "nosniff")
            .header("Content-Security-Policy", "default-src 'none'")
            .body(diffoscope);
        Ok(resp)
    } else {
        not_found()
    }
}

#[derive(Debug)]
pub struct DashboardState {
    response: Option<DashboardResponse>,
    last_update: Instant,
}

impl DashboardState {
    pub fn new() -> DashboardState {
        DashboardState {
            response: None,
            last_update: Instant::now(),
        }
    }

    pub fn is_fresh(&self) -> bool {
        if self.response.is_some() {
            self.last_update.elapsed() < Duration::from_secs(DASHBOARD_UPDATE_INTERVAL)
        } else {
            false
        }
    }

    pub fn update(&mut self, connection: &diesel::SqliteConnection) -> Result<()> {
        const LIMIT: Option<i64> = Some(25);

        models::Queued::free_stale_jobs(connection)?;
        // TODO: this should list jobs that are specifically active
        let queue = models::Queued::list(LIMIT, connection)?;
        let pkgs = models::Package::list(connection)?;

        let mut suites = HashMap::new();
        for pkg in pkgs {
            if !suites.contains_key(&pkg.suite) {
                suites.insert(pkg.suite.clone(), SuiteStats::default());
            }
            if let Some(stats) = suites.get_mut(&pkg.suite) {
                if let Ok(status) = pkg.status.parse() {
                    match status {
                        Status::Good => stats.good += 1,
                        Status::Unknown => stats.unknown += 1,
                        Status::Bad => stats.bad += 1,
                    }
                }
            }
        }

        let mut active_builds = Vec::new();
        for item in queue {
            if item.started_at.is_some() {
                let item = item.into_api_item(connection)?;
                active_builds.push(item);
            }
        }

        let now = Utc::now().naive_utc();
        self.response = Some(DashboardResponse {
            suites,
            active_builds,
            now,
        });
        self.last_update = Instant::now();
        Ok(())
    }

    pub fn get_response(&self) -> Result<&DashboardResponse> {
        if let Some(resp) =&self.response {
            Ok(&resp)
        } else {
            bail!("No cached state")
        }
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
