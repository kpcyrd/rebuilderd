use crate::api::v1::models::queue::QueuedJob;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardState {
    pub distributions: HashMap<String, DistributionStatistics>,
    pub active_builds: Vec<QueuedJob>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DistributionStatistics {
    pub good: u32,
    pub bad: u32,
    pub unknown: u32,
}
