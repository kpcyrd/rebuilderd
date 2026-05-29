use crate::api::v0::into_queue_item;
use crate::models::Queued;
use crate::schema::{
    binary_packages, build_inputs, queue, rebuild_artifacts, rebuilds, source_packages,
};
use aliases::*;
use chrono::prelude::*;
use diesel::NullableExpressionMethods;
use diesel::dsl::{case_when, sum};
use diesel::sql_types::Integer;
use diesel::{BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v0::*;
use rebuilderd_common::errors::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

mod aliases {
    diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);
}

const DASHBOARD_UPDATE_INTERVAL: u64 = 1; // seconds

#[derive(Debug)]
pub struct DashboardState {
    response: Option<DashboardResponse>,
    last_update: Instant,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardState {
    pub fn new() -> DashboardState {
        DashboardState {
            response: None,
            last_update: Instant::now(),
        }
    }

    pub fn is_fresh(&self) -> bool {
        if self.response.is_some() {
            self.last_update.elapsed() < Duration::from_secs(DASHBOARD_UPDATE_INTERVAL)
        } else {
            false
        }
    }

    pub fn update(&mut self, connection: &mut diesel::SqliteConnection) -> Result<()> {
        let queue = queue::table
            .filter(queue::started_at.is_not_null())
            .load::<Queued>(connection)?;

        let queue_length = queue.len();

        let components = binary_packages::table
            .inner_join(build_inputs::table.inner_join(source_packages::table))
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
            .filter(source_packages::seen_in_last_sync.eq(true))
            .group_by(binary_packages::component)
            .select((
                binary_packages::component,
                sum(
                    case_when::<_, _, Integer>(rebuild_artifacts::status.nullable().eq("GOOD"), 1)
                        .otherwise(0),
                ),
                sum(
                    case_when::<_, _, Integer>(rebuild_artifacts::status.nullable().eq("BAD"), 1)
                        .otherwise(0),
                ),
                sum(case_when::<_, _, Integer>(
                    rebuild_artifacts::status
                        .nullable()
                        .eq("UNKWN")
                        .or(rebuild_artifacts::status.nullable().is_null()),
                    1,
                )
                .otherwise(0)),
            ))
            .get_results::<(Option<String>, Option<i64>, Option<i64>, Option<i64>)>(connection)?;

        let mut suites = BTreeMap::new();
        for (component, good, bad, unknown) in components {
            suites.insert(
                component.unwrap_or_default(), // TODO: behaviour change
                SuiteStats {
                    good: good.unwrap_or_default() as usize,
                    bad: bad.unwrap_or_default() as usize,
                    unknown: unknown.unwrap_or_default() as usize,
                },
            );
        }

        let mut active_builds = Vec::new();
        for item in queue {
            if item.started_at.is_some() {
                let item = into_queue_item(item, connection)?;
                active_builds.push(item);
            }
        }

        let now = Utc::now().naive_utc();
        self.response = Some(DashboardResponse {
            suites,
            active_builds,
            queue_length,
            now,
        });

        self.last_update = Instant::now();
        Ok(())
    }

    pub fn get_response(&self) -> Result<&DashboardResponse> {
        if let Some(resp) = &self.response {
            Ok(resp)
        } else {
            bail!("No cached state")
        }
    }
}
