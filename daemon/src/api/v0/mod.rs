use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    OptionalExtension, SelectableHelper, SqliteConnection,
};
mod auth;
mod dashboard;

use crate::api::forward_compressed_data;
use crate::api::v0::aliases::{r1, r2};
use crate::attestation::{self};
use crate::config::Config;
use crate::db::Pool;
use crate::models;
use crate::models::{BinaryPackage, BuildInput, Queued, SourcePackage};
use crate::schema::*;
use crate::web;
use actix_web::{get, http, post, HttpRequest, HttpResponse, Responder};
use chrono::prelude::*;
use chrono::Duration;
pub(crate) use dashboard::DashboardState;
use diesel::dsl::auto_type;
use diesel::{QueryDsl, RunQueryDsl};
use in_toto::crypto::PrivateKey;
use rebuilderd_common::api::v0::*;
use rebuilderd_common::config::PING_DEADLINE;
use rebuilderd_common::errors::*;
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

#[get("/workers")]
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

#[post("/pkgs/sync")]
pub async fn sync_work(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _import: web::Json<SuiteImport>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
}

#[auto_type(no_type_alias)]
fn filter_binary_packages_by<'a>(
    name: Option<&'a str>,
    distribution: Option<&'a str>,
    release: Option<&'a str>,
    component: Option<&'a str>,
    architecture: Option<&'a str>,
    status: Option<&'a str>,
) -> _ {
    let mut query = binary_packages::table
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
        .into_boxed::<'a, diesel::sqlite::Sqlite>();

    if let Some(name) = name {
        query = query.filter(source_packages::name.eq(name));
    }

    if let Some(distribution) = distribution {
        query = query.filter(source_packages::distribution.eq(distribution));
    }

    if let Some(release) = release {
        query = query.filter(source_packages::release.eq(release));
    }

    if let Some(component) = component {
        query = query.filter(source_packages::component.eq(component));
    }

    if let Some(architecture) = architecture {
        query = query.filter(build_inputs::architecture.eq(architecture));
    }

    if let Some(status) = status {
        if status == "UNKWN" {
            query = query.filter(
                r1.field(rebuilds::status)
                    .eq(status.to_string())
                    .or(r1.field(rebuilds::status).is_null()),
            );
        } else {
            query = query.filter(r1.field(rebuilds::status).eq(status.to_string()));
        }
    }

    query
}

#[get("/pkgs/list")]
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

    let data = filter_binary_packages_by(
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
        rebuild_artifacts::status.nullable(),
        source_packages::component,
        binary_packages::artifact_url,
        r1.field(rebuilds::id).nullable(),
        r1.field(rebuilds::built_at).nullable(),
        rebuild_artifacts::diffoscope_log_id
            .is_not_null()
            .nullable(),
        rebuild_artifacts::attestation_log_id
            .is_not_null()
            .nullable(),
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

#[post("/queue/list")]
pub async fn list_queue(
    query: web::Json<ListQueue>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = queue::table
        .order_by((queue::priority, queue::queued_at, queue::id))
        .into_boxed();

    if let Some(limit) = &query.limit {
        sql = sql.limit(*limit);
    }

    let queue = sql
        .load::<Queued>(connection.as_mut())
        .map_err(Error::from)?
        .into_iter()
        .map(|x| into_queue_item(x, connection.as_mut()))
        .collect::<Result<Vec<QueueItem>>>()?;

    let now = Utc::now().naive_utc();
    Ok(HttpResponse::Ok().json(QueueList { now, queue }))
}

pub fn into_queue_item(queued: Queued, connection: &mut SqliteConnection) -> Result<QueueItem> {
    let build_input = build_inputs::table
        .filter(build_inputs::id.eq(queued.build_input_id))
        .get_result::<BuildInput>(connection)?;

    let source_package = source_packages::table
        .filter(source_packages::id.eq(build_input.source_package_id))
        .select(SourcePackage::as_select())
        .get_result(connection)?;

    let binary_packages = binary_packages::table
        .filter(binary_packages::source_package_id.eq(source_package.id))
        .load::<BinaryPackage>(connection)?;

    let version = source_package.version.clone();
    let artifacts = binary_packages
        .iter()
        .map(|b| PkgArtifact {
            name: b.name.clone(),
            version: b.version.clone(),
            url: b.artifact_url.clone(),
        })
        .collect();

    let pkgbase = into_pkg_group(
        source_package,
        build_input.architecture,
        Some(build_input.url),
        artifacts,
    )?;

    Ok(QueueItem {
        id: queued.id,
        pkgbase,
        version,
        queued_at: queued.queued_at,
        worker_id: queued.worker,
        started_at: queued.started_at,
        last_ping: queued.last_ping,
    })
}

fn into_pkg_group(
    source_package: SourcePackage,
    architecture: String,
    input_url: Option<String>,
    artifacts: Vec<PkgArtifact>,
) -> Result<PkgGroup> {
    Ok(PkgGroup {
        name: source_package.name,
        version: source_package.version,

        distro: source_package.distribution,
        suite: source_package.component.unwrap_or_default(),
        architecture,

        input_url,
        artifacts,
    })
}

#[post("/queue/push")]
pub async fn push_queue(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _query: web::Json<PushQueue>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[post("/queue/pop")]
pub async fn pop_queue(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _query: web::Json<WorkQuery>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[post("/queue/drop")]
pub async fn drop_from_queue(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _query: web::Json<DropQueueItem>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[post("/pkg/requeue")]
pub async fn requeue_pkgbase(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _query: web::Json<RequeueQuery>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[post("/build/ping")]
pub async fn ping_build(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _item: web::Json<PingRequest>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[post("/build/report")]
pub async fn report_build(
    _req: HttpRequest,
    _cfg: web::Data<Config>,
    _privkey: web::Data<Arc<PrivateKey>>,
    _report: web::Json<BuildReport>,
    _pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    // v0 is only available as a read-only API for backwards compatibility
    Ok(HttpResponse::NotImplemented())
}

#[get("/builds/{id}/log")]
pub async fn get_build_log(
    req: HttpRequest,
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    // get log of the latest rebuild - v0 has no concept of multiple successful builds
    let build_log = rebuild_artifacts::table
        .filter(rebuild_artifacts::id.eq(id.into_inner()))
        .inner_join(rebuilds::table.inner_join(build_logs::table))
        .select(build_logs::build_log)
        .order_by(rebuilds::built_at.desc())
        .first::<Vec<u8>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(build_log) = build_log {
        forward_compressed_data(req, "text/plain; charset=utf-8", build_log).await
    } else {
        Ok(not_found())
    }
}

#[get("/builds/{id}/attestation")]
pub async fn get_attestation(
    req: HttpRequest,
    id: web::Path<i32>,
    _cfg: web::Data<Config>,
    _privkey: web::Data<Arc<PrivateKey>>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    // get log of the first artifact - v0 has no concept of separate attestation logs
    let attestation = rebuild_artifacts::table
        .filter(rebuild_artifacts::rebuild_id.eq(id.into_inner()))
        .inner_join(attestation_logs::table)
        .select(attestation_logs::attestation_log)
        .order_by(rebuild_artifacts::id.asc())
        .first::<Vec<u8>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(attestation) = attestation {
        // v0 used to transparently sign attestations here, but for now v0 is entirely read-only
        forward_compressed_data(req, "application/json; charset=utf-8", attestation).await
    } else {
        Ok(not_found())
    }
}

#[get("/builds/{id}/diffoscope")]
pub async fn get_diffoscope(
    req: HttpRequest,
    id: web::Path<i32>,
    pool: web::Data<Pool>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let diffoscope = rebuild_artifacts::table
        .filter(rebuild_artifacts::rebuild_id.eq(id.into_inner()))
        .inner_join(diffoscope_logs::table)
        .select(diffoscope_logs::diffoscope_log)
        .order_by(rebuild_artifacts::id.asc())
        .first::<Vec<u8>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(diffoscope) = diffoscope {
        forward_compressed_data(req, "text/plain; charset=utf-8", diffoscope).await
    } else {
        Ok(not_found())
    }
}

#[get("/dashboard")]
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

#[get("/public-keys")]
pub async fn get_public_key(privkey: web::Data<Arc<PrivateKey>>) -> web::Result<impl Responder> {
    let pubkey = attestation::pubkey_to_pem(privkey.public())?;
    Ok(HttpResponse::Ok().json(PublicKeys {
        current: vec![pubkey],
    }))
}
