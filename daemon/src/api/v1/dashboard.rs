use crate::db::Pool;
use crate::schema::{build_inputs, queue, source_packages};
use crate::web;
use actix_web::{HttpResponse, Responder, get};
use chrono::Utc;
use diesel::deserialize::QueryableByName;
use diesel::sql_types::Text;
use diesel::sqlite::Sqlite;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, SqliteExpressionMethods,
    sql_query,
};
use rebuilderd_common::api::v1::{
    DashboardJobState, DashboardRebuildState, DashboardState, OriginFilter,
};
use rebuilderd_common::errors::Error;

use crate::api::v1::util::filters::IntoOriginFilter;

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
}

#[diesel::dsl::auto_type]
fn queue_count_base<'a>() -> _ {
    let mut sql = queue::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .into_boxed::<'a, Sqlite>();

    // dashboards rarely care about historical data for sums
    sql = sql.filter(source_packages::seen_in_last_sync.is(true));

    sql
}

#[get("")]
pub async fn get_dashboard(
    pool: web::Data<Pool>,
    origin_filter: web::Query<OriginFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    #[derive(Debug, QueryableByName)]
    struct DashboardRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        status: String,

        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let rows = sql_query("
SELECT a.status, count(DISTINCT s.name) as count
FROM rebuild_artifacts a
RIGHT JOIN rebuilds r ON a.rebuild_id = r.id
JOIN build_inputs i ON r.build_input_id = i.id
JOIN source_packages s ON i.source_package_id = s.id AND s.seen_in_last_sync = True AND s.release = ? and i.architecture = ? AND r.id IN (
    SELECT r.id
    FROM rebuilds r
    JOIN build_inputs i ON r.build_input_id = i.id
    where i.architecture = 'arm64'
    GROUP BY i.source_package_id
    HAVING r.id = MAX(r.id)
)
GROUP BY a.status;
")
    .bind::<Text, _>("forky")
    .bind::<Text, _>("arm64")
    .load::<DashboardRow>(connection.as_mut())
    .map_err(Error::from)?;

    let rebuilds = rows
        .into_iter()
        .fold(DashboardRebuildState::default(), |mut acc, row| {
            match row.status.as_str() {
                "GOOD" => acc.good = row.count,
                "BAD" => acc.bad = row.count,
                "FAIL" => acc.fail = row.count,
                "UNKNOWN" => acc.unknown = row.count,
                _ => (),
            }
            acc
        });

    let now = Utc::now();

    let running_jobs = queue_count_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(queue::worker.is_not_null())
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    let available_jobs = queue_count_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(queue::worker.is_null())
        .filter(
            build_inputs::next_retry
                .is_null()
                .or(build_inputs::next_retry.le(now.naive_utc())),
        )
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    let pending_jobs = queue_count_base()
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .filter(queue::worker.is_null())
        .filter(
            build_inputs::next_retry
                .is_not_null()
                .and(build_inputs::next_retry.gt(now.naive_utc())),
        )
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    let dashboard = DashboardState {
        rebuilds,
        jobs: DashboardJobState {
            running: running_jobs,
            available: available_jobs,
            pending: pending_jobs,
        },
    };

    Ok(HttpResponse::Ok().json(dashboard))
}
