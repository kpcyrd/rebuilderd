use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardState {
    pub rebuilds: DashboardRebuildState,
    pub jobs: DashboardJobState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardRebuildState {
    pub good: i64,
    pub bad: i64,
    pub fail: i64,
    pub unknown: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardJobState {
    pub running: i64,
    pub available: i64,
    pub pending: i64,
}
