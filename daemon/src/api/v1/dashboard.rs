use crate::api::v1::util::filters::DieselOriginFilter;
use crate::db::Pool;
use crate::schema::{build_inputs, queue, rebuilds, source_packages};
use crate::web;
use actix_web::{get, HttpResponse, Responder};
use diesel::dsl::{case_when, sum};
use diesel::sql_types::Integer;
use diesel::ExpressionMethods;
use diesel::NullableExpressionMethods;
use diesel::RunQueryDsl;
use diesel::{BoolExpressionMethods, JoinOnDsl, QueryDsl};
use rebuilderd_common::api::v1::{DashboardState, OriginFilter, QueuedJob};
use rebuilderd_common::errors::Error;

use aliases::*;

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
}

#[get("")]
pub async fn get_dashboard(
    pool: web::Data<Pool>,
    origin_filter: web::Query<OriginFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let mut sql = source_packages::table
        .inner_join(build_inputs::table)
        .left_join(r1.on(r1.field(rebuilds::build_input_id).eq(build_inputs::id)))
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
        .into_boxed();

    sql = origin_filter.filter(sql);

    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

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

    let mut sql = queue::table
        .inner_join(build_inputs::table.inner_join(source_packages::table))
        .into_boxed();

    sql = origin_filter.filter(sql);

    if let Some(architecture) = &origin_filter.architecture {
        sql = sql.filter(build_inputs::architecture.eq(architecture));
    }

    let jobs = sql
        .select((
            queue::id,
            source_packages::name,
            source_packages::version,
            source_packages::distribution,
            source_packages::release,
            source_packages::component,
            build_inputs::architecture,
            build_inputs::backend,
            build_inputs::url,
            build_inputs::next_retry,
            queue::queued_at,
            queue::started_at,
        ))
        .load::<QueuedJob>(connection.as_mut())
        .map_err(Error::from)?;

    let dashboard = DashboardState {
        good: sums.0.unwrap_or(0),
        bad: sums.1.unwrap_or(0),
        fail: sums.2.unwrap_or(0),
        unknown: sums.3.unwrap_or(0),
        active_builds: jobs,
    };

    Ok(HttpResponse::Ok().json(dashboard))
}
