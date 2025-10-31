use crate::api::header;
use crate::api::v1::util::auth;
use crate::api::v1::util::pagination::PaginateDsl;
use crate::config::Config;
use crate::db::Pool;
use crate::models::{NewTag, NewWorker, NewWorkerTag, Tag, WorkerTag};
use crate::schema::{tags, worker_tags, workers};
use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put};
use chrono::Utc;
use diesel::dsl::{delete, exists, select};
use diesel::ExpressionMethods;
use diesel::{Connection, OptionalExtension, QueryDsl, RunQueryDsl, SqliteExpressionMethods};
use rebuilderd_common::api::WORKER_KEY_HEADER;
use rebuilderd_common::api::v1::{Page, RegisterWorkerRequest, ResultPage};
use rebuilderd_common::errors::{Context, Error, format_err};
use std::net::IpAddr;

#[diesel::dsl::auto_type]
fn workers_base() -> _ {
    workers::table.select((
        workers::id,
        workers::name,
        workers::address,
        workers::status,
        workers::last_ping,
        workers::online,
    ))
}

#[get("")]
pub async fn get_workers(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let records = workers_base()
        .paginate(page.into_inner())
        .load::<rebuilderd_common::api::v1::Worker>(connection.as_mut())
        .map_err(Error::from)?;

    let total = workers_base()
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(ResultPage { total, records }))
}

#[post("")]
pub async fn register_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<RegisterWorkerRequest>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    if auth::signup(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let key = header(&req, WORKER_KEY_HEADER).context("Failed to get worker key")?;
    let ip = if let Some(real_ip_header) = &cfg.real_ip_header {
        let ip = header(&req, real_ip_header).context("Failed to locate real ip header")?;
        ip.parse::<IpAddr>()
            .context("Can't parse real ip header as ip address")?
    } else {
        let ci = req
            .peer_addr()
            .ok_or_else(|| format_err!("Can't determine client ip"))?;
        ci.ip()
    };

    let new_worker = NewWorker {
        key: key.to_string(),
        name: request.name.clone(),
        address: ip.to_string(),
        status: None,
        last_ping: Utc::now().naive_utc(),
        online: true,
    };

    new_worker.upsert(connection.as_mut())?;

    Ok(HttpResponse::NoContent().finish())
}

#[get("/{id}")]
pub async fn get_worker(pool: web::Data<Pool>, id: web::Path<i32>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = workers_base()
        .filter(workers::id.is(id.into_inner()))
        .get_result::<rebuilderd_common::api::v1::Worker>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[delete("/{id}")]
pub async fn unregister_worker(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    if auth::worker(&cfg, &req, connection.as_mut()).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    connection
        .transaction(|conn| {
            diesel::delete(workers::table)
                .filter(workers::id.is(id.into_inner()))
                .execute(conn)
        })
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}

#[get("/{id}/tags")]
pub async fn get_worker_tags(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let worker_exists = select(exists(workers::table.filter(workers::id.eq(*id))))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

    if !worker_exists {
        return Ok(HttpResponse::NotFound().finish());
    }

    let tags = worker_tags::table
        .inner_join(tags::table)
        .filter(worker_tags::worker_id.eq(id.into_inner()))
        .select(tags::tag)
        .get_results::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(tags))
}

#[put("/{id}/tags")]
pub async fn set_worker_tags(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
    tags: web::Json<Vec<String>>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let worker_exists = select(exists(workers::table.filter(workers::id.eq(*id))))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

    if !worker_exists {
        return Ok(HttpResponse::NotFound().finish());
    }

    let tags = tags
        .into_inner()
        .into_iter()
        .map(|v| NewTag { tag: v }.ensure_exists(connection.as_mut()))
        .collect::<Result<Vec<Tag>, _>>()?;

    connection.transaction(|conn| {
        // drop all existing tag associations
        delete(worker_tags::table.filter(worker_tags::worker_id.eq(*id))).execute(conn)?;

        // create new tag associations for the input set
        tags.into_iter()
            .map(|t| {
                NewWorkerTag {
                    worker_id: *id,
                    tag_id: t.id,
                }
                .ensure_exists(conn.as_mut())
            })
            .collect::<Result<Vec<WorkerTag>, _>>()?;

        Ok::<(), Error>(())
    })?;

    Ok(HttpResponse::NotImplemented().finish())
}

#[put("/{id}/tags/{tag}")]
pub async fn create_worker_tag(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    parameters: web::Path<(i32, String)>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let (worker_id, tag_name) = parameters.into_inner();
    let mut connection = pool.get().map_err(Error::from)?;

    let worker_exists = select(exists(workers::table.filter(workers::id.eq(worker_id))))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

    if !worker_exists {
        return Ok(HttpResponse::NotFound().finish());
    }

    let tag = NewTag { tag: tag_name }.ensure_exists(connection.as_mut())?;
    NewWorkerTag {
        worker_id,
        tag_id: tag.id,
    }
    .ensure_exists(connection.as_mut())?;

    Ok(HttpResponse::NoContent().finish())
}

#[delete("/{id}/tags/{tag}")]
pub async fn delete_worker_tag(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    parameters: web::Path<(i32, String)>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let (worker_id, tag_name) = parameters.into_inner();
    let mut connection = pool.get().map_err(Error::from)?;

    let worker_exists = select(exists(workers::table.filter(workers::id.eq(worker_id))))
        .get_result::<bool>(connection.as_mut())
        .map_err(Error::from)?;

    if !worker_exists {
        return Ok(HttpResponse::NotFound().finish());
    }

    let tag_id = tags::table
        .filter(tags::tag.eq(tag_name))
        .select(tags::id)
        .get_result::<i32>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if tag_id.is_none() {
        return Ok(HttpResponse::NotFound().finish());
    }

    connection.transaction(|conn| {
        // remove the association between this worker and the tag
        delete(
            worker_tags::table
                .filter(worker_tags::worker_id.eq(worker_id))
                .filter(worker_tags::tag_id.eq(tag_id.unwrap())),
        )
        .execute(conn)?;

        // clean up unused tags
        delete(
            tags::table
                .filter(tags::id.ne_all(worker_tags::table.select(worker_tags::tag_id).distinct())),
        )
        .execute(conn)?;

        Ok::<(), Error>(())
    })?;

    Ok(HttpResponse::NoContent().finish())
}
