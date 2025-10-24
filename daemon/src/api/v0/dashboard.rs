use crate::api::v0::into_queue_item;
use crate::models::Queued;
use crate::schema::{build_inputs, queue, rebuilds, source_packages};
use chrono::prelude::*;
use diesel::dsl::{case_when, sum};
use diesel::sql_types::Integer;
use diesel::NullableExpressionMethods;
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use rebuilderd_common::api::v0::*;
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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

        let components = build_inputs::table
            .left_join(rebuilds::table)
            .inner_join(source_packages::table)
            .group_by(source_packages::component)
            .select((
                source_packages::component,
                sum(
                    case_when::<_, _, Integer>(rebuilds::status.nullable().eq("GOOD"), 1)
                        .otherwise(0),
                ),
                sum(
                    case_when::<_, _, Integer>(rebuilds::status.nullable().eq("BAD"), 1)
                        .otherwise(0),
                ),
                sum(case_when::<_, _, Integer>(
                    rebuilds::status
                        .nullable()
                        .eq("UNKWN")
                        .or(rebuilds::status.nullable().is_null()),
                    1,
                )
                .otherwise(0)),
            ))
            .get_results::<(Option<String>, Option<i64>, Option<i64>, Option<i64>)>(connection)?;

        let mut suites = HashMap::new();
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
