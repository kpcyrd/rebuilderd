use chrono::prelude::*;
use crate::models;
use rebuilderd_common::Status;
use rebuilderd_common::api::*;
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

    pub fn update(&mut self, connection: &diesel::SqliteConnection) -> Result<()> {
        models::Queued::free_stale_jobs(connection)?;
        let pkgs = models::Package::list(connection)?;
        let queue = models::Queued::list(None, connection)?;
        let queue_length = queue.len();

        let mut suites = HashMap::new();
        for pkg in pkgs {
            if !suites.contains_key(&pkg.suite) {
                suites.insert(pkg.suite.clone(), SuiteStats::default());
            }
            if let Some(stats) = suites.get_mut(&pkg.suite) {
                if let Ok(status) = pkg.status.parse() {
                    match status {
                        Status::Good => stats.good += 1,
                        Status::Unknown => stats.unknown += 1,
                        Status::Bad => stats.bad += 1,
                    }
                }
            }
        }

        let mut active_builds = Vec::new();
        for item in queue {
            if item.started_at.is_some() {
                let item = item.into_api_item(connection)?;
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
        if let Some(resp) =&self.response {
            Ok(resp)
        } else {
            bail!("No cached state")
        }
    }
}
