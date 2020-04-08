use actix_web::{get, post, web, HttpRequest, Responder};
use rebuilderd_common::errors::*;
use rebuilderd_common::Status;
use crate::models;
use rebuilderd_common::api::*;
use rebuilderd_common::PkgRelease;
use crate::db::Pool;
use crate::sync;
use diesel::SqliteConnection;

#[get("/api/v0/workers")]
pub async fn list_workers(
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();
    // TODO: fix unwrap
    models::Worker::mark_stale_workers_offline(connection.as_ref()).unwrap();
    let workers = models::Worker::list(connection.as_ref()).unwrap();
    web::Json(workers)
}

// #[post("/api/v0/job/sync")]
pub async fn sync_work(
    import: web::Json<SuiteImport>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let import = import.into_inner();
    let connection = pool.get().unwrap();

    sync::run(import, connection.as_ref()).unwrap();

    web::Json(JobAssignment::Nothing)
}

fn opt_filter(this: &String, filter: &Option<String>) -> bool {
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
) -> impl Responder {
    let connection = pool.get().unwrap();

    let mut pkgs = Vec::<PkgRelease>::new();
    for pkg in models::Package::list(connection.as_ref()).unwrap() {
        if opt_filter(&pkg.name, &query.name) {
            continue;
        }
        if opt_filter(&pkg.status, &query.status) {
            continue;
        }
        if opt_filter(&pkg.distro, &query.distro) {
            continue;
        }
        if opt_filter(&pkg.suite, &query.suite) {
            continue;
        }
        if opt_filter(&pkg.architecture, &query.architecture) {
            continue;
        }

        let status = pkg.status.parse::<Status>().unwrap();

        pkgs.push(PkgRelease {
             name: pkg.name,
             version: pkg.version,
             status: status,
             distro: pkg.distro,
             suite: pkg.suite,
             architecture: pkg.architecture,
             url: pkg.url,
        });
    }

    web::Json(pkgs)
}

#[post("/api/v0/queue/list")]
pub async fn list_queue(
    query: web::Json<ListQueue>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();

    models::Queued::free_stale_jobs(connection.as_ref()).unwrap();
    let queue = models::Queued::list(query.limit, connection.as_ref()).unwrap();
    let queue: Vec<QueueItem> = queue.into_iter()
        .map(|x| x.into_api_item(connection.as_ref()))
        .collect::<Result<_>>().unwrap();

    web::Json(queue)
}

fn get_worker_from_request(req: &HttpRequest, connection: &SqliteConnection) -> Result<models::Worker> {
    let key = req.headers().get(WORKER_HEADER)
        .ok_or_else(|| format_err!("Missing worker header"))?
        .to_str()
        .context("Failed to decode worker header")?;

    let ci = req.peer_addr()
        .ok_or_else(|| format_err!("Can't determine client ip"))?;

    if let Some(mut worker) = models::Worker::get(key, connection)? {
        worker.bump_last_ping();
        Ok(worker)
    } else {
        let worker = models::NewWorker::new(key.to_string(), ci.ip(), None);
        worker.insert(connection)?;
        get_worker_from_request(req, connection)
    }
}

#[post("/api/v0/queue/pop")]
pub async fn pop_queue(
    req: HttpRequest,
    _query: web::Json<WorkQuery>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();

    let mut worker = get_worker_from_request(&req, connection.as_ref()).unwrap();

    models::Queued::free_stale_jobs(connection.as_ref()).unwrap();
    let (resp, status) = if let Some(item) = models::Queued::pop_next(worker.id, connection.as_ref()).unwrap() {


        // TODO: claim item correctly


        let status = format!("working hard on {} {}", item.package.name, item.package.version);
        (JobAssignment::Rebuild(item), Some(status))
    } else {
        (JobAssignment::Nothing, None)
    };

    worker.status = status;
    worker.update(connection.as_ref()).unwrap();
    // let worker = models::NewWorker::new(query.into_inner(), ci.ip(), status);
    // worker.insert(connection.as_ref()).unwrap();

    web::Json(resp)
}

#[post("/api/v0/build/ping")]
pub async fn ping_build(
    req: HttpRequest,
    item: web::Json<QueueItem>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();

    let worker = get_worker_from_request(&req, connection.as_ref()).unwrap();
    let mut item = models::Queued::get_id(item.id, connection.as_ref()).unwrap();

    if item.worker_id != Some(worker.id) {
        panic!("Trying to write to item we didn't assign")
    }

    item.ping_job(connection.as_ref()).unwrap();
    worker.update(connection.as_ref()).unwrap();

    web::Json(())
}

#[post("/api/v0/build/report")]
pub async fn report_build(
    req: HttpRequest,
    report: web::Json<BuildReport>,
    pool: web::Data<Pool>,
) -> impl Responder {
    let connection = pool.get().unwrap();

    let mut worker = get_worker_from_request(&req, connection.as_ref()).unwrap();
    let item = models::Queued::get_id(report.queue.id, connection.as_ref()).unwrap();
    let mut pkg = models::Package::get_id(item.package_id, connection.as_ref()).unwrap();
    let status = match report.status {
        BuildStatus::Good => Status::Good,
        _ => Status::Bad,
    };
    pkg.update_status_safely(status, connection.as_ref()).unwrap();
    item.delete(connection.as_ref()).unwrap();
    worker.status = None; // TODO: this might not set to null
    worker.update(connection.as_ref()).unwrap();

    web::Json(())
}
