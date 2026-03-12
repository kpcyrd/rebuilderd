use crate::api::v1::util::auth;
use crate::config::Config;
use crate::db::Pool;
use crate::models::{NewStatsCategory, NewStatsSnapshot};
use crate::schema::{build_inputs, rebuilds, source_packages, stats, stats_categories};
use crate::stats_config::StatsConfigFile;
use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post};
use chrono::{NaiveDateTime, Utc};
use diesel::dsl::{case_when, sum};
use diesel::prelude::*;
use diesel::sql_types::{Binary, Integer, Nullable};
use diesel::{BoolExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl};
use diesel::{ExpressionMethods, SqliteExpressionMethods};
use rebuilderd_common::api::v1::{StatsCategoryCount, StatsCollectRequest, StatsSnapshot};
use rebuilderd_common::errors::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

mod aliases {
    diesel::alias!(
        crate::schema::rebuilds as r1: RebuildsAlias1,
        crate::schema::rebuilds as r2: RebuildsAlias2
    );
}

use aliases::*;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(QueryableByName, Debug)]
struct IdRow {
    #[diesel(sql_type = Integer)]
    id: i32,
}

#[derive(QueryableByName, Debug)]
struct BadPackageRow {
    #[diesel(sql_type = Binary)]
    build_log: Vec<u8>,
    #[diesel(sql_type = Nullable<Binary>)]
    diffoscope_log: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// GET /api/v1/stats
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub limit: Option<i64>,
}

#[get("")]
pub async fn get_stats(
    pool: web::Data<Pool>,
    query: web::Query<StatsQuery>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;
    let limit = query.limit.unwrap_or(100).min(10_000);

    let mut sql = stats::table
        .select((
            stats::id,
            stats::captured_at,
            stats::distribution,
            stats::release,
            stats::architecture,
            stats::good,
            stats::bad,
            stats::fail,
            stats::unknown,
        ))
        .order(stats::captured_at.asc())
        .into_boxed();

    if let Some(dist) = &query.distribution {
        sql = sql.filter(stats::distribution.eq(dist));
    }
    if let Some(rel) = &query.release {
        sql = sql.filter(stats::release.eq(rel));
    }
    if let Some(arch) = &query.architecture {
        sql = sql.filter(stats::architecture.eq(arch));
    }
    if let Some(since) = query.since {
        sql = sql.filter(stats::captured_at.ge(since));
    }

    // Keep only the latest snapshot per (day, distribution, release, architecture).
    let latest_ids: Vec<i32> = diesel::sql_query(
        "SELECT MAX(id) AS id FROM stats \
         GROUP BY DATE(captured_at), distribution, release, architecture",
    )
    .load::<IdRow>(connection.as_mut())
    .map_err(Error::from)?
    .into_iter()
    .map(|r| r.id)
    .collect();

    sql = sql.filter(stats::id.eq_any(latest_ids));

    let rows = sql
        .limit(limit)
        .get_results::<(i32, NaiveDateTime, Option<String>, Option<String>, Option<String>, i32, i32, i32, i32)>(
            connection.as_mut(),
        )
        .map_err(Error::from)?;

    // Batch-load categories for all returned snapshots.
    let ids: Vec<i32> = rows.iter().map(|(id, ..)| *id).collect();

    let cat_rows = stats_categories::table
        .filter(stats_categories::stats_id.eq_any(&ids))
        .select((
            stats_categories::stats_id,
            stats_categories::category,
            stats_categories::count,
        ))
        .get_results::<(i32, String, i32)>(connection.as_mut())
        .map_err(Error::from)?;

    let mut cats_by_snapshot: HashMap<i32, Vec<StatsCategoryCount>> = HashMap::new();
    for (stats_id, category, count) in cat_rows {
        cats_by_snapshot
            .entry(stats_id)
            .or_default()
            .push(StatsCategoryCount { category, count });
    }

    let snapshots: Vec<StatsSnapshot> = rows
        .into_iter()
        .map(|(id, captured_at, distribution, release, architecture, good, bad, fail, unknown)| {
            let categories = cats_by_snapshot.remove(&id).unwrap_or_default();
            StatsSnapshot { id, captured_at, distribution, release, architecture, good, bad, fail, unknown, categories }
        })
        .collect();

    Ok(HttpResponse::Ok().json(snapshots))
}

// ---------------------------------------------------------------------------
// POST /api/v1/stats
// ---------------------------------------------------------------------------

#[post("")]
pub async fn collect_stats(
    pool: web::Data<Pool>,
    config: web::Data<Config>,
    stats_cfg: web::Data<Arc<StatsConfigFile>>,
    req: HttpRequest,
    body: web::Json<StatsCollectRequest>,
) -> web::Result<impl Responder> {
    auth::admin(&config, &req).map_err(Error::from)?;

    let mut connection = pool.get().map_err(Error::from)?;
    let now = Utc::now().naive_utc();

    let snapshots = if body.distribution.is_none()
        && body.release.is_none()
        && body.architecture.is_none()
    {
        // Enumerate all backends from the stats config (excluding "all"),
        // then for each backend query the DB for distinct (release, architecture)
        // combos and collect one snapshot per combo.
        let backend_names: Vec<String> = stats_cfg
            .backends
            .keys()
            .filter(|k| k.as_str() != "all")
            .cloned()
            .collect();

        let mut snapshots = Vec::new();
        for backend in &backend_names {
            let combos = query_combos_for_backend(&mut *connection, backend)
                .map_err(Error::from)?;
            for (release, architecture) in combos {
                let snapshot = collect_one(
                    &mut *connection,
                    &stats_cfg,
                    Some(backend.as_str()),
                    release.as_deref(),
                    Some(architecture.as_str()),
                    Some(backend.as_str()),
                    now,
                )
                .map_err(Error::from)?;
                snapshots.push(snapshot);
            }
        }
        snapshots
    } else {
        // Single-combo mode: honour the explicit filters from the request body.
        // If no backend was specified, fall back to using the distribution name.
        let backend = body
            .backend
            .as_deref()
            .or(body.distribution.as_deref());

        let snapshot = collect_one(
            &mut *connection,
            &stats_cfg,
            body.distribution.as_deref(),
            body.release.as_deref(),
            body.architecture.as_deref(),
            backend,
            now,
        )
        .map_err(Error::from)?;

        vec![snapshot]
    };

    Ok(HttpResponse::Ok().json(snapshots))
}

// ---------------------------------------------------------------------------
// Core snapshot collection
// ---------------------------------------------------------------------------

fn query_combos_for_backend(
    connection: &mut crate::db::SqliteConnectionWrap,
    distribution: &str,
) -> rebuilderd_common::errors::Result<Vec<(Option<String>, String)>> {
    let rows = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.eq(distribution))
        .filter(source_packages::seen_in_last_sync.is(true))
        .select((
            source_packages::release,
            build_inputs::architecture,
        ))
        .distinct()
        .get_results::<(Option<String>, String)>(connection.as_mut())
        .map_err(rebuilderd_common::errors::Error::from)?;

    Ok(rows)
}

fn collect_one(
    connection: &mut crate::db::SqliteConnectionWrap,
    stats_cfg: &StatsConfigFile,
    distribution: Option<&str>,
    release: Option<&str>,
    architecture: Option<&str>,
    backend: Option<&str>,
    now: NaiveDateTime,
) -> rebuilderd_common::errors::Result<StatsSnapshot> {
    // ------------------------------------------------------------------
    // Rebuild counts: latest rebuild per build_input, filtered by origin
    // ------------------------------------------------------------------
    let mut rebuild_sql = source_packages::table
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).is(build_inputs::id)))
        .left_join(
            r2.on(r2
                .field(rebuilds::build_input_id)
                .is(build_inputs::id)
                .and(
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
        .filter(source_packages::seen_in_last_sync.is(true))
        .into_boxed();

    if let Some(dist) = distribution {
        rebuild_sql = rebuild_sql.filter(source_packages::distribution.eq(dist));
    }
    if let Some(rel) = release {
        rebuild_sql = rebuild_sql.filter(source_packages::release.eq(rel));
    }
    if let Some(arch) = architecture {
        rebuild_sql = rebuild_sql.filter(build_inputs::architecture.eq(arch));
    }

    let (good, bad, fail, unknown) = rebuild_sql
        .select((
            sum(case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("GOOD"), 1).otherwise(0)),
            sum(case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("BAD"),  1).otherwise(0)),
            sum(case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("FAIL"), 1).otherwise(0)),
            sum(case_when::<_, _, Integer>(
                r1.field(rebuilds::status).nullable().eq("UNKWN")
                    .or(r1.field(rebuilds::status).nullable().is_null()),
                1,
            ).otherwise(0)),
        ))
        .get_result::<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(connection.as_mut())
        .map_err(rebuilderd_common::errors::Error::from)?;

    // ------------------------------------------------------------------
    // Insert snapshot
    // ------------------------------------------------------------------
    let new_snapshot = NewStatsSnapshot {
        captured_at: now,
        distribution: distribution.map(|s| s.to_owned()),
        release: release.map(|s| s.to_owned()),
        architecture: architecture.map(|s| s.to_owned()),
        good: good.unwrap_or(0) as i32,
        bad: bad.unwrap_or(0) as i32,
        fail: fail.unwrap_or(0) as i32,
        unknown: unknown.unwrap_or(0) as i32,
    };

    let snapshot_id = new_snapshot
        .insert(connection.as_mut())
        .map_err(rebuilderd_common::errors::Error::from)?;

    // ------------------------------------------------------------------
    // Error category breakdown
    // ------------------------------------------------------------------
    let category_counts = if let Some(backend_name) = backend {
        let categories = stats_cfg.categories_for(backend_name);
        if categories.is_empty() {
            log::warn!(
                "Stats collect: backend {:?} not found in stats config, skipping categorization",
                backend_name
            );
        }
        categorize_bad_packages(connection, distribution, release, architecture, &categories)
            .map_err(rebuilderd_common::errors::Error::from)?
    } else {
        vec![]
    };

    let category_rows: Vec<NewStatsCategory> = category_counts
        .iter()
        .map(|(cat, count)| NewStatsCategory {
            stats_id: snapshot_id,
            category: cat.clone(),
            count: *count,
        })
        .collect();

    if !category_rows.is_empty() {
        NewStatsCategory::insert_batch(&category_rows, connection.as_mut())
            .map_err(rebuilderd_common::errors::Error::from)?;
    }

    let categories = category_counts
        .into_iter()
        .map(|(category, count)| StatsCategoryCount { category, count })
        .collect();

    Ok(StatsSnapshot {
        id: snapshot_id,
        captured_at: now,
        distribution: new_snapshot.distribution,
        release: new_snapshot.release,
        architecture: new_snapshot.architecture,
        good: new_snapshot.good,
        bad: new_snapshot.bad,
        fail: new_snapshot.fail,
        unknown: new_snapshot.unknown,
        categories,
    })
}

// ---------------------------------------------------------------------------
// Categorization logic
// ---------------------------------------------------------------------------

fn categorize_bad_packages(
    connection: &mut crate::db::SqliteConnectionWrap,
    distribution: Option<&str>,
    release: Option<&str>,
    architecture: Option<&str>,
    categories: &[&crate::stats_config::ErrorCategory],
) -> rebuilderd_common::errors::Result<Vec<(String, i32)>> {
    // Use raw SQL for the complex join + subquery; Diesel's type-safe builder
    // becomes unwieldy for this particular shape.
    let mut where_clauses = vec![
        "sp.seen_in_last_sync = 1".to_string(),
    ];

    if let Some(dist) = distribution {
        where_clauses.push(format!("sp.distribution = '{}'", dist.replace('\'', "''")));
    }
    if let Some(rel) = release {
        where_clauses.push(format!("sp.release = '{}'", rel.replace('\'', "''")));
    }
    if let Some(arch) = architecture {
        where_clauses.push(format!("bi.architecture = '{}'", arch.replace('\'', "''")));
    }

    let sql = format!(
        "SELECT l.build_log, \
                ( SELECT d.diffoscope_log \
                  FROM rebuild_artifacts a \
                  LEFT JOIN diffoscope_logs d ON a.diffoscope_log_id = d.id \
                  WHERE a.rebuild_id = r.id AND d.diffoscope_log IS NOT NULL \
                  LIMIT 1 ) AS diffoscope_log \
         FROM rebuilds r \
         JOIN build_logs l ON r.build_log_id = l.id \
         JOIN build_inputs bi ON r.build_input_id = bi.id \
         JOIN source_packages sp ON bi.source_package_id = sp.id \
         WHERE {} \
         AND ( EXISTS ( SELECT 1 FROM rebuild_artifacts a \
                        WHERE a.rebuild_id = r.id AND a.status = 'BAD' ) \
               OR r.status = 'FAIL' ) \
         AND r.id IN ( \
             SELECT MAX(r2.id) FROM rebuilds r2 \
             GROUP BY r2.build_input_id \
         )",
        where_clauses.join(" AND ")
    );

    let rows = diesel::sql_query(&sql)
        .load::<BadPackageRow>(connection)
        .map_err(rebuilderd_common::errors::Error::from)?;

    let mut counts: HashMap<String, i32> = HashMap::new();

    'outer: for row in &rows {
        let log = zstd::stream::decode_all(row.build_log.as_slice()).unwrap_or_default();
        let log = String::from_utf8_lossy(&log);

        let diff = row.diffoscope_log.as_deref()
            .map(|d| String::from_utf8_lossy(&zstd::stream::decode_all(d).unwrap_or_default()).into_owned())
            .unwrap_or_default();

        for cat in categories {
            match cat.matches(&log, &diff, architecture) {
                Ok(true) => {
                    *counts.entry(cat.name.clone()).or_insert(0) += 1;
                    continue 'outer;
                }
                Ok(false) => continue,
                Err(e) => {
                    log::warn!("Error matching category {:?}: {e}", cat.name);
                    continue;
                }
            }
        }
    }

    // Return in config order so the response is stable.
    let result = categories
        .iter()
        .filter_map(|cat| counts.get(&cat.name).map(|&c| (cat.name.clone(), c)))
        .collect();

    Ok(result)
}
