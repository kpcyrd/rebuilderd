use crate::db::Pool;
use crate::schema::{build_inputs, queue, rebuilds, source_packages};
use crate::web;
use actix_web::{HttpResponse, Responder, get};
use chrono::Utc;
use diesel::NullableExpressionMethods;
use diesel::RunQueryDsl;
use diesel::dsl::{case_when, sum};
use diesel::sql_types::Integer;
use diesel::sqlite::Sqlite;
use diesel::{BoolExpressionMethods, JoinOnDsl, QueryDsl};
use diesel::{ExpressionMethods, SqliteExpressionMethods};
use rebuilderd_common::api::v1::{
    DashboardJobState, DashboardRebuildState, DashboardState, OriginFilter,
};
use rebuilderd_common::errors::Error;

use crate::api::v1::util::filters::IntoOriginFilter;
use aliases::*;

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

    let mut sql = source_packages::table
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).is(build_inputs::id)))
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).is(build_inputs::id).and(
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
        .filter(
            origin_filter
                .clone()
                .into_inner()
                .into_filter(build_inputs::architecture),
        )
        .into_boxed();

    // dashboards rarely care about historical data for sums
    sql = sql.filter(source_packages::seen_in_last_sync.is(true));

    let sums = sql
        .select((
            sum(
                case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("GOOD"), 1)
                    .otherwise(0),
            ),
            sum(
                case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("BAD"), 1)
                    .otherwise(0),
            ),
            sum(
                case_when::<_, _, Integer>(r1.field(rebuilds::status).nullable().eq("FAIL"), 1)
                    .otherwise(0),
            ),
            sum(case_when::<_, _, Integer>(
                r1.field(rebuilds::status)
                    .nullable()
                    .eq("UNKWN")
                    .or(r1.field(rebuilds::status).nullable().is_null()),
                1,
            )
            .otherwise(0)),
        ))
        .get_result::<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(connection.as_mut())
        .map_err(Error::from)?;

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
        rebuilds: DashboardRebuildState {
            good: sums.0.unwrap_or(0),
            bad: sums.1.unwrap_or(0),
            fail: sums.2.unwrap_or(0),
            unknown: sums.3.unwrap_or(0),
        },
        jobs: DashboardJobState {
            running: running_jobs,
            available: available_jobs,
            pending: pending_jobs,
        },
    };

    Ok(HttpResponse::Ok().json(dashboard))
}
