use crate::api::v1::models::queue::QueuedJob;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardState {
    pub good: i64,
    pub bad: i64,
    pub fail: i64,
    pub unknown: i64,
    pub active_builds: Vec<QueuedJob>,
}
