use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsCategoryCount {
    pub category: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub id: i32,
    pub captured_at: NaiveDateTime,
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
    pub good: i32,
    pub bad: i32,
    pub fail: i32,
    pub unknown: i32,
    /// Per-category breakdown of bad/failed packages. Empty when no stats config
    /// backend matched or when categories have not been configured.
    pub categories: Vec<StatsCategoryCount>,
}

/// Sent by `rebuildctl stats collect` to trigger a snapshot on the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsCollectRequest {
    /// Backend name used to look up error categories in rebuilderd-stats.conf
    /// (e.g. "debian"). Leave empty to skip error categorization.
    pub backend: Option<String>,
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsFilter {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
    /// Only return snapshots captured at or after this timestamp.
    pub since: Option<NaiveDateTime>,
    /// Maximum number of snapshots to return (default: 100).
    pub limit: Option<i64>,
}
