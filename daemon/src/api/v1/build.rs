use crate::api::v1::util::auth;
use crate::api::v1::util::filters::DieselIdentityFilter;
use crate::api::v1::util::filters::DieselOriginFilter;
use crate::api::v1::util::pagination::PaginateDsl;
use crate::api::{forward_compressed_data, DEFAULT_QUEUE_PRIORITY};
use crate::config::Config;
use crate::db::Pool;
use crate::models::{
    NewAttestationLog, NewBuildLog, NewDiffoscopeLog, NewQueued, NewRebuild, NewRebuildArtifact,
    Queued,
};
use crate::schema::{
    attestation_logs, build_inputs, build_logs, diffoscope_logs, queue, rebuild_artifacts,
    rebuilds, source_packages,
};
use crate::{attestation, web};
use actix_web::{get, post, HttpRequest, HttpResponse, Responder};
use chrono::{Duration, Utc};
use diesel::dsl::update;
use diesel::ExpressionMethods;
use diesel::NullableExpressionMethods;
use diesel::QueryDsl;
use diesel::{OptionalExtension, RunQueryDsl};
use in_toto::crypto::PrivateKey;
use rebuilderd_common::api;
use rebuilderd_common::api::v1::{
    BuildStatus, IdentityFilter, OriginFilter, Page, Rebuild, RebuildReport, ResultPage,
};
use rebuilderd_common::errors::Error;
use rebuilderd_common::utils::{is_zstd_compressed, zstd_compress};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

#[diesel::dsl::auto_type]
fn builds_base() -> _ {
    rebuilds::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .select((
            rebuilds::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            build_inputs::architecture,
            build_inputs::backend,
            build_inputs::retries,
            rebuilds::started_at,
            rebuilds::built_at,
            rebuilds::status,
        ))
}

#[get("")]
pub async fn get_builds(
    pool: web::Data<Pool>,
    page: web::Query<Page>,
    origin_filter: web::Query<OriginFilter>,
    identity_filter: web::Query<IdentityFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = builds_base().into_boxed();
    sql = origin_filter.filter(sql);

    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

    sql = identity_filter.filter(sql, source_packages::name, source_packages::version);

    let records = sql
        .paginate(page.into_inner())
        .load::<Rebuild>(connection.as_mut())
        .map_err(Error::from)?;

    let mut total_sql = builds_base().into_boxed();
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
pub async fn submit_rebuild_report(
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    request: web::Json<RebuildReport>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    if auth::worker(&cfg, &req, connection.as_mut()).is_err() {
        return Ok(HttpResponse::Forbidden());
    }

    let report = request.into_inner();
    let queued = queue::table
        .filter(queue::id.eq(report.queue_id))
        .get_result::<Queued>(connection.as_mut())
        .map_err(Error::from)?;

    // figure out any other build inputs that should share this result (same input, backend, and arch). Will include the
    // enqueued build ID as well, so no need to add it later.
    let b1 = diesel::alias!(build_inputs as b1);
    let build_inputs = build_inputs::table
        .select(build_inputs::id)
        .filter(
            build_inputs::url.eq(b1
                .filter(b1.field(build_inputs::id).eq(queued.build_input_id))
                .select(b1.field(build_inputs::url))
                .single_value()
                .assume_not_null()),
        )
        .filter(
            build_inputs::backend.eq(b1
                .filter(b1.field(build_inputs::id).eq(queued.build_input_id))
                .select(b1.field(build_inputs::backend))
                .single_value()
                .assume_not_null()),
        )
        .filter(
            build_inputs::architecture.eq(b1
                .filter(b1.field(build_inputs::id).eq(queued.build_input_id))
                .select(b1.field(build_inputs::architecture))
                .single_value()
                .assume_not_null()),
        )
        .load::<i32>(connection.as_mut())
        .map_err(Error::from)?;

    let encoded_log = if is_zstd_compressed(&report.build_log) {
        report.build_log
    } else {
        zstd_compress(&report.build_log[..])
            .await
            .map_err(Error::from)?
    };

    let new_log = NewBuildLog {
        build_log: encoded_log,
    };

    let new_log_id = new_log.insert(connection.as_mut())?;

    let mut artifact_logs: HashMap<&String, (Option<i32>, Option<i32>)> = HashMap::new();

    for build_input_id in build_inputs {
        let new_rebuild = NewRebuild {
            build_input_id,
            started_at: queued.started_at,
            built_at: Some(report.built_at),
            build_log_id: new_log_id,
            status: Some(report.status.to_string()),
        };

        let new_rebuild_id = new_rebuild.insert(connection.as_mut())?;

        for artifact_report in &report.artifacts {
            let entry = artifact_logs.entry(&artifact_report.name);

            let logs = match entry {
                Entry::Occupied(oc) => oc.into_mut(),
                Entry::Vacant(vc) => {
                    let encoded_diffoscope = if let Some(diffoscope) = &artifact_report.diffoscope {
                        Some(if is_zstd_compressed(diffoscope) {
                            diffoscope.clone()
                        } else {
                            zstd_compress(&diffoscope[..]).await.map_err(Error::from)?
                        })
                    } else {
                        None::<Vec<u8>>
                    };

                    let encoded_attestation =
                        if let Some(attestation) = &artifact_report.attestation {
                            Some(if is_zstd_compressed(attestation) {
                                attestation.clone()
                            } else {
                                zstd_compress(&attestation[..]).await.map_err(Error::from)?
                            })
                        } else {
                            None::<Vec<u8>>
                        };

                    let new_diffoscope_id = if let Some(encoded_diffoscope) = encoded_diffoscope {
                        let new_diffoscope_log = NewDiffoscopeLog {
                            diffoscope_log: encoded_diffoscope.clone(),
                        };

                        Some(new_diffoscope_log.insert(connection.as_mut())?)
                    } else {
                        None::<i32>
                    };

                    let new_attestation_id = if let Some(encoded_attestation) = encoded_attestation
                    {
                        let new_attestation_log = NewAttestationLog {
                            attestation_log: encoded_attestation.clone(),
                        };

                        Some(new_attestation_log.insert(connection.as_mut())?)
                    } else {
                        None::<i32>
                    };

                    vc.insert((new_diffoscope_id, new_attestation_id))
                }
            };

            let new_rebuild_artifact = NewRebuildArtifact {
                rebuild_id: new_rebuild_id,
                name: artifact_report.name.clone(),
                diffoscope_log_id: logs.0,
                attestation_log_id: logs.1,
                status: Some(artifact_report.status.to_string()),
            };

            new_rebuild_artifact.insert(connection.as_mut())?;
        }
    }

    queued.delete(connection.as_mut())?;

    if report.status != BuildStatus::Good {
        // increment retries and requeue
        let retry_count = build_inputs::table
            .filter(build_inputs::id.eq(queued.build_input_id))
            .select(build_inputs::retries)
            .get_result::<i32>(connection.as_mut())
            .map_err(Error::from)?;

        let now = Utc::now();
        let then = now + Duration::hours(((retry_count + 1) * 24) as i64);

        update(build_inputs::table)
            .filter(build_inputs::id.eq(queued.build_input_id))
            .set((
                build_inputs::retries.eq(build_inputs::retries + 1),
                build_inputs::next_retry.eq(then.naive_utc()),
            ))
            .execute(connection.as_mut())
            .map_err(Error::from)?;

        let new_queue = NewQueued {
            build_input_id: queued.build_input_id,
            priority: DEFAULT_QUEUE_PRIORITY + 1,
            queued_at: now.naive_utc(),
        };

        new_queue.upsert(connection.as_mut())?;
    }

    Ok(HttpResponse::NoContent())
}

#[get("/{id}")]
pub async fn get_build(pool: web::Data<Pool>, id: web::Path<i32>) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    if let Some(record) = rebuilds::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .filter(rebuilds::id.eq(*id))
        .select((
            rebuilds::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            build_inputs::architecture,
            build_inputs::backend,
            build_inputs::retries,
            rebuilds::started_at,
            rebuilds::built_at,
            rebuilds::status,
        ))
        .get_result::<Rebuild>(connection.as_mut())
        .optional()
        .map_err(Error::from)?
    {
        Ok(HttpResponse::Ok().json(record))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/{id}/log")]
pub async fn get_build_log(
    req: HttpRequest,
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let build_log = rebuilds::table
        .filter(rebuilds::id.eq(id.into_inner()))
        .inner_join(build_logs::table)
        .select(build_logs::build_log)
        .first::<Vec<u8>>(connection.as_mut())
        .map_err(Error::from)?;

    forward_compressed_data(req, "text/plain; charset=utf-8", build_log).await
}

#[get("/{id}/artifacts")]
pub async fn get_build_artifacts(
    pool: web::Data<Pool>,
    id: web::Path<i32>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    let records = rebuilds::table
        .inner_join(
            rebuild_artifacts::table
                .left_join(diffoscope_logs::table)
                .left_join(attestation_logs::table),
        )
        .filter(rebuilds::id.eq(id.into_inner()))
        .select((
            rebuild_artifacts::id,
            rebuild_artifacts::name,
            diffoscope_logs::diffoscope_log.nullable().is_not_null(),
            attestation_logs::attestation_log.nullable().is_not_null(),
            rebuild_artifacts::status,
        ))
        .get_results::<api::v1::RebuildArtifact>(connection.as_mut())
        .map_err(Error::from)?;

    Ok(HttpResponse::Ok().json(records))
}

#[get("/{id}/artifacts/{artifact_id}")]
pub async fn get_build_artifact(
    pool: web::Data<Pool>,
    path: web::Path<(i32, i32)>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let artifact = rebuilds::table
        .inner_join(
            rebuild_artifacts::table
                .left_join(diffoscope_logs::table)
                .left_join(attestation_logs::table),
        )
        .filter(rebuilds::id.eq(path.0))
        .filter(rebuild_artifacts::id.eq(path.1))
        .select((
            rebuild_artifacts::id,
            rebuild_artifacts::name,
            diffoscope_logs::diffoscope_log.nullable().is_not_null(),
            attestation_logs::attestation_log.nullable().is_not_null(),
            rebuild_artifacts::status,
        ))
        .first::<api::v1::RebuildArtifact>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(artifact) = artifact {
        Ok(HttpResponse::Ok().json(artifact))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/{id}/artifacts/{artifact_id}/diffoscope")]
pub async fn get_build_artifact_diffoscope(
    req: HttpRequest,
    pool: web::Data<Pool>,
    path: web::Path<(i32, i32)>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let diffoscope = rebuilds::table
        .inner_join(rebuild_artifacts::table.left_join(diffoscope_logs::table))
        .filter(rebuilds::id.eq(path.0))
        .filter(rebuild_artifacts::id.eq(path.1))
        .select(diffoscope_logs::diffoscope_log.nullable())
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    if let Some(diffoscope) = diffoscope.flatten() {
        forward_compressed_data(req, "text/plain; charset=utf-8", diffoscope).await
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/{id}/artifacts/{artifact_id}/attestation")]
pub async fn get_build_artifact_attestation(
    req: HttpRequest,
    pool: web::Data<Pool>,
    path: web::Path<(i32, i32)>,
    cfg: web::Data<Config>,
    private_key: web::Data<Arc<PrivateKey>>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let attestation = rebuilds::table
        .inner_join(rebuild_artifacts::table.left_join(attestation_logs::table))
        .filter(rebuilds::id.eq(path.0))
        .filter(rebuild_artifacts::id.eq(path.1))
        .select(attestation_logs::attestation_log.nullable())
        .first::<Option<Vec<u8>>>(connection.as_mut())
        .optional()
        .map_err(Error::from)?;

    let Some(mut attestation) = attestation.flatten() else {
        return Ok(HttpResponse::NotFound().finish());
    };

    if cfg.transparently_sign_attestations {
        let (bytes, has_new_signature) = attestation::compressed_attestation_sign_if_necessary(
            attestation.clone(),
            &private_key,
        )
        .await?;

        if has_new_signature {
            let attestation_id = rebuild_artifacts::table
                .filter(rebuild_artifacts::id.eq(path.1))
                .select(rebuild_artifacts::attestation_log_id.assume_not_null())
                .get_result::<i32>(connection.as_mut())
                .map_err(Error::from)?;

            // TODO: GET with side effects?
            update(attestation_logs::table)
                .filter(attestation_logs::id.eq(attestation_id))
                .set(attestation_logs::attestation_log.eq(bytes.clone()))
                .execute(connection.as_mut())
                .map_err(Error::from)?;

            attestation = bytes
        }
    }

    forward_compressed_data(req, "application/json; charset=utf-8", attestation).await
}
