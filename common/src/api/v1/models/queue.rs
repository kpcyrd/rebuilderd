use crate::api::v1::BuildStatus;
use chrono::NaiveDateTime;
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
    pub status: Option<BuildStatus>,
    pub priority: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PopQueuedJobRequest {
    pub supported_backends: Vec<String>,
    pub architecture: String,
    pub supported_architectures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub queued_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct QueuedJobArtifact {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedJobWithArtifacts {
    pub job: QueuedJob,
    pub artifacts: Vec<QueuedJobArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobAssignment {
    Nothing,
    Rebuild(Box<QueuedJobWithArtifacts>),
}
