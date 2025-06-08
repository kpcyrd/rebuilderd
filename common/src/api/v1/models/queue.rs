#[cfg(feature = "diesel")]
use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueJobRequest {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PopQueuedJobRequest {
    pub supported_backends: Vec<String>,
    pub architecture: String,
    pub supported_architectures: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct QueuedJob {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub backend: String,
    pub url: String,
}
