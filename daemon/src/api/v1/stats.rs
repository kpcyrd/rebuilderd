use crate::api::v1::util::auth;
use crate::config::Config;
use crate::db::Pool;
use crate::models::{NewStatsCategory, NewStatsSnapshot};
use crate::schema::{build_inputs, build_logs, diffoscope_logs, rebuild_artifacts, rebuilds, source_packages, stats, stats_categories};
use crate::stats_config::{CompiledCategory, ErrorCategory, StatsConfigFile};
use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post};
use chrono::{NaiveDateTime, Utc};
use std::time::Instant;
use diesel::dsl::{case_when, max, sum};
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::{BoolExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl};
use diesel::{ExpressionMethods, SqliteExpressionMethods};
use rebuilderd_common::api::v1::{StatsCategoryCount, StatsCollectRequest, StatsSnapshot};
use rebuilderd_common::errors::{Error, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

diesel::alias!(
    crate::schema::rebuilds as r1: RebuildsAlias1,
    crate::schema::rebuilds as r2: RebuildsAlias2
);

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(QueryableByName, Debug)]
struct IdRow {
    #[diesel(sql_type = Integer)]
    id: i32,
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
    // This is done as a separate query rather than an inline subquery because Diesel's
    // boxed query builder cannot express GROUP BY DATE(captured_at) in a type-safe way.
    // The resulting ID list is small in practice (daily granularity × handful of distros)
    // so the two-round-trip approach is harmless.
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
    req: HttpRequest,
    cfg: web::Data<Config>,
    pool: web::Data<Pool>,
    stats_cfg: web::Data<Arc<StatsConfigFile>>,
    body: web::Json<StatsCollectRequest>,
) -> web::Result<impl Responder> {
    if auth::admin(&cfg, &req).is_err() {
        return Ok(HttpResponse::Forbidden().finish());
    }

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
            let combos = query_combos_for_backend(connection.as_mut(), backend)
                .map_err(Error::from)?;
            for (release, architecture) in combos {
                let snapshot = collect_one(
                    connection.as_mut(),
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
            connection.as_mut(),
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
    connection: &mut SqliteConnection,
    distribution: &str,
) -> Result<Vec<(Option<String>, String)>> {
    let rows = source_packages::table
        .inner_join(build_inputs::table)
        .filter(source_packages::distribution.eq(distribution))
        .filter(source_packages::seen_in_last_sync.is(true))
        .select((
            source_packages::release,
            build_inputs::architecture,
        ))
        .distinct()
        .get_results::<(Option<String>, String)>(connection)
        .map_err(Error::from)?;

    Ok(rows)
}

fn collect_one(
    connection: &mut SqliteConnection,
    stats_cfg: &StatsConfigFile,
    distribution: Option<&str>,
    release: Option<&str>,
    architecture: Option<&str>,
    backend: Option<&str>,
    now: NaiveDateTime,
) -> Result<StatsSnapshot> {
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

    let t = Instant::now();
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
        .get_result::<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(connection)
        .map_err(Error::from)?;
    log::info!("collect_one: rebuild counts query took {:?}", t.elapsed());

    // ------------------------------------------------------------------
    // Error category breakdown (read-only, before the write transaction)
    // ------------------------------------------------------------------
    let category_counts = if let Some(backend_name) = backend {
        let categories = stats_cfg.categories_for(backend_name);
        if categories.is_empty() {
            log::warn!(
                "Stats collect: backend {:?} not found in stats config, skipping categorization",
                backend_name
            );
        }
        categorize_bad_packages(connection, distribution, release, architecture, &categories)?
    } else {
        vec![]
    };

    // ------------------------------------------------------------------
    // Insert snapshot and categories atomically
    // ------------------------------------------------------------------
    let (snapshot_id, categories) = connection.transaction(|conn| {
        let new_snapshot = NewStatsSnapshot {
            captured_at: now,
            distribution: distribution.map(str::to_owned),
            release: release.map(str::to_owned),
            architecture: architecture.map(str::to_owned),
            good: good.unwrap_or(0) as i32,
            bad: bad.unwrap_or(0) as i32,
            fail: fail.unwrap_or(0) as i32,
            unknown: unknown.unwrap_or(0) as i32,
        };

        let id = new_snapshot.insert(conn).map_err(Error::from)?;

        let category_rows: Vec<NewStatsCategory> = category_counts
            .iter()
            .map(|(cat, count)| NewStatsCategory {
                stats_id: id,
                category: cat.clone(),
                count: *count,
            })
            .collect();

        if !category_rows.is_empty() {
            NewStatsCategory::insert_batch(&category_rows, conn).map_err(Error::from)?;
        }

        let categories = category_counts
            .into_iter()
            .map(|(category, count)| StatsCategoryCount { category, count })
            .collect();

        Ok::<_, Error>((id, categories))
    }).map_err(Error::from)?;

    Ok(StatsSnapshot {
        id: snapshot_id,
        captured_at: now,
        distribution: distribution.map(str::to_owned),
        release: release.map(str::to_owned),
        architecture: architecture.map(str::to_owned),
        good: good.unwrap_or(0) as i32,
        bad: bad.unwrap_or(0) as i32,
        fail: fail.unwrap_or(0) as i32,
        unknown: unknown.unwrap_or(0) as i32,
        categories,
    })
}

// ---------------------------------------------------------------------------
// Categorization logic
// ---------------------------------------------------------------------------

fn categorize_bad_packages(
    connection: &mut SqliteConnection,
    distribution: Option<&str>,
    release: Option<&str>,
    architecture: Option<&str>,
    categories: &[&ErrorCategory],
) -> Result<Vec<(String, i32)>> {
    // Step 1: Latest rebuild ID per build_input, pre-filtered to the relevant
    // distro/release/arch so subsequent IN clauses are small. The previous
    // approach loaded 198k global IDs causing SQLite to scan rebuild_artifacts
    // with a correlated EXISTS for each of them (~2 min per distro/arch combo).
    let t = Instant::now();
    let mut latest_ids_query = rebuilds::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .filter(source_packages::seen_in_last_sync.is(true))
        .select(max(rebuilds::id).assume_not_null())
        .group_by(rebuilds::build_input_id)
        .into_boxed::<diesel::sqlite::Sqlite>();
    if let Some(dist) = distribution {
        latest_ids_query = latest_ids_query.filter(source_packages::distribution.eq(dist));
    }
    if let Some(rel) = release {
        latest_ids_query = latest_ids_query.filter(source_packages::release.eq(rel));
    }
    if let Some(arch) = architecture {
        latest_ids_query = latest_ids_query.filter(build_inputs::architecture.eq(arch));
    }
    let latest_ids: Vec<i32> = latest_ids_query.load::<i32>(connection).map_err(Error::from)?;
    log::info!("categorize: latest_ids: {} rows in {:?}", latest_ids.len(), t.elapsed());

    if latest_ids.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: Find which of the latest rebuilds are BAD/FAIL via two cheap indexed
    // lookups and union them in Rust. Replaces the previous correlated EXISTS per row.
    let t = Instant::now();
    let bad_rebuild_ids: Vec<i32> = rebuild_artifacts::table
        .filter(rebuild_artifacts::rebuild_id.eq_any(&latest_ids))
        .filter(rebuild_artifacts::status.is("BAD"))
        .select(rebuild_artifacts::rebuild_id)
        .distinct()
        .load::<i32>(connection)
        .map_err(Error::from)?;
    let fail_rebuild_ids: Vec<i32> = rebuilds::table
        .filter(rebuilds::id.eq_any(&latest_ids))
        .filter(rebuilds::status.is("FAIL"))
        .select(rebuilds::id)
        .load::<i32>(connection)
        .map_err(Error::from)?;
    let bad_or_fail_ids: Vec<i32> = {
        let mut ids: HashSet<i32> = bad_rebuild_ids.into_iter().collect();
        ids.extend(fail_rebuild_ids);
        ids.into_iter().collect()
    };
    log::info!("categorize: bad/fail ids: {} in {:?}", bad_or_fail_ids.len(), t.elapsed());

    if bad_or_fail_ids.is_empty() {
        return Ok(vec![]);
    }

    // Step 3: Load build logs only for the bad/fail subset.
    let t = Instant::now();
    let rebuild_rows = rebuilds::table
        .inner_join(build_logs::table)
        .filter(rebuilds::id.eq_any(&bad_or_fail_ids))
        .select((rebuilds::id, build_logs::build_log))
        .load::<(i32, Vec<u8>)>(connection)
        .map_err(Error::from)?;
    log::info!("categorize: build logs: {} rows in {:?}", rebuild_rows.len(), t.elapsed());

    // Step 4: Diffoscope logs for the matched rebuilds.
    let rebuild_ids: Vec<i32> = rebuild_rows.iter().map(|(id, _)| *id).collect();
    let t = Instant::now();
    let diffoscope_rows = rebuild_artifacts::table
        .inner_join(diffoscope_logs::table)
        .filter(rebuild_artifacts::rebuild_id.eq_any(&rebuild_ids))
        .select((rebuild_artifacts::rebuild_id, diffoscope_logs::diffoscope_log))
        .load::<(i32, Vec<u8>)>(connection)
        .map_err(Error::from)?;
    log::info!("categorize: diffoscope logs: {} rows in {:?}", diffoscope_rows.len(), t.elapsed());

    // First diffoscope log per rebuild wins (rebuild_artifacts may have multiple entries).
    let mut diffoscope_by_rebuild: HashMap<i32, Vec<u8>> = HashMap::new();
    for (rebuild_id, log) in diffoscope_rows {
        diffoscope_by_rebuild.entry(rebuild_id).or_insert(log);
    }

    // Pre-compile regexes once before the per-package loop.
    let compiled: Vec<CompiledCategory<'_>> = categories
        .iter()
        .map(|cat| cat.compile())
        .collect::<Result<_>>()?;

    let mut counts: HashMap<String, i32> = HashMap::new();

    let t = Instant::now();
    'outer: for (rebuild_id, build_log) in &rebuild_rows {
        let log = zstd::stream::decode_all(build_log.as_slice()).unwrap_or_default();
        let log = String::from_utf8_lossy(&log);

        let diff = diffoscope_by_rebuild
            .get(rebuild_id)
            .map(|d| {
                String::from_utf8_lossy(&zstd::stream::decode_all(d.as_slice()).unwrap_or_default())
                    .into_owned()
            })
            .unwrap_or_default();

        for cat in &compiled {
            match cat.matches(&log, &diff, architecture) {
                Ok(true) => {
                    *counts.entry(cat.inner.name.clone()).or_insert(0) += 1;
                    continue 'outer;
                }
                Ok(false) => continue,
                Err(e) => {
                    log::warn!("Error matching category {:?}: {e}", cat.inner.name);
                    continue;
                }
            }
        }
    }
    log::info!("categorize: regex matching loop: {:?}", t.elapsed());

    // Return in config order so the response is stable.
    let result = categories
        .iter()
        .filter_map(|cat| counts.get(&cat.name).map(|&c| (cat.name.clone(), c)))
        .collect();

    Ok(result)
}
