use crate::attestation::{self, Attestation};
use crate::auth;
use crate::config::Config;
use crate::dashboard::DashboardState;
use crate::db::Pool;
use crate::diesel::ExpressionMethods;
use crate::diesel::NullableExpressionMethods;
use crate::diesel::QueryDsl;
use crate::models;
use crate::models::{r1, BinaryPackage, BuildInput, Queued, SourcePackage};
use crate::schema::*;
use crate::sync;
use crate::util::{is_zstd_compressed, zstd_compress, zstd_decompress};
use crate::web;
use actix_web::http::header::{AcceptEncoding, ContentEncoding, Encoding, Header};
use actix_web::{get, http, post, HttpRequest, HttpResponse, Responder};
use chrono::prelude::*;
use chrono::Duration;
use diesel::{OptionalExtension, RunQueryDsl, SelectableHelper, SqliteConnection};
use in_toto::crypto::PrivateKey;
use rebuilderd_common::api::v0::*;
use rebuilderd_common::config::PING_DEADLINE;
use rebuilderd_common::errors::*;
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

    // mark stale workers as offline before returning any results
    let now = Utc::now().naive_utc();
    let deadline = now - Duration::seconds(PING_DEADLINE);

    diesel::update(workers::table.filter(workers::last_ping.lt(deadline)))
        .set((
            workers::online.eq(false),
            workers::status.eq(None as Option<String>),
        ))
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    // grab online workers
    let workers = workers::table
        .filter(workers::online.eq(true))
        .load::<models::Worker>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(workers))
}

// this route is configured in src/main.rs so we can reconfigure the json extractor
// #[post("/api/v0/pkgs/sync")]
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

    // TODO: querify
    sync::run(import, connection.as_mut())?;

    Ok(HttpResponse::Ok().json(JobAssignment::Nothing))
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
    if let Some(latest_built_at) = rebuilds::table
        .select(diesel::dsl::max(rebuilds::built_at))
        .first(connection.as_mut())
        .map_err(Error::from)?
    {
        let latest_built_at = DateTime::from_naive_utc_and_offset(latest_built_at, Utc);
        if let Some(duration) = modified_since_duration(&req, latest_built_at) {
            if duration.num_seconds() >= 0 {
                return Ok(not_modified());
            }
        }

        let latest_built_at = SystemTime::from(latest_built_at);
        builder.insert_header(http::header::LastModified(latest_built_at.into()));
    }

    let data = BinaryPackage::filter_by(
        query.name.as_deref(),
        query.distro.as_deref(),
        None,
        query.suite.as_deref(),
        query.architecture.as_deref(),
        query.status.map(|s| s.to_string()).as_deref(),
    )
    .select((
        binary_packages::name,
        source_packages::distribution,
        binary_packages::architecture,
        binary_packages::version,
        r1.field(rebuilds::status).nullable(),
        source_packages::component,
        binary_packages::artifact_url,
        r1.field(rebuilds::id).nullable(),
        r1.field(rebuilds::built_at).nullable(),
        rebuild_artifacts::diffoscope.is_not_null().nullable(),
        rebuild_artifacts::attestation.is_not_null().nullable(),
    ))
    .get_results::<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<i32>,
        Option<NaiveDateTime>,
        Option<bool>,
        Option<bool>,
    )>(connection.as_mut())
    .map_err(Error::from)?;

    let mapped = data
        .into_iter()
        .map(|d| {
            let release = PkgRelease {
                name: d.0,
                distro: d.1,
                architecture: d.2,
                version: d.3,
                status: d.4.unwrap_or("UNKWN".to_string()).parse()?,
                suite: d.5.unwrap_or_default(), // TODO: behaviour change, was always present, may not be now
                artifact_url: d.6,
                build_id: d.7,
                built_at: d.8,
                has_diffoscope: d.9.unwrap_or_default(),
                has_attestation: d.10.unwrap_or_default(),
            };

            Ok(release)
        })
        .collect::<Result<Vec<PkgRelease>>>()?;

    Ok(builder.json(mapped))
}

#[post("/api/v0/queue/list")]
pub async fn list_queue(
    query: web::Json<ListQueue>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    models::Queued::free_stale_jobs(connection.as_mut())?;

    let mut sql = queue::table
        .order_by((queue::priority, queue::queued_at, queue::id))
        .into_boxed();

    if let Some(limit) = &query.limit {
        sql = sql.limit(*limit);
    }

    let queue = sql
        .load::<models::Queued>(connection.as_mut())
        .map_err(Error::from)?
        .into_iter()
        .map(|x| x.into_api_item(connection.as_mut()))
        .collect::<Result<Vec<QueueItem>>>()?;

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
    let build_input_ids = models::BinaryPackage::filter_by(
        Some(&query.name),
        Some(&query.distro),
        None,
        Some(&query.suite),
        query.architecture.as_deref(),
        None,
    )
    .select(binary_packages::build_input_id)
    .distinct()
    .load::<i32>(connection.as_mut())
    .map_err(Error::from)?;

    for build_input_id in build_input_ids {
        let item = models::NewQueued::new(build_input_id, query.priority);

        debug!("adding to queue: {:?}", item);
        if let Err(err) = item.upsert(connection.as_mut()) {
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

    Queued::free_stale_jobs(connection.as_mut())?;

    let mut sql = queue::table
        .inner_join(build_inputs::table)
        .filter(queue::worker.is_null())
        .into_boxed();

    if !query.supported_backends.is_empty() {
        sql = sql.filter(build_inputs::backend.eq_any(&query.supported_backends));
    }

    let item = sql
        .order_by((queue::priority, queue::queued_at, queue::id))
        .select(Queued::as_select())
        .first::<Queued>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    let (resp, status) = if let Some(mut item) = item {
        let now: DateTime<Utc> = Utc::now();

        item.worker = Some(worker.id);
        item.started_at = Some(now.naive_utc());
        item.last_ping = Some(now.naive_utc());
        item.update(connection.as_mut())?;

        let api_item = item.into_api_item(connection.as_mut())?;
        let status = format!(
            "working hard on {} {}",
            api_item.pkgbase.name, api_item.pkgbase.version
        );

        (JobAssignment::Rebuild(Box::new(api_item)), Some(status))
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

    let source_packages = models::SourcePackage::filter_by(
        Some(&query.name),
        Some(&query.distro),
        None,
        Some(&query.suite),
        query.architecture.as_deref(),
    )
    .select(source_packages::id)
    .distinct()
    .load::<i32>(connection.as_mut())
    .map_err(Error::from)?;

    Queued::drop_for_source_packages(&source_packages, connection.as_mut())?;

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

    let build_input_ids = BinaryPackage::filter_by(
        query.name.as_deref(),
        query.distro.as_deref(),
        None,
        query.suite.as_deref(),
        query.architecture.as_deref(),
        query.status.map(|q| q.to_string()).as_deref(),
    )
    .select(build_inputs::id)
    .distinct()
    .load::<i32>(connection.as_mut())
    .map_err(Error::from)?;

    let rebuild_ids = rebuilds::table
        .filter(rebuilds::build_input_id.eq_any(&build_input_ids))
        .select(rebuilds::id)
        .load::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    let to_be_queued = build_input_ids
        .into_iter()
        .map(|id| models::NewQueued::new(id, query.priority))
        .collect::<Vec<_>>();

    diesel::insert_into(queue::table)
        .values(to_be_queued)
        //.on_conflict_do_nothing()
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    if query.reset {
        diesel::update(rebuilds::table)
            .set(rebuilds::status.eq("UNKWN"))
            .filter(rebuilds::id.eq_any(&rebuild_ids))
            .execute(connection.as_mut())
            .map_err(Error::from)?;

        diesel::update(rebuild_artifacts::table)
            .set(rebuild_artifacts::status.eq("UNKWN"))
            .filter(rebuild_artifacts::rebuild_id.eq_any(&rebuild_ids))
            .execute(connection.as_mut())
            .map_err(Error::from)?;
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

    if item.worker != Some(worker.id) {
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
    privkey: web::Data<Arc<PrivateKey>>,
    report: web::Json<BuildReport>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    if auth::worker(&cfg, &req).is_err() {
        return Ok(forbidden());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let mut worker = get_worker_from_request(&req, &cfg, connection.as_mut())?;

    let joined = source_packages::table
        .inner_join(build_inputs::table.inner_join(queue::table))
        .filter(queue::id.eq(report.queue.id))
        .select((
            SourcePackage::as_select(),
            BuildInput::as_select(),
            Queued::as_select(),
        ))
        .first::<(SourcePackage, BuildInput, Queued)>(connection.as_mut())
        .map_err(Error::from)?;

    let encoded_log = zstd_compress(report.build_log.as_bytes())
        .await
        .map_err(Error::from)?;

    let overall_status = if report
        .rebuilds
        .iter()
        .all(|s| s.1.status == BuildStatus::Good)
    {
        Status::Good
    } else {
        Status::Bad
    };

    let rebuild = models::NewRebuild {
        build_input_id: joined.1.id,
        started_at: joined.2.started_at,
        built_at: Some(Utc::now().naive_utc()),
        build_log: encoded_log,
        status: Some(overall_status.to_string()),
    };

    let rebuild_id = rebuild.insert(connection.as_mut())?;

    let mut needs_retry = false;
    for (artifact, rebuild) in &report.rebuilds {
        let package_count = binary_packages::table
            .filter(binary_packages::name.eq(&artifact.name))
            .filter(binary_packages::version.eq(&artifact.version))
            .filter(binary_packages::source_package_id.eq(joined.0.id))
            .filter(binary_packages::build_input_id.eq(joined.1.id))
            .select(diesel::dsl::count(binary_packages::id))
            .first::<i64>(connection.as_mut())
            .map_err(Error::from)?;

        if package_count != 1 {
            error!("rebuilt artifact didn't match a unique package in database. matches={:?} instead of 1", package_count);
            continue;
        }

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
                let mut attestation = Attestation::parse(attestation.as_bytes())?;

                // add additional signature
                attestation.sign(&privkey)?;

                // compress attestation
                let compressed = attestation.to_compressed_bytes().await?;

                Some(compressed)
            }
            _ => None,
        };

        let status = match rebuild.status {
            BuildStatus::Good => Status::Good.to_string(),
            _ => Status::Bad.to_string(),
        };

        let rebuild_artifact = models::NewRebuildArtifact {
            rebuild_id,
            name: artifact.name.clone(),
            diffoscope: encoded_diffoscope,
            attestation: encoded_attestation,
            status: Some(status),
        };

        rebuild_artifact.insert(connection.as_mut())?;

        if rebuild.status != BuildStatus::Good {
            needs_retry = true;
        }
    }

    // update build_inputs
    let mut new_build_input = joined.1;
    if needs_retry {
        new_build_input.retries += 1;
        new_build_input.schedule_retry(cfg.schedule.retry_delay_base(), connection.as_mut())?;
    } else {
        new_build_input.clear_retry(connection.as_mut())?;
    }

    // cleanup queue item and worker status
    let queued = joined.2;
    queued.delete(connection.as_mut())?;

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

    let build_log = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .inner_join(rebuilds::table)
        .select(rebuilds::build_log)
        .first::<Vec<u8>>(connection.as_mut())
        .map_err(Error::from)?;

    forward_compressed_data(req, "text/plain; charset=utf-8", build_log).await
}

#[get("/api/v0/builds/{id}/attestation")]
pub async fn get_attestation(
    req: HttpRequest,
    id: web::Path<i32>,
    cfg: web::Data<Config>,
    privkey: web::Data<Arc<PrivateKey>>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let attestation = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .select(rebuild_artifacts::attestation)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .map_err(Error::from)?;

    if let Some(attestation) = attestation {
        if cfg.transparently_sign_attestations {
            let (bytes, has_new_signature) =
                attestation::compressed_attestation_sign_if_necessary(attestation, &privkey)
                    .await?;

            if has_new_signature {
                build.attestation = Some(bytes.clone());
                build.update(connection.as_mut())?;
            }

            attestation = bytes;
        }

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

    let diffoscope = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .select(rebuild_artifacts::diffoscope)
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .map_err(Error::from)?;

    if let Some(diffoscope) = diffoscope {
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

#[get("/api/v0/public-keys")]
pub async fn get_public_key(privkey: web::Data<Arc<PrivateKey>>) -> web::Result<impl Responder> {
    let pubkey = attestation::pubkey_to_pem(privkey.public())?;
    Ok(HttpResponse::Ok().json(PublicKeys {
        current: vec![pubkey],
    }))
}
