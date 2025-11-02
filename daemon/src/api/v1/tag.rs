use crate::api::v1::util::auth;
use crate::config::Config;
use crate::db::Pool;
use crate::models::{NewSourcePackageTagRule, NewTag};
use crate::schema::{tag_rules, tags};
use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post};
use diesel::{ExpressionMethods, delete};
use diesel::{QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v1::{CreateTagRequest, CreateTagRuleRequest, TagRule};
use rebuilderd_common::errors::Error;

#[get("")]
pub async fn get_tags(pool: web::Data<Pool>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let tags = tags::table
        .select(tags::tag)
        .get_results::<String>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(tags))
}

#[post("")]
pub async fn create_tag(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<CreateTagRequest>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let tag = NewTag {
        tag: request.tag.clone(),
    }
    .ensure_exists(connection.as_mut())?;

    Ok(HttpResponse::Ok().json(tag.tag))
}

#[delete("/{tag}")]
pub async fn delete_tag(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    tag: web::Path<String>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    delete(tags::table.filter(tags::tag.eq(tag.into_inner())))
        .execute(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}

#[get("/{tag}")]
pub async fn get_tag_rules(
    pool: web::Data<Pool>,
    tag: web::Path<String>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let tag_rules = tags::table
        .inner_join(tag_rules::table)
        .filter(tags::tag.eq(tag.into_inner()))
        .select((
            tag_rules::id,
            tag_rules::name_pattern,
            tag_rules::version_pattern,
        ))
        .get_results::<TagRule>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(tag_rules))
}

#[post("/{tag}")]
pub async fn create_tag_rule(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    tag: web::Path<String>,
    request: web::Json<CreateTagRuleRequest>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;

    let tag_id = tags::table
        .filter(tags::tag.eq(tag.into_inner()))
        .select(tags::id)
        .get_result::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    let tag_rule = NewSourcePackageTagRule {
        tag_id,
        name_pattern: request.name_pattern.clone(),
        version_pattern: request.version_pattern.clone(),
    }
    .ensure_exists(connection.as_mut())?;

    Ok(HttpResponse::Ok().json(tag_rule))
}

#[delete("/{tag}/{id}")]
pub async fn delete_tag_rule(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    parameters: web::Path<(String, i32)>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let mut connection = pool.get().map_err(Error::from)?;
    let (tag, tag_rule_id) = parameters.into_inner();

    let tag_id = tags::table
        .filter(tags::tag.eq(tag))
        .select(tags::id)
        .get_result::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    delete(
        tag_rules::table
            .filter(tag_rules::id.eq(tag_rule_id))
            .filter(tag_rules::tag_id.eq(tag_id)),
    )
    .execute(connection.as_mut())
    .map_err(Error::from)?;

    Ok(HttpResponse::NoContent().finish())
}
