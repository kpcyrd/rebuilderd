use crate::attestation::Attestation;
use crate::auth;
use crate::config::Config;
use crate::dashboard::DashboardState;
use crate::db::Pool;
use crate::models;
use crate::sync;
use crate::util::{is_zstd_compressed, zstd_compress, zstd_decompress};
use crate::web;
use actix_web::http::header::{AcceptEncoding, ContentEncoding, Encoding, Header};
use actix_web::{get, http, post, HttpRequest, HttpResponse, Responder};
use chrono::prelude::*;
use diesel::SqliteConnection;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::{PkgRelease, Status};
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().body("Authentication failed\n")
}

fn not_found() -> HttpResponse {
    HttpResponse::NotFound().body("Not found\n")
}

fn not_modified() -> HttpResponse {
    HttpResponse::NotModified().body("")
}

pub fn header<'a>(req: &'a HttpRequest, key: &str) -> Result<&'a str> {
    let value = req
        .headers()
        .get(key)
        .ok_or_else(|| format_err!("Missing header"))?
        .to_str()
        .context("Failed to decode header value")?;
    Ok(value)
}

fn modified_since_duration(req: &HttpRequest, datetime: DateTime<Utc>) -> Option<chrono::Duration> {
    header(req, http::header::IF_MODIFIED_SINCE.as_str())
        .ok()
        .and_then(|value| chrono::DateTime::parse_from_rfc2822(value).ok())
        .map(|value| value.signed_duration_since(datetime))
}

async fn forward_compressed_data(
    request: HttpRequest,
    content_type: &str,
    data: Vec<u8>,
) -> web::Result<HttpResponse> {
    let mut builder = HttpResponse::Ok();

    builder
        .content_type(content_type)
        .append_header(("X-Content-Type-Options", "nosniff"))
        .append_header(("Content-Security-Policy", "default-src 'none'"));

    if is_zstd_compressed(data.as_slice()) {
        let client_supports_zstd = AcceptEncoding::parse(&request)
            .ok()
            .and_then(|a| a.negotiate([Encoding::zstd()].iter()))
            .map(|e| e == Encoding::zstd())
            .unwrap_or(false);

        if client_supports_zstd {
            builder.insert_header(ContentEncoding::Zstd);

            let resp = builder.body(data);
            Ok(resp)
        } else {
            let decoded_log = zstd_decompress(data.as_slice())
                .await
                .map_err(Error::from)?;

            let resp = builder.body(decoded_log);
            Ok(resp)
        }
    } else {
        let resp = builder.body(data);
        Ok(resp)
    }
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

    let mut connection = pool.get().map_err(Error::from)?;
    models::Worker::mark_stale_workers_offline(connection.as_mut())?;
    let workers = models::Worker::list(connection.as_mut())?;
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
    let mut connection = pool.get().map_err(Error::from)?;

    sync::run(import, connection.as_mut())?;

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
    let mut connection = pool.get().map_err(Error::from)?;
    let mut builder = HttpResponse::Ok();

    // Set Last-Modified header to the most recent build package time
    // If If-Modified-Since header is set, compare it to the latest built time.
    if let Some(latest_built_at) = models::Package::most_recent_built_at(connection.as_mut())? {
        let latest_built_at = DateTime::from_naive_utc_and_offset(latest_built_at, Utc);
        if let Some(duration) = modified_since_duration(&req, latest_built_at) {
            if duration.num_seconds() >= 0 {
                return Ok(not_modified());
            }
        }
        let latest_built_at = SystemTime::from(latest_built_at);
        builder.insert_header(http::header::LastModified(latest_built_at.into()));
    }

    let mut pkgs = Vec::<PkgRelease>::new();
    for pkg in models::Package::list(connection.as_mut())? {
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
    let mut connection = pool.get().map_err(Error::from)?;

    models::Queued::free_stale_jobs(connection.as_mut())?;
    let queue = models::Queued::list(query.limit, connection.as_mut())?;
    let queue: Vec<QueueItem> = queue
        .into_iter()
        .map(|x| x.into_api_item(connection.as_mut()))
        .collect::<Result<_>>()?;

    let now = Utc::now().naive_utc();

    Ok(HttpResponse::Ok().json(QueueList { now, queue }))
}

fn get_worker_from_request(
    req: &HttpRequest,
    cfg: &Config,
    connection: &mut SqliteConnection,
) -> web::Result<models::Worker> {
    let key = header(req, WORKER_KEY_HEADER).context("Failed to get worker key")?;

    let ip = if let Some(real_ip_header) = &cfg.real_ip_header {
        let ip = header(req, real_ip_header).context("Failed to locate real ip header")?;
        ip.parse::<IpAddr>()
            .context("Can't parse real ip header as ip address")?
    } else {
        let ci = req
            .peer_addr()
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
    let mut connection = pool.get().map_err(Error::from)?;

    debug!("searching pkg: {:?}", query);
    let pkgs = models::Package::get_by(
        &query.name,
        &query.distro,
        &query.suite,
        query.architecture.as_deref(),
        connection.as_mut(),
    )?;

    for pkg in pkgs {
        debug!("found pkg: {:?}", pkg);

        let pkgbase = models::PkgBase::get_id(pkg.pkgbase_id, connection.as_mut())?;
        let item =
            models::NewQueued::new(pkgbase.id, pkgbase.version, pkgbase.distro, query.priority);

        debug!("adding to queue: {:?}", item);
        if let Err(err) = item.insert(connection.as_mut()) {
            error!("failed to queue item: {:#?}", err);
        }
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

    let mut connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_mut())?;

    models::Queued::free_stale_jobs(connection.as_mut())?;
    let (resp, status) = if let Some(item) =
        models::Queued::pop_next(worker.id, &query.supported_backends, connection.as_mut())?
    {
        // TODO: claim item correctly

        let status = format!(
            "working hard on {} {}",
            item.pkgbase.name, item.pkgbase.version
        );
        (JobAssignment::Rebuild(Box::new(item)), Some(status))
    } else {
        (JobAssignment::Nothing, None)
    };

    worker.status = status;
    worker.update(connection.as_mut())?;

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
    let mut connection = pool.get().map_err(Error::from)?;

    let pkgbases = models::PkgBase::get_by(
        &query.name,
        &query.distro,
        &query.suite,
        None,
        query.architecture.as_deref(),
        connection.as_mut(),
    )?;
    let pkgbases = pkgbases.iter().map(|p| p.id).collect::<Vec<_>>();

    models::Queued::drop_for_pkgbases(&pkgbases, connection.as_mut())?;

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

    let mut connection = pool.get().map_err(Error::from)?;

    let mut pkg_ids = Vec::new();
    let mut pkgbase_ids = HashSet::new();
    for pkg in models::Package::list(connection.as_mut())? {
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

        debug!(
            "Adding pkgbase to be requeued for {:?} {:?}: pkgbase={:?}",
            pkg.name, pkg.version, pkg.pkgbase_id
        );
        pkg_ids.push(pkg.id);
        pkgbase_ids.insert(pkg.pkgbase_id);
    }

    let pkgbase_ids = pkgbase_ids.into_iter().collect::<Vec<_>>();
    let pkgbases = models::PkgBase::get_id_list(&pkgbase_ids, connection.as_mut())?;

    let to_be_queued = pkgbases
        .into_iter()
        .map(|pkgbase| {
            models::NewQueued::new(
                pkgbase.id,
                pkgbase.version.to_string(),
                pkgbase.distro,
                query.priority,
            )
        })
        .collect::<Vec<_>>();

    models::Queued::insert_batch(&to_be_queued, connection.as_mut())?;

    if query.reset {
        models::Package::reset_status_for_requeued_list(&pkg_ids, connection.as_mut())?;
    }

    Ok(HttpResponse::Ok().json(()))
}

#[post("/api/v0/build/ping")]
pub async fn ping_build(
    req: HttpRequest,
    cfg: web::Data<Config>,
    item: web::Json<PingRequest>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let worker = get_worker_from_request(&req, &cfg, connection.as_mut())?;
    debug!("ping from worker: {:?}", worker);
    let mut item = models::Queued::get_id(item.queue_id, connection.as_mut())?;
    debug!("trying to ping item: {:?}", item);

    if item.worker_id != Some(worker.id) {
        return Err(anyhow!("Trying to write to item we didn't assign").into());
    }

    debug!("updating database (item)");
    item.ping_job(connection.as_mut())?;
    debug!("updating database (worker)");
    worker.update(connection.as_mut())?;
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

    let mut connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_mut())?;
    let queue_item = models::Queued::get_id(report.queue.id, connection.as_mut())?;
    let mut pkgbase = models::PkgBase::get_id(queue_item.pkgbase_id, connection.as_mut())?;

    let mut needs_retry = false;
    for (artifact, rebuild) in &report.rebuilds {
        let mut packages = models::Package::get_by(
            &artifact.name,
            &pkgbase.distro,
            &pkgbase.suite,
            None,
            connection.as_mut(),
        )?;

        packages.retain(|x| x.pkgbase_id == pkgbase.id);
        if packages.len() != 1 {
            error!("rebuilt artifact didn't match a unique package in database. matches={:?} instead of 1", packages.len());
            continue;
        }
        let mut pkg = packages.remove(0);

        // adding build to package
        let encoded_log = zstd_compress(report.build_log.as_bytes())
            .await
            .map_err(Error::from)?;

        let encoded_diffoscope = match &rebuild.diffoscope {
            Some(diffoscope) => Some(
                zstd_compress(diffoscope.as_bytes())
                    .await
                    .map_err(Error::from)?,
            ),
            _ => None,
        };

        let encoded_attestation = match &rebuild.attestation {
            Some(attestation) => {
                let attestation = Attestation::parse(attestation.as_bytes())?;

                // add additional signature
                // attestation.sign(privkey)?;

                // compress attestation
                let compressed = attestation.to_compressed_bytes().await?;

                Some(compressed)
            }
            _ => None,
        };

        let build =
            models::NewBuild::from_api(encoded_diffoscope, encoded_log, encoded_attestation);

        let build_id = build.insert(connection.as_mut())?;
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

        pkg.update(connection.as_mut())?;
    }

    // update pkgbase
    if needs_retry {
        pkgbase.retries += 1;
        pkgbase.schedule_retry(cfg.schedule.retry_delay_base());
    } else {
        pkgbase.clear_retry(connection.as_mut())?;
    }
    pkgbase.update(connection.as_mut())?;

    // cleanup queue item and worker status
    queue_item.delete(connection.as_mut())?;
    worker.status = None;
    worker.update(connection.as_mut())?;

    Ok(HttpResponse::Ok().json(()))
}

#[get("/api/v0/builds/{id}/log")]
pub async fn get_build_log(
    req: HttpRequest,
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_mut()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    forward_compressed_data(req, "text/plain; charset=utf-8", build.build_log).await
}

#[get("/api/v0/builds/{id}/attestation")]
pub async fn get_attestation(
    req: HttpRequest,
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_mut()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    if let Some(attestation) = build.attestation {
        forward_compressed_data(req, "application/json; charset=utf-8", attestation).await
    } else {
        Ok(not_found())
    }
}

#[get("/api/v0/builds/{id}/diffoscope")]
pub async fn get_diffoscope(
    req: HttpRequest,
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let build = match models::Build::get_id(*id, connection.as_mut()) {
        Ok(build) => build,
        Err(_) => return Ok(not_found()),
    };

    if let Some(diffoscope) = build.diffoscope {
        forward_compressed_data(req, "text/plain; charset=utf-8", diffoscope).await
    } else {
        Ok(not_found())
    }
}

#[get("/api/v0/dashboard")]
pub async fn get_dashboard(
    pool: web::Data<Pool>,
    lock: web::Data<Arc<RwLock<DashboardState>>>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    let stale = {
        let state = lock.read().unwrap();
        !state.is_fresh()
    };
    if stale {
        let mut state = lock.write().unwrap();
        debug!("Updating cached dashboard");
        state.update(connection.as_mut())?;
    }
    let state = lock.read().unwrap();
    let resp = state.get_response()?;
    Ok(HttpResponse::Ok().json(resp))
}
