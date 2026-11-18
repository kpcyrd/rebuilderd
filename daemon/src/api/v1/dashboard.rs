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
    DashboardFilter, DashboardJobState, DashboardRebuildState, DashboardState,
};
use rebuilderd_common::errors::Error;

use crate::api::v1::util::filters::IntoDashboardFilter;
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
    dashboard_filter: web::Query<DashboardFilter>,
) -> web::Result<impl Responder> {
    let mut connection = pool.get().map_err(Error::from)?;

    let sql = build_inputs::table
        .left_join(r1.on(r1.field(rebuilds::build_input_id).is(build_inputs::id)))
        .left_join(
            r2.on(r2.field(rebuilds::build_input_id).is(build_inputs::id).and(
                r1.field(rebuilds::built_at)
                    .lt(r2.field(rebuilds::built_at))
                    .or(r1
                        .field(rebuilds::built_at)
                        .is_null()
                        .and(r2.field(rebuilds::built_at).is_not_null()))
                    .or(r1.fields(
                        rebuilds::built_at
                            .is(r2.field(rebuilds::built_at))
                            .and(r1.field(rebuilds::id).lt(r2.field(rebuilds::id))),
                    )),
            )),
        )
        .filter(r2.field(rebuilds::id).is_null())
        .filter(dashboard_filter.clone().into_inner().into_filter())
        .into_boxed();

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
                    .ne_all(["GOOD", "BAD", "FAIL"])
                    .or(r1.field(rebuilds::status).nullable().is_null()),
                1,
            )
            .otherwise(0)),
        ))
        .get_result::<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(connection.as_mut())
        .map_err(Error::from)?;

    let now = Utc::now();

    let running_jobs = queue_count_base()
        .filter(dashboard_filter.clone().into_inner().into_filter())
        .filter(queue::worker.is_not_null())
        .count()
        .get_result::<i64>(connection.as_mut())
        .map_err(Error::from)?;

    let available_jobs = queue_count_base()
        .filter(dashboard_filter.clone().into_inner().into_filter())
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
        .filter(dashboard_filter.clone().into_inner().into_filter())
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, test};
    use diesel::connection::SimpleConnection;

    #[actix_web::test]
    async fn counts_latest_rebuild_once_per_matching_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dashboard.db");
        let pool = crate::db::setup_pool(path.to_str().unwrap()).unwrap();
        let mut connection = pool.get().unwrap();
        connection
            .batch_execute(
                "INSERT INTO source_packages
                    (id, name, version, distribution, release, last_seen, seen_in_last_sync)
                 VALUES (1, 'source', '1', 'distro', 'release', '2020-01-01', 1);
                 INSERT INTO build_inputs (id, source_package_id, url, backend, architecture, retries)
                 VALUES (1, 1, '1', 'distro', 'arch', 0),
                        (2, 1, '2', 'distro', 'arch', 0),
                        (3, 1, '3', 'distro', 'arch', 0),
                        (4, 1, '4', 'distro', 'arch', 0),
                        (5, 1, '5', 'distro', 'arch', 0),
                        (6, 1, '6', 'distro', 'arch', 0),
                        (7, 1, '7', 'distro', 'arch', 0),
                        (8, 1, '8', 'distro', 'arch', 0);
                 INSERT INTO binary_packages
                    (source_package_id, build_input_id, name, version, component, architecture,
                     artifact_url, last_seen, seen_in_last_sync)
                 VALUES (1, 1, 'a', '1', 'main', 'arch', 'a', '2020-01-01', 1),
                        (1, 1, 'b', '1', 'main', 'arch', 'b', '2020-01-01', 1),
                        (1, 2, 'c', '1', 'main', 'arch', 'c', '2020-01-01', 1),
                        (1, 3, 'd', '1', 'main', 'arch', 'd', '2020-01-01', 1),
                        (1, 4, 'e', '1', 'main', 'arch', 'e', '2020-01-01', 1),
                        (1, 5, 'f', '1', 'main', 'arch', 'f', '2020-01-01', 1),
                        (1, 6, 'g', '1', 'other', 'arch', 'g', '2020-01-01', 1),
                        (1, 6, 'h', '1', 'main', 'arch', 'h', '2020-01-01', 0),
                        (1, 7, 'i', '1', 'main', 'arch', 'i', '2020-01-01', 0);
                 INSERT INTO build_logs (id, build_log) VALUES (1, X'');
                 INSERT INTO rebuilds (id, build_input_id, built_at, build_log_id, status)
                 VALUES (1, 1, NULL, 1, 'BAD'),
                        (2, 1, '2020-01-01', 1, 'GOOD'),
                        (3, 2, '2020-01-01', 1, 'GOOD'),
                        (4, 2, '2020-01-01', 1, 'BAD'),
                        (5, 3, NULL, 1, 'GOOD'),
                        (6, 3, NULL, 1, 'FAIL'),
                        (7, 4, '2020-01-01', 1, 'OTHER'),
                        (8, 5, '2020-01-01', 1, NULL),
                        (9, 7, '2020-01-01', 1, 'GOOD'),
                        (10, 1, '2019-01-01', 1, 'FAIL'),
                        (11, 5, '2019-01-01', 1, 'GOOD');",
            )
            .unwrap();
        drop(connection);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .service(actix_web::web::scope("/dashboard").service(get_dashboard)),
        )
        .await;

        for (component, unknown) in [("", 3), ("&component=main", 2)] {
            let request = test::TestRequest::get()
                .uri(&format!(
                    "/dashboard?distribution=distro&release=release&architecture=arch{component}"
                ))
                .to_request();
            let dashboard: DashboardState = test::call_and_read_body_json(&app, request).await;
            assert_eq!(dashboard.rebuilds.good, 1);
            assert_eq!(dashboard.rebuilds.bad, 1);
            assert_eq!(dashboard.rebuilds.fail, 1);
            assert_eq!(dashboard.rebuilds.unknown, unknown);
        }
    }
}
